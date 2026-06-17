use crate::edits::EditSet;
use crate::pyutil;
use crate::Options;
use std::io::Write;
use std::process::{Command, Stdio};
use tree_sitter::{Node, Tree};

/// Collect edits that reformat embedded SQL/JSON/markdown strings.
///
/// Detection: the string is the right-hand side of a top-level assignment to
/// a name ending in `_sql`, `_json`, `_md`, or `_markdown` (case-insensitive),
/// OR the string node has a leading `# language: <lang>` marker comment.
/// Only triple-quoted strings are touched.
pub fn collect_edits(tree: &Tree, source: &str, opts: &Options, edits: &mut EditSet) {
    pyutil::walk(tree.root_node(), |node| {
        if node.kind() != "string" {
            return;
        }
        let Some(lang) = detect_language(node, source) else {
            return;
        };
        // Only triple-quoted: we must not collapse single-quoted into multi-line.
        let text = pyutil::node_text(node, source);
        if !(text.contains("\"\"\"") || text.contains("'''")) {
            return;
        }
        if !pyutil::any_line_over(
            source,
            node.start_byte(),
            node.end_byte(),
            opts.line_length,
            opts.tab_width,
        ) {
            return;
        }
        let Some(replacement) = reformat(node, source, lang) else {
            return;
        };
        edits.push(node.start_byte(), node.end_byte(), replacement);
    });
}

#[derive(Copy, Clone, Debug)]
enum Lang {
    Sql,
    Json,
    Markdown,
}

fn detect_language(node: Node, source: &str) -> Option<Lang> {
    // Look for assignment target name suffix.
    let assignment = nearest_ancestor(node, "assignment")?;
    let left = assignment.child_by_field_name("left")?;
    let name = pyutil::node_text(left, source);
    let lname = name.to_ascii_lowercase();
    if name_has_suffix(&lname, "sql") {
        return Some(Lang::Sql);
    }
    if name_has_suffix(&lname, "json") {
        return Some(Lang::Json);
    }
    if name_has_suffix(&lname, "md") || name_has_suffix(&lname, "markdown") {
        return Some(Lang::Markdown);
    }
    None
}

/// True if `name` ends in `suffix`, preceded by an underscore or at the start.
fn name_has_suffix(name: &str, suffix: &str) -> bool {
    if name == suffix {
        return true;
    }
    let needle = format!("_{suffix}");
    name.ends_with(&needle)
}

fn nearest_ancestor<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cur = node.parent();
    while let Some(n) = cur {
        if n.kind() == kind {
            return Some(n);
        }
        cur = n.parent();
    }
    None
}

fn reformat(node: Node, source: &str, lang: Lang) -> Option<String> {
    let text = pyutil::node_text(node, source);
    let (prefix_len, quote) = quote_info(text)?;
    if quote.len() != 3 {
        return None;
    }
    let prefix = &text[..prefix_len];
    let inner = &text[prefix_len + quote.len()..text.len() - quote.len()];
    let stripped = inner.trim_matches('\n');

    let formatted = match lang {
        Lang::Sql => run_formatter("sqlfluff", &["format", "--dialect", "ansi", "-"], stripped)?,
        Lang::Json => run_formatter("jq", &["."], stripped)?,
        Lang::Markdown => run_formatter("prettier", &["--parser", "markdown"], stripped)?,
    };
    let formatted = formatted.trim_end_matches('\n');

    let indent = pyutil::indent_of_line(source, node.start_byte()).to_string();
    let mut body = String::new();
    body.push('\n');
    for line in formatted.split('\n') {
        body.push_str(&indent);
        body.push_str(line);
        body.push('\n');
    }
    body.push_str(&indent);

    let rebuilt = format!("{}{}{}{}", prefix, quote, body, quote);
    if rebuilt == text {
        return None;
    }
    Some(rebuilt)
}

fn run_formatter(cmd: &str, args: &[&str], stdin: &str) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    {
        let mut sin = child.stdin.take()?;
        sin.write_all(stdin.as_bytes()).ok()?;
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn quote_info(text: &str) -> Option<(usize, &str)> {
    let mut idx = 0usize;
    let bytes = text.as_bytes();
    while idx < bytes.len() {
        let c = bytes[idx] as char;
        if matches!(c, 'r' | 'R' | 'b' | 'B' | 'f' | 'F' | 'u' | 'U') {
            idx += 1;
        } else {
            break;
        }
    }
    let rest = &text[idx..];
    let q = if rest.starts_with("\"\"\"") {
        "\"\"\""
    } else if rest.starts_with("'''") {
        "'''"
    } else if rest.starts_with('"') {
        "\""
    } else if rest.starts_with('\'') {
        "'"
    } else {
        return None;
    };
    Some((idx, q))
}

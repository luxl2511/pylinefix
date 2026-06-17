use crate::edits::EditSet;
use crate::pyutil;
use crate::Options;
use tree_sitter::{Node, Tree};

/// Collect edits that wrap overlong docstrings to fit `line_length`.
pub fn collect_edits(tree: &Tree, source: &str, opts: &Options, edits: &mut EditSet) {
    pyutil::walk(tree.root_node(), |node| {
        if !is_docstring_target(node) {
            return;
        }
        let Some(string_node) = first_string_in(node) else {
            return;
        };
        if let Some(rep) = wrap_docstring(string_node, source, opts) {
            edits.push(string_node.start_byte(), string_node.end_byte(), rep);
        }
    });
}

/// True if `node` is the docstring expression-statement of a module, class, or
/// function body.
fn is_docstring_target(node: Node) -> bool {
    if node.kind() != "expression_statement" {
        return false;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    if !matches!(parent.kind(), "module" | "block") {
        return false;
    }
    // Must be the first non-comment child.
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if child.kind() == "comment" {
            continue;
        }
        return child.id() == node.id();
    }
    false
}

fn first_string_in(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|&child| child.kind() == "string")
}

fn wrap_docstring(node: Node, source: &str, opts: &Options) -> Option<String> {
    let text = pyutil::node_text(node, source);
    let (prefix_len, quote) = quote_info(text)?;
    if quote.len() != 3 {
        return None;
    }
    let prefix = &text[..prefix_len];
    let inner = &text[prefix_len + quote.len()..text.len() - quote.len()];

    if !pyutil::any_line_over(
        source,
        node.start_byte(),
        node.end_byte(),
        opts.line_length,
        opts.tab_width,
    ) {
        return None;
    }

    let indent = pyutil::indent_of_line(source, node.start_byte()).to_string();
    let budget = opts
        .line_length
        .saturating_sub(indent.chars().count())
        .max(20);

    let leading_newline = inner.starts_with('\n');
    let trailing_newline = inner.ends_with('\n');
    let body = if leading_newline { &inner[1..] } else { inner };
    let body = if trailing_newline { body.trim_end_matches('\n') } else { body };

    let blocks = parse_blocks(body, &indent);
    let rendered = render_blocks(&blocks, budget, &indent)?;
    let new_inner = format!("{}\n{}", rendered, indent);
    let rebuilt = format!("{}{}{}{}", prefix, quote, new_inner, quote);
    if rebuilt == text {
        return None;
    }
    Some(rebuilt)
}

#[derive(Debug, Clone)]
enum Block {
    /// Wrapping paragraph.
    Paragraph(String),
    /// Blank line(s) between blocks.
    Blank,
    /// Preserve as-is. Already dedented (no common indent).
    Verbatim(String),
}

fn parse_blocks(body: &str, _indent: &str) -> Vec<Block> {
    // Determine common indent from non-blank lines except possibly the very
    // first line (PEP 257: """summary on first line, body indented""").
    let raw_lines: Vec<&str> = body.split('\n').collect();
    let mut common: Option<usize> = None;
    for (i, line) in raw_lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if i == 0 {
            // The first line shares indentation with the """ on the same
            // source line, so its leading spaces are absent in `inner`.
            continue;
        }
        let lead: usize = line.chars().take_while(|c| *c == ' ').count();
        common = Some(common.map(|c| c.min(lead)).unwrap_or(lead));
    }
    let common = common.unwrap_or(0);

    let dedent = |line: &str, is_first: bool| -> String {
        if is_first || line.chars().count() < common {
            line.to_string()
        } else {
            line[common..].to_string()
        }
    };

    let mut blocks: Vec<Block> = Vec::new();
    let mut current_para: Vec<String> = Vec::new();
    let mut in_fenced = false;

    let push_para = |para: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !para.is_empty() {
            blocks.push(Block::Paragraph(para.join(" ")));
            para.clear();
        }
    };

    for (i, raw) in raw_lines.iter().enumerate() {
        let line = dedent(raw, i == 0);
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            push_para(&mut current_para, &mut blocks);
            blocks.push(Block::Verbatim(line.clone()));
            in_fenced = !in_fenced;
            continue;
        }
        if in_fenced {
            blocks.push(Block::Verbatim(line));
            continue;
        }

        if line.trim().is_empty() {
            push_para(&mut current_para, &mut blocks);
            if !matches!(blocks.last(), Some(Block::Blank)) {
                blocks.push(Block::Blank);
            }
            continue;
        }

        let leading = line.chars().take_while(|c| *c == ' ').count();
        // Indented code block (>= 4 leading spaces, not a list).
        if leading >= 4 && !is_listish(&line) {
            push_para(&mut current_para, &mut blocks);
            blocks.push(Block::Verbatim(line));
            continue;
        }

        // Section headers, list items, sphinx fields: keep as their own line.
        if is_section_header(&line) || is_listish(&line) || is_sphinx_field(&line) {
            push_para(&mut current_para, &mut blocks);
            blocks.push(Block::Verbatim(line));
            continue;
        }

        // NumPy-style underline ("-----" right after a heading word) is
        // preserved as verbatim too; detected as section header above.

        current_para.push(line.trim().to_string());
    }
    push_para(&mut current_para, &mut blocks);

    // Trim trailing Blank.
    while matches!(blocks.last(), Some(Block::Blank)) {
        blocks.pop();
    }
    blocks
}

fn render_blocks(blocks: &[Block], budget: usize, indent: &str) -> Option<String> {
    let mut out = String::new();
    let mut first_emitted = false;
    let mut just_emitted_blank = false;

    let push_indent = |out: &mut String| {
        out.push('\n');
        out.push_str(indent);
    };

    for block in blocks {
        match block {
            Block::Blank => {
                if first_emitted && !just_emitted_blank {
                    out.push('\n');
                    just_emitted_blank = true;
                }
            }
            Block::Paragraph(p) => {
                let wrapped = textwrap::fill(p, budget);
                if !first_emitted {
                    let mut lines = wrapped.split('\n');
                    if let Some(first) = lines.next() {
                        out.push_str(first);
                    }
                    for line in lines {
                        push_indent(&mut out);
                        out.push_str(line);
                    }
                    first_emitted = true;
                } else {
                    push_indent(&mut out);
                    let mut lines = wrapped.split('\n');
                    if let Some(first) = lines.next() {
                        out.push_str(first);
                    }
                    for line in lines {
                        push_indent(&mut out);
                        out.push_str(line);
                    }
                }
                just_emitted_blank = false;
            }
            Block::Verbatim(line) => {
                if !first_emitted {
                    out.push_str(line);
                    first_emitted = true;
                } else {
                    push_indent(&mut out);
                    out.push_str(line);
                }
                just_emitted_blank = false;
            }
        }
    }
    if !first_emitted {
        return None;
    }
    Some(out)
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

fn is_listish(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    let mut chars = t.chars();
    let mut digits = 0usize;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            digits += 1;
        } else if digits > 0 && (c == '.' || c == ')') {
            return chars.next() == Some(' ');
        } else {
            break;
        }
    }
    false
}

fn is_section_header(line: &str) -> bool {
    let t = line.trim();
    const GOOGLE: &[&str] = &[
        "Args:",
        "Arguments:",
        "Attributes:",
        "Example:",
        "Examples:",
        "Note:",
        "Notes:",
        "Raises:",
        "Returns:",
        "Yields:",
        "Warns:",
        "Warning:",
        "See Also:",
        "References:",
        "Todo:",
    ];
    if GOOGLE.contains(&t) {
        return true;
    }
    if !t.is_empty() && t.len() >= 3 && t.chars().all(|c| matches!(c, '-' | '=' | '~')) {
        return true;
    }
    false
}

fn is_sphinx_field(line: &str) -> bool {
    let t = line.trim_start();
    if !t.starts_with(':') {
        return false;
    }
    // Must have a closing colon on the same line.
    t[1..].find(':').is_some()
}

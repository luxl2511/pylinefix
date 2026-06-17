use crate::edits::EditSet;
use crate::pyutil;
use crate::Options;
use tree_sitter::{Node, Tree};

/// Collect edits for comments. Two operations:
///   1. Trailing/inline comments on overlong lines are hoisted to a line above
///      the code (preserving indent). A second format pass then wraps the new
///      full-line comment if it's still too long.
///   2. Overlong full-line `#` prose comments are wrapped at word boundaries.
///
/// Commented-out code and lint/type directives are preserved verbatim in both
/// modes.
pub fn collect_edits(tree: &Tree, source: &str, opts: &Options, edits: &mut EditSet) {
    pyutil::walk(tree.root_node(), |node| {
        if node.kind() != "comment" {
            return;
        }
        if let Some((s, e, r)) = hoist_trailing_comment(node, source, opts) {
            edits.push(s, e, r);
            return;
        }
        if let Some(rep) = wrap_comment(node, source, opts) {
            edits.push(node.start_byte(), node.end_byte(), rep);
        }
    });
}

/// If `node` is an inline trailing comment on an overlong line, return an
/// edit that rewrites the whole line as `<indent># comment\n<code>`.
fn hoist_trailing_comment(
    node: Node,
    source: &str,
    opts: &Options,
) -> Option<(usize, usize, String)> {
    let line_start = pyutil::line_start(source, node.start_byte());
    let leading = &source[line_start..node.start_byte()];
    // Trailing comments have non-whitespace before the `#`.
    if leading.chars().all(|c| c == ' ' || c == '\t') {
        return None;
    }

    let line_end = pyutil::line_end(source, node.start_byte());
    let line = &source[line_start..line_end];
    if pyutil::longest_line(line, opts.tab_width) <= opts.line_length {
        return None;
    }

    let text = pyutil::node_text(node, source);
    if !text.starts_with('#') {
        return None;
    }

    // Lint/type directives must stay attached to their code line — moving
    // them above breaks `# noqa`/`# type: ignore` semantics.
    let (hashes, body) = split_hashes(text);
    let body_trimmed = body.trim_start();
    if is_directive(body_trimmed) {
        return None;
    }

    let indent: String = leading
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();

    // Strip trailing whitespace between the code and the `#`.
    let code_with_ws = &source[line_start..node.start_byte()];
    let code_part = code_with_ws.trim_end_matches(|c: char| c == ' ' || c == '\t');
    if code_part.trim().is_empty() {
        return None;
    }

    let hoisted = if body_trimmed.is_empty() {
        format!("{}{}", indent, hashes)
    } else {
        format!("{}{} {}", indent, hashes, body_trimmed)
    };
    let replacement = format!("{}\n{}", hoisted, code_part);
    Some((line_start, line_end, replacement))
}

fn wrap_comment(node: Node, source: &str, opts: &Options) -> Option<String> {
    // Must be the first non-whitespace token on its line.
    let line_start = pyutil::line_start(source, node.start_byte());
    let leading = &source[line_start..node.start_byte()];
    if !leading.chars().all(|c| c == ' ' || c == '\t') {
        return None;
    }

    let text = pyutil::node_text(node, source);
    if !text.starts_with('#') {
        return None;
    }

    let line_end = pyutil::line_end(source, node.start_byte());
    let line_width = pyutil::longest_line(&source[line_start..line_end], opts.tab_width);
    if line_width <= opts.line_length {
        return None;
    }

    // Strip leading `#`s; preserve whatever was there (`#`, `##`, `#!`, etc.).
    let (hashes, body) = split_hashes(text);
    let stripped_body = body.trim_start_matches(' ');

    // Preserve special directive comments verbatim.
    if is_directive(stripped_body) {
        return None;
    }

    let indent = leading.to_string();
    let prefix = format!("{}{} ", indent, hashes);
    let budget = opts
        .line_length
        .saturating_sub(prefix.chars().count())
        .max(20);

    let wrapped = textwrap::fill(stripped_body, budget);
    let mut out = String::new();
    for (i, line) in wrapped.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(&indent);
        }
        out.push_str(hashes);
        if !line.is_empty() {
            out.push(' ');
            out.push_str(line);
        }
    }
    if out == text {
        return None;
    }
    Some(out)
}

fn split_hashes(text: &str) -> (&str, &str) {
    let mut idx = 0usize;
    let bytes = text.as_bytes();
    while idx < bytes.len() && bytes[idx] == b'#' {
        idx += 1;
    }
    (&text[..idx], &text[idx..])
}

fn is_directive(s: &str) -> bool {
    let s = s.trim_start();
    // Lint suppressions and type comments. Match `noqa` only with `:` or
    // end-of-comment to avoid false positives on prose starting with "noqa".
    if let Some(rest) = s.strip_prefix("noqa") {
        if rest.is_empty() || rest.starts_with(':') {
            return true;
        }
    }
    if let Some(rest) = s.strip_prefix("nosec") {
        if rest.is_empty() || rest.starts_with(':') || rest.starts_with(' ') {
            return true;
        }
    }
    s.starts_with("type:")
        || s.starts_with("type :")
        || s.starts_with("pragma:")
        || s.starts_with("pylint:")
        || s.starts_with("pyright:")
        || s.starts_with("mypy:")
        || s.starts_with("ruff:")
        || s.starts_with("fmt:")
        || s.starts_with("isort:")
        || s.starts_with("flake8:")
        || s.starts_with("yapf:")
        || s.starts_with("!")
        || s.starts_with("-*-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directive_detected() {
        assert!(is_directive("noqa: E501"));
        assert!(is_directive("type: ignore"));
        assert!(!is_directive("noqa style commentary"));
    }
}

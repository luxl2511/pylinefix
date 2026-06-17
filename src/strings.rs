use crate::edits::EditSet;
use crate::pyutil;
use crate::Options;
use tree_sitter::{Node, Tree};

/// Collect edits that split overlong single-line string literals into
/// implicit-concatenation chunks at word boundaries.
pub fn collect_edits(tree: &Tree, source: &str, opts: &Options, edits: &mut EditSet) {
    pyutil::walk(tree.root_node(), |node| {
        if node.kind() != "string" {
            return;
        }
        if let Some(edit) = consider_string(node, source, opts) {
            edits.push(edit.0, edit.1, edit.2);
        }
    });
}

fn consider_string(node: Node, source: &str, opts: &Options) -> Option<(usize, usize, String)> {
    if pyutil::is_inside_typing_literal(node, source) {
        return None;
    }
    let parsed = ParsedString::from_node(node, source)?;
    if parsed.is_triple_quoted {
        return None;
    }
    if parsed.is_bytes {
        return None;
    }
    if !pyutil::any_line_over(
        source,
        node.start_byte(),
        node.end_byte(),
        opts.line_length,
        opts.tab_width,
    ) {
        return None;
    }
    if parsed.is_fstring && !fstring_safely_splittable(source, node) {
        return None;
    }

    let line_indent = pyutil::indent_of_line(source, node.start_byte()).to_string();
    let needs_parens = !context_provides_parens(node);
    let inner_indent = if needs_parens {
        format!("{line_indent}    ")
    } else {
        line_indent.clone()
    };

    // Budget for the inner string content per line: line_length minus
    // (indent + prefix + 2*quote chars).
    let overhead =
        inner_indent.chars().count() + parsed.prefix.chars().count() + 2 * parsed.quote.chars().count();
    if overhead + 16 > opts.line_length {
        return None;
    }
    let target = opts.line_length - overhead;

    let chunks = pack_tokens(tokenize(parsed.inner), target);
    if chunks.is_empty() {
        return None;
    }
    // For the no-parens path (e.g. inside a function call) we only emit a
    // multi-line layout when there are at least 2 chunks. With 1 chunk it
    // would be a no-op replacement of the string with itself.
    if !needs_parens && chunks.len() < 2 {
        return None;
    }

    let body_lines: Vec<String> = chunks
        .iter()
        .map(|c| {
            format!(
                "{}{}{}{}{}",
                inner_indent,
                parsed.prefix,
                parsed.quote,
                c,
                parsed.quote
            )
        })
        .collect();
    let body = body_lines.join("\n");

    let replacement = if needs_parens {
        format!("(\n{}\n{})", body, line_indent)
    } else {
        // Strip the leading indent of the first line — the string node already
        // sits at that column in the original source.
        let mut iter = body.splitn(2, '\n');
        let first = iter.next().unwrap_or("").trim_start_matches(' ');
        match iter.next() {
            Some(rest) => format!("{}\n{}", first, rest),
            None => first.to_string(),
        }
    };

    if replacement == source[node.byte_range()] {
        return None;
    }
    Some((node.start_byte(), node.end_byte(), replacement))
}

/// True if the immediate enclosing brackets already permit implicit
/// continuation across lines without adding extra parens. Conservatively this
/// is call-arg lists, parenthesized expressions, list/tuple/set literals, and
/// generator/comprehension forms. Dict literals and `pair` are deliberately
/// excluded — by convention, splitting a dict value uses an explicit `(...)`
/// to make grouping obvious.
fn context_provides_parens(node: Node) -> bool {
    let mut cur = node.parent();
    while let Some(n) = cur {
        match n.kind() {
            "argument_list"
            | "parenthesized_expression"
            | "list"
            | "tuple"
            | "set"
            | "generator_expression"
            | "list_comprehension"
            | "set_comprehension" => return true,
            "pair" | "dictionary" | "dictionary_comprehension" | "assignment"
            | "expression_statement" | "return_statement" | "yield" => return false,
            _ => cur = n.parent(),
        }
    }
    false
}

#[derive(Debug)]
struct ParsedString<'a> {
    prefix: String,
    quote: &'a str,
    inner: &'a str,
    is_triple_quoted: bool,
    is_bytes: bool,
    is_fstring: bool,
}

impl<'a> ParsedString<'a> {
    fn from_node(node: Node, source: &'a str) -> Option<Self> {
        let text = &source[node.byte_range()];
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
        let prefix = text[..idx].to_lowercase();
        let rest = &text[idx..];
        let quote = if rest.starts_with("\"\"\"") {
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
        let is_triple_quoted = quote.len() == 3;
        if !rest.ends_with(quote) || rest.len() < 2 * quote.len() {
            return None;
        }
        let inner = &rest[quote.len()..rest.len() - quote.len()];
        Some(Self {
            is_bytes: prefix.contains('b'),
            is_fstring: prefix.contains('f'),
            prefix,
            quote,
            inner,
            is_triple_quoted,
        })
    }
}

/// Tokenize so each token absorbs its trailing space. Concatenating tokens
/// reproduces the input byte-for-byte. Adjacent runs of internal whitespace
/// are kept intact (tokens may contain them).
fn tokenize(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut buf = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ' ' {
            // Trailing space attaches to the current token if there is one;
            // otherwise it forms its own (rare) leading-space token.
            buf.push(c);
            if chars.peek().is_none_or(|&n| n != ' ')
                && !buf.trim_end_matches(' ').is_empty() {
                    out.push(std::mem::take(&mut buf));
                }
        } else {
            buf.push(c);
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

/// Greedy first-fit packer that preserves token concatenation.
fn pack_tokens(tokens: Vec<String>, target: usize) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for tok in tokens {
        if cur.is_empty() {
            cur = tok;
            continue;
        }
        let combined = cur.chars().count() + tok.chars().count();
        if combined <= target {
            cur.push_str(&tok);
        } else {
            chunks.push(std::mem::take(&mut cur));
            cur = tok;
        }
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    chunks
}

/// f-string is safe to split if every interpolation `{...}` is fully contained
/// within a single token (i.e. has no spaces inside).
fn fstring_safely_splittable(source: &str, node: Node) -> bool {
    let text = &source[node.byte_range()];
    let mut depth = 0i32;
    let mut iter = text.chars().peekable();
    while let Some(ch) = iter.next() {
        match ch {
            '{' => {
                if iter.peek() == Some(&'{') {
                    iter.next();
                    continue;
                }
                depth += 1;
            }
            '}' => {
                if iter.peek() == Some(&'}') {
                    iter.next();
                    continue;
                }
                depth -= 1;
            }
            ' ' if depth > 0 => return false,
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_round_trip() {
        let s = "Free-form description. Combined with German Name.";
        let toks = tokenize(s);
        assert_eq!(toks.concat(), s);
    }

    #[test]
    fn pack_preserves_text() {
        let s = "alpha beta gamma delta epsilon zeta eta theta";
        let toks = tokenize(s);
        let chunks = pack_tokens(toks, 16);
        assert_eq!(chunks.concat(), s);
        assert!(chunks.len() > 1);
    }
}

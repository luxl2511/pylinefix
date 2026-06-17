use tree_sitter::Node;

/// Byte offset of the start of the line containing `byte`.
pub fn line_start(source: &str, byte: usize) -> usize {
    source[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

/// Visible column (in characters) of `byte` within its line.
/// Tabs expand to `tab_width` columns.
pub fn visible_column(source: &str, byte: usize, tab_width: usize) -> usize {
    let line_begin = line_start(source, byte);
    let prefix = &source[line_begin..byte];
    let mut col = 0usize;
    for ch in prefix.chars() {
        if ch == '\t' {
            col += tab_width - (col % tab_width);
        } else {
            col += 1;
        }
    }
    col
}

/// Byte offset of the end of the line containing `byte` (excluding the
/// trailing newline; or end-of-source).
pub fn line_end(source: &str, byte: usize) -> usize {
    source[byte..]
        .find('\n')
        .map(|i| byte + i)
        .unwrap_or(source.len())
}

/// Indentation (whitespace prefix) of the line containing `byte`.
pub fn indent_of_line(source: &str, byte: usize) -> &str {
    let start = line_start(source, byte);
    let bytes = source.as_bytes();
    let mut end = start;
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    &source[start..end]
}

/// Length in display columns of the longest line in `text`.
pub fn longest_line(text: &str, tab_width: usize) -> usize {
    let mut max = 0usize;
    for line in text.split('\n') {
        let mut col = 0usize;
        for ch in line.chars() {
            if ch == '\t' {
                col += tab_width - (col % tab_width);
            } else {
                col += 1;
            }
        }
        if col > max {
            max = col;
        }
    }
    max
}

/// True if any line in `range` of `source` exceeds `limit` display columns.
pub fn any_line_over(source: &str, start: usize, end: usize, limit: usize, tab_width: usize) -> bool {
    let segment = &source[line_start(source, start)..end];
    longest_line(segment, tab_width) > limit
}

/// Walk the syntax tree, calling `visitor` on every node in pre-order.
pub fn walk<'a, F: FnMut(Node<'a>)>(root: Node<'a>, mut visitor: F) {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        visitor(node);
        for child in node.children(&mut cursor).collect::<Vec<_>>().into_iter().rev() {
            stack.push(child);
        }
    }
}

/// Whether a node is enclosed by an existing parenthesized expression
/// (so we don't need to add parentheses when splitting a string).
pub fn is_inside_parenthesized(node: Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    matches!(
        parent.kind(),
        "parenthesized_expression"
            | "tuple"
            | "list"
            | "set"
            | "dictionary"
            | "argument_list"
            | "parameters"
            | "subscript"
            | "generator_expression"
            | "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "pair"
    ) || is_inside_parenthesized(parent)
}

/// True if any ancestor is a `subscript` whose value is the name `Literal`
/// (typing.Literal[...] values must not be edited).
pub fn is_inside_typing_literal(node: Node, source: &str) -> bool {
    let mut cur = node.parent();
    while let Some(n) = cur {
        if n.kind() == "subscript"
            && let Some(value) = n.child_by_field_name("value") {
                let text = node_text(value, source);
                if text == "Literal" || text.ends_with(".Literal") {
                    return true;
                }
            }
        cur = n.parent();
    }
    false
}

pub fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

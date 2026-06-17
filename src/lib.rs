pub mod cli;
pub mod comments;
pub mod config;
pub mod docstrings;
pub mod edits;
pub mod embedded;
pub mod parse;
pub mod pyutil;
pub mod strings;

use anyhow::Result;

#[derive(Clone, Debug)]
pub struct Options {
    pub line_length: usize,
    pub tab_width: usize,
    pub embedded_enabled: bool,
    pub docstrings_enabled: bool,
    pub strings_enabled: bool,
    pub comments_enabled: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            line_length: 88,
            tab_width: 4,
            embedded_enabled: true,
            docstrings_enabled: true,
            strings_enabled: true,
            comments_enabled: true,
        }
    }
}

/// Format a Python source string until fixed point (idempotent).
pub fn format_source(source: &str, opts: &Options) -> Result<String> {
    let first = format_pass(source, opts)?;
    if first == source {
        return Ok(first);
    }
    let second = format_pass(&first, opts)?;
    Ok(second)
}

fn format_pass(source: &str, opts: &Options) -> Result<String> {
    let tree = parse::parse_python(source)?;
    let mut edits = edits::EditSet::new();
    if opts.strings_enabled {
        strings::collect_edits(&tree, source, opts, &mut edits);
    }
    if opts.docstrings_enabled {
        docstrings::collect_edits(&tree, source, opts, &mut edits);
    }
    if opts.embedded_enabled {
        embedded::collect_edits(&tree, source, opts, &mut edits);
    }
    if opts.comments_enabled {
        comments::collect_edits(&tree, source, opts, &mut edits);
    }
    Ok(edits.apply(source))
}

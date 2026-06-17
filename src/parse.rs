use anyhow::{anyhow, Result};
use tree_sitter::{Parser, Tree};

pub fn parse_python(source: &str) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| anyhow!("set tree-sitter-python language: {e}"))?;
    parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("tree-sitter parse returned None"))
}

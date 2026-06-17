use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
struct PyProject {
    #[serde(default)]
    tool: Tool,
}

#[derive(Deserialize, Default)]
struct Tool {
    ruff: Option<RuffSection>,
    black: Option<Section>,
}

#[derive(Deserialize, Default)]
struct RuffSection {
    #[serde(rename = "line-length")]
    line_length: Option<usize>,
    #[serde(rename = "line_length")]
    line_length_alt: Option<usize>,
    format: Option<Section>,
    lint: Option<Section>,
}

#[derive(Deserialize, Default)]
struct Section {
    #[serde(rename = "line-length")]
    line_length: Option<usize>,
    #[serde(rename = "line_length")]
    line_length_alt: Option<usize>,
}

/// Find the line-length setting in the nearest pyproject.toml.
/// Walks up from the first input path until one is found.
///
/// Lookup order within pyproject.toml:
///   1. `[tool.ruff].line-length`
///   2. `[tool.ruff.format].line-length`
///   3. `[tool.ruff.lint].line-length`
///   4. `[tool.black].line-length`
pub fn find_line_length(paths: &[PathBuf]) -> Result<Option<usize>> {
    let start = match paths.first() {
        Some(p) => p.canonicalize().unwrap_or_else(|_| p.clone()),
        None => return Ok(None),
    };
    let start_dir = if start.is_file() {
        start.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        start
    };
    let Some(toml_path) = walk_up_for(&start_dir, "pyproject.toml") else {
        return Ok(None);
    };
    let raw = std::fs::read_to_string(&toml_path)
        .with_context(|| format!("read {}", toml_path.display()))?;
    let parsed: PyProject = toml::from_str(&raw)
        .with_context(|| format!("parse {}", toml_path.display()))?;

    let ll = parsed
        .tool
        .ruff
        .as_ref()
        .and_then(|r| {
            r.line_length
                .or(r.line_length_alt)
                .or_else(|| {
                    r.format
                        .as_ref()
                        .and_then(|s| s.line_length.or(s.line_length_alt))
                })
                .or_else(|| {
                    r.lint
                        .as_ref()
                        .and_then(|s| s.line_length.or(s.line_length_alt))
                })
        })
        .or_else(|| {
            parsed
                .tool
                .black
                .as_ref()
                .and_then(|s| s.line_length.or(s.line_length_alt))
        });
    Ok(ll)
}

fn walk_up_for(start: &Path, name: &str) -> Option<PathBuf> {
    let mut cur: &Path = start;
    loop {
        let candidate = cur.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        cur = cur.parent()?;
    }
}

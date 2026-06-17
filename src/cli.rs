use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "pylinefix",
    version,
    about = "Fix line-length issues Ruff leaves alone (long strings, docstrings, embedded SQL/JSON/markdown)."
)]
pub struct Args {
    /// Files or directories to format. Directories are walked respecting .gitignore.
    pub paths: Vec<PathBuf>,

    /// Override line length. Default: read from pyproject.toml or 88.
    #[arg(long)]
    pub line_length: Option<usize>,

    /// In stdin mode, the original path of the buffer being formatted. Used
    /// to locate the project's pyproject.toml. Mirrors `ruff --stdin-filename`.
    #[arg(long, value_name = "PATH")]
    pub stdin_filename: Option<PathBuf>,

    /// Don't write changes; exit code 1 if any file would change.
    #[arg(long)]
    pub check: bool,

    /// Print summary to stderr.
    #[arg(short, long)]
    pub verbose: bool,

    /// Disable string-literal splitting.
    #[arg(long)]
    pub no_strings: bool,

    /// Disable docstring wrapping.
    #[arg(long)]
    pub no_docstrings: bool,

    /// Disable embedded-language formatting.
    #[arg(long)]
    pub no_embedded: bool,

    /// Disable comment wrapping.
    #[arg(long)]
    pub no_comments: bool,
}

pub fn parse() -> Args {
    Args::parse()
}

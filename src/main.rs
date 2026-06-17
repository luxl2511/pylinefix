use anyhow::{Context, Result};
use pylinefix::{cli, config, format_source, Options};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("pylinefix: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let args = cli::parse();

    let mut opts = Options::default();
    if let Some(ll) = args.line_length {
        opts.line_length = ll;
    } else {
        // Resolve line-length from pyproject.toml. Lookup origin order:
        //   1. --stdin-filename if given (matches ruff's behavior)
        //   2. CWD when reading from stdin without a filename hint
        //   3. The first input path in file mode
        let stdin_mode = args.paths.len() == 1 && args.paths[0].as_os_str() == "-";
        let lookup_paths: Vec<PathBuf> = if stdin_mode {
            if let Some(ref f) = args.stdin_filename {
                vec![f.clone()]
            } else {
                vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
            }
        } else {
            args.paths.clone()
        };
        if let Some(ll) = config::find_line_length(&lookup_paths)? {
            opts.line_length = ll;
        }
    }
    opts.embedded_enabled = !args.no_embedded;
    opts.docstrings_enabled = !args.no_docstrings;
    opts.strings_enabled = !args.no_strings;
    opts.comments_enabled = !args.no_comments;

    // Stdin/stdout mode: a single "-" path reads source from stdin and writes
    // the formatted result to stdout. Used by editor formatter integrations
    // (conform.nvim, none-ls, etc.).
    if args.paths.len() == 1 && args.paths[0].as_os_str() == "-" {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .context("read stdin")?;
        let formatted = format_source(&source, &opts)?;
        std::io::stdout()
            .write_all(formatted.as_bytes())
            .context("write stdout")?;
        return Ok(ExitCode::SUCCESS);
    }

    let files = collect_files(&args.paths)?;
    let mut changed = 0usize;
    let mut errors = 0usize;

    for file in &files {
        match process_file(file, &opts, args.check) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(err) => {
                errors += 1;
                eprintln!("pylinefix: {}: {err:#}", file.display());
            }
        }
    }

    if args.verbose {
        eprintln!(
            "pylinefix: {} file(s) scanned, {} changed, {} error(s)",
            files.len(),
            changed,
            errors,
        );
    }

    if errors > 0 {
        return Ok(ExitCode::from(2));
    }
    if args.check && changed > 0 {
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

fn process_file(path: &PathBuf, opts: &Options, check: bool) -> Result<bool> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let formatted = format_source(&source, opts)?;
    if formatted == source {
        return Ok(false);
    }
    if !check {
        std::fs::write(path, &formatted)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(true)
}

fn collect_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    use ignore::WalkBuilder;
    let mut out = Vec::new();
    if paths.is_empty() {
        return Ok(out);
    }
    let mut builder = WalkBuilder::new(&paths[0]);
    for p in &paths[1..] {
        builder.add(p);
    }
    builder.standard_filters(true).follow_links(false);
    for entry in builder.build() {
        let entry = entry?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if entry.path().extension().is_some_and(|e| e == "py") {
            out.push(entry.path().to_path_buf());
        }
    }
    Ok(out)
}

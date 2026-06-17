# pylinefix

Fix the line-length issues Ruff leaves alone: long string literals, docstrings, and embedded SQL/JSON/markdown.

Ruff's formatter reflows code but leaves overlong strings and docstrings untouched. `pylinefix` handles those cases: it splits long string literals, wraps docstrings, and reformats embedded languages so the whole file fits the line limit.

## Install

From source (requires Rust 1.94+):

```
cargo install --git https://github.com/luxl2511/pylinefix
```

Or grab a prebuilt binary from the [releases page](https://github.com/luxl2511/pylinefix/releases).

## Usage

```
pylinefix path/to/file.py            # format in place
pylinefix src/                       # walk a directory, respecting .gitignore
pylinefix --check src/               # exit 1 if anything would change, write nothing
cat file.py | pylinefix --stdin-filename file.py
```

Line length is read from `pyproject.toml` (falling back to 88), or set it with `--line-length`.

### Options

| Flag | Effect |
| --- | --- |
| `--line-length N` | Override the line length. |
| `--check` | Don't write; exit 1 if any file would change. |
| `--stdin-filename PATH` | Original path of a stdin buffer, used to locate `pyproject.toml`. |
| `--no-strings` | Disable string-literal splitting. |
| `--no-docstrings` | Disable docstring wrapping. |
| `--no-embedded` | Disable embedded-language formatting. |
| `--no-comments` | Disable comment reflowing. |
| `-v`, `--verbose` | Print a summary to stderr. |

## Run after Ruff

`pylinefix` is meant to run as a second pass after Ruff:

```
ruff format .
pylinefix .
```

## License

MIT

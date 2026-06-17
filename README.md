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

## LazyVim / Neovim

Copy [`pylinefix.lua`](pylinefix.lua) into your LazyVim plugins folder and restart:

```
~/.config/nvim/lua/plugins/pylinefix.lua
```

That's the whole install. Like any other LazyVim plugin: drop it in, restart, nothing to configure. It loads on python files, builds the binary with `cargo build --release`, and registers itself so it runs on save after whatever you already use (ruff, black, isort, ...). It appends, it does not replace them.

The file itself:

```lua
return {
  "luxl2511/pylinefix",
  build = "cargo build --release",
  ft = "python",
  dependencies = { "stevearc/conform.nvim" },
  config = function()
    local conform = require("conform")
    conform.formatters.pylinefix = require("pylinefix").formatter()

    local py = conform.formatters_by_ft.python
    if py == nil then
      conform.formatters_by_ft.python = { "pylinefix" }
    elseif type(py) == "table" and not vim.tbl_contains(py, "pylinefix") then
      table.insert(py, "pylinefix")
    end
  end,
}
```

pylinefix formats over stdin, so it works through [conform.nvim](https://github.com/stevearc/conform.nvim), the default formatter engine in LazyVim. If `pylinefix` is on your `PATH` it's used directly; otherwise the plugin uses the binary it built in its own directory, so a Rust toolchain at install time is the only requirement.

## Run after Ruff

`pylinefix` is meant to run as a second pass after Ruff:

```
ruff format .
pylinefix .
```

## License

MIT

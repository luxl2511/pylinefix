-- Drop this file into your LazyVim plugins folder as
-- ~/.config/nvim/lua/plugins/pylinefix.lua and restart. Nothing else to edit.
--
-- It installs pylinefix, builds the binary, and appends it to your python
-- formatters so it runs on save after ruff/black/isort/whatever you already use.
return {
  "stevearc/conform.nvim",
  dependencies = {
    { "luxl2511/pylinefix", build = "cargo build --release" },
  },
  opts = function(_, opts)
    opts.formatters = opts.formatters or {}
    opts.formatters.pylinefix = require("pylinefix").formatter()

    opts.formatters_by_ft = opts.formatters_by_ft or {}
    local py = opts.formatters_by_ft.python or {}
    table.insert(py, "pylinefix")
    opts.formatters_by_ft.python = py
  end,
}

-- Drop this file into your LazyVim plugins folder as
-- ~/.config/nvim/lua/plugins/pylinefix.lua and restart. Nothing to configure.
--
-- Loads on python files, builds the binary, and registers itself with conform
-- so it runs on save after ruff/black/isort or whatever you already use.
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

local M = {}

-- NOTE: prefer a pylinefix on PATH, otherwise fall back to the binary lazy.nvim
-- built inside this plugin dir via the `build = "cargo build --release"` step.
function M.bin()
  if vim.fn.executable("pylinefix") == 1 then
    return "pylinefix"
  end
  local root = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":h:h:h")
  local built = root .. "/target/release/pylinefix"
  if vim.fn.executable(built) == 1 then
    return built
  end
  return "pylinefix"
end

function M.formatter()
  return {
    command = M.bin(),
    args = { "--stdin-filename", "$FILENAME", "-" },
    stdin = true,
  }
end

-- Register pylinefix with conform.nvim and append it to the given filetypes so
-- it runs after whatever formatter already handles them (ruff for python).
function M.setup(opts)
  opts = vim.tbl_deep_extend("force", { filetypes = { "python" } }, opts or {})
  local conform = require("conform")
  conform.formatters.pylinefix = vim.tbl_deep_extend("force", M.formatter(), opts.formatter or {})
  for _, ft in ipairs(opts.filetypes) do
    local list = conform.formatters_by_ft[ft] or {}
    table.insert(list, "pylinefix")
    conform.formatters_by_ft[ft] = list
  end
end

return M

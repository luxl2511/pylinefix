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

return M

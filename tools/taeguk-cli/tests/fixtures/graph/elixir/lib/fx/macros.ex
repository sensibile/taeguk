defmodule Fx.Macros do
  defmacro twice(x), do: quote(do: unquote(x) * 2)
end

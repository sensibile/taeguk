defmodule Fx.Runner13 do
  import Function, only: [capture: 3]
  def load(m), do: capture(m, :run, 0)
end

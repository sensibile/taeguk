defmodule Fx.Runner8 do
  def run(m), do: :"Elixir.Kernel".apply(m, :run, [])
end

defmodule Fx.Runner10 do
  alias :"Elixir.Kernel", as: K
  def run(m), do: K.apply(m, :run, [])
end

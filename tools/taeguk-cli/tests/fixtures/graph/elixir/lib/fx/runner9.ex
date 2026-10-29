defmodule Fx.Runner9 do
  alias Kernel, as: K
  def run(m), do: K.apply(m, :run, [])
  alias Enum, as: K
  def none, do: K.count([])
end

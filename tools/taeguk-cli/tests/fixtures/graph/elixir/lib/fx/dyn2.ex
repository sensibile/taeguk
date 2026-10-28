defmodule Fx.Dyn2 do
  def call(m), do: apply m, :run, []
end

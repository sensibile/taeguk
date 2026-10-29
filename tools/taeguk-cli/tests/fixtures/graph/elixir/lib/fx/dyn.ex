defmodule Fx.Dyn do
  def call(module), do: apply(module, :run, [])
end

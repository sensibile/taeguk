defmodule Fx.B do
  require Fx.Macros

  def go, do: Fx.Macros.twice(1)
end

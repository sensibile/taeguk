defmodule Fx.Runner11 do
  def run(m) do
    f = &apply/3
    f.(m, :run, [])
  end
end

defmodule Fx.Runner7 do
  alias Kernel, as: K

  def run(target), do: K.apply(target, :run, [])
end

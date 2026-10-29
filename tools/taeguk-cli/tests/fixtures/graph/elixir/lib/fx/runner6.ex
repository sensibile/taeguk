defmodule Fx.Runner6 do
  def run(mod) do
    mod.
      execute()
  end

  @doc "Calls Fx.B.b. The value is returned."
  def ok, do: Fx.B.b()
end

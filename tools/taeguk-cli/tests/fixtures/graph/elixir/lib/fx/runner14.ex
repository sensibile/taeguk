defmodule Fx.Runner14 do
  def run(mods), do: Task.start(List.first(mods), :run, [])
end

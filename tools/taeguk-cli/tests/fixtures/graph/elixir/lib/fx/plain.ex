defmodule Fx.Plain do
  # Three runtime values passed to a call that does not take an MFA are ordinary data.
  def ask(server, request, timeout), do: GenServer.call(server, request, timeout)
end

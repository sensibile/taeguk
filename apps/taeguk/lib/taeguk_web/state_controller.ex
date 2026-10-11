defmodule TaegukWeb.StateController do
  use Phoenix.Controller, formats: [:json]
  alias Taeguk.State
  def index(conn, _), do: respond(conn, State.list())
  def show(conn, %{"id" => id}), do: respond(conn, State.get(id))
  def create(conn, _), do: respond(conn, State.create(conn.body_params))
  def command(conn, %{"id" => id}), do: respond(conn, State.command(id, conn.body_params))

  def history(conn, %{"id" => id} = params) do
    case Integer.parse(params["after_revision"] || "0") do
      {n, ""} when n >= 0 -> respond(conn, State.history(id, n))
      _ -> respond(conn, {:error, {:invalid, "invalid_cursor"}})
    end
  end

  defp respond(conn, {:ok, value}), do: json(conn, %{data: value})

  defp respond(conn, {:error, {kind, reason}}) do
    status = %{invalid: 422, conflict: 409, not_found: 404, unavailable: 503}[kind]
    conn |> put_status(status) |> json(%{error: reason})
  end
end

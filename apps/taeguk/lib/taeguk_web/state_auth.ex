defmodule TaegukWeb.StateAuth do
  import Plug.Conn
  def init(opts), do: opts

  def call(conn, _) do
    expected = Application.get_env(:taeguk, :state_token)
    supplied = get_req_header(conn, "x-taeguk-token")

    cond do
      not Application.get_env(:taeguk, :state_enabled) or not is_binary(expected) or
          byte_size(expected) < 24 ->
        reject(conn, 503, "state_not_configured")

      length(supplied) != 1 ->
        reject(conn, 401, "unauthorized")

      not Plug.Crypto.secure_compare(hd(supplied), expected) ->
        reject(conn, 401, "unauthorized")

      true ->
        conn
    end
  end

  defp reject(conn, status, reason) do
    conn
    |> put_resp_content_type("application/json")
    |> send_resp(status, Jason.encode!(%{error: reason}))
    |> halt()
  end
end

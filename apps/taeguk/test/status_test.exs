defmodule TaegukWeb.StatusTest do
  use ExUnit.Case, async: true
  import Plug.Test
  defp request(path), do: TaegukWeb.Endpoint.call(conn(:get, path), TaegukWeb.Endpoint.init([]))

  test "health is a response signal, not workflow proof" do
    conn = request("/api/health")
    assert conn.status == 200
    assert Jason.decode!(conn.resp_body)["scope"] == "process_response_only"
  end

  test "capabilities do not claim unimplemented authority" do
    payload = request("/api/capabilities").resp_body |> Jason.decode!()
    assert payload["deployment"] == false
    assert payload["persistence"] == Application.get_env(:taeguk, :state_enabled, false)
    assert payload["mcp"] == false
  end

  test "homepage describes bootstrap scope" do
    conn = request("/")
    assert conn.status == 200
    assert conn.resp_body =~ "<html lang=\"ko\">"
    assert conn.resp_body =~ "현재 작업 저장"
  end
end

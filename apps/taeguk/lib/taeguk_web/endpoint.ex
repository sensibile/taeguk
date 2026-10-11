defmodule TaegukWeb.Endpoint do
  use Phoenix.Endpoint, otp_app: :taeguk
  plug(Plug.RequestId)
  plug(TaegukWeb.Router)
end

defmodule TaegukWeb.Router do
  use Phoenix.Router

  pipeline :browser do
    plug(:accepts, ["html"])
    plug(:put_secure_browser_headers)
  end

  pipeline :api do
    plug(:accepts, ["json"])
  end

  scope "/" do
    pipe_through(:browser)
    get("/", TaegukWeb.StatusController, :index)
  end

  scope "/api" do
    pipe_through(:api)
    get("/health", TaegukWeb.StatusController, :health)
    get("/capabilities", TaegukWeb.StatusController, :capabilities)
  end
end

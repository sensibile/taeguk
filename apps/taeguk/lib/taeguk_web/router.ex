defmodule TaegukWeb.Router do
  use Phoenix.Router

  pipeline :browser do
    plug(:accepts, ["html"])
    plug(:put_secure_browser_headers)
  end

  pipeline :api do
    plug(:accepts, ["json"])
  end

  pipeline :state do
    plug(TaegukWeb.StateAuth)
    plug(Plug.Parsers, parsers: [:json], json_decoder: Jason, length: 1_048_576)
  end

  scope "/api/work-items" do
    pipe_through([:api, :state])
    get("/", TaegukWeb.StateController, :index)
    post("/", TaegukWeb.StateController, :create)
    get("/:id", TaegukWeb.StateController, :show)
    get("/:id/history", TaegukWeb.StateController, :history)
    post("/:id/commands", TaegukWeb.StateController, :command)
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

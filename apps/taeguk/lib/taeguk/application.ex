defmodule Taeguk.Application do
  use Application

  def start(_type, _args) do
    Supervisor.start_link([{Phoenix.PubSub, name: Taeguk.PubSub}, TaegukWeb.Endpoint],
      strategy: :one_for_one,
      name: Taeguk.Supervisor
    )
  end

  def config_change(changed, _new, removed),
    do: TaegukWeb.Endpoint.config_change(changed, removed)
end

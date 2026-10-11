defmodule Taeguk.Application do
  use Application

  def start(_type, _args) do
    repo = if Application.get_env(:taeguk, :state_enabled), do: [Taeguk.Repo], else: []

    Supervisor.start_link(repo ++ [{Phoenix.PubSub, name: Taeguk.PubSub}, TaegukWeb.Endpoint],
      strategy: :one_for_one,
      name: Taeguk.Supervisor
    )
  end

  def config_change(changed, _new, removed),
    do: TaegukWeb.Endpoint.config_change(changed, removed)
end

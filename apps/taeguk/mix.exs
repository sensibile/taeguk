defmodule Taeguk.MixProject do
  use Mix.Project

  def project,
    do: [
      app: :taeguk,
      version: "0.1.0",
      elixir: "~> 1.18",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]

  def application, do: [extra_applications: [:logger], mod: {Taeguk.Application, []}]
  defp deps, do: [{:phoenix, "~> 1.8.15"}, {:bandit, "~> 1.0"}, {:jason, "~> 1.4"}]
end

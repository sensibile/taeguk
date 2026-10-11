import Config

config :taeguk, TaegukWeb.Endpoint,
  adapter: Bandit.PhoenixAdapter,
  http: [ip: {127, 0, 0, 1}, port: 4000],
  url: [host: "localhost"],
  secret_key_base: String.duplicate("local-development-only-", 4),
  render_errors: [formats: [html: TaegukWeb.ErrorHTML, json: TaegukWeb.ErrorJSON], layout: false],
  pubsub_server: Taeguk.PubSub

config :phoenix, :json_library, Jason

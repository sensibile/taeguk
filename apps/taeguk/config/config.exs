import Config

config :taeguk, TaegukWeb.Endpoint,
  adapter: Bandit.PhoenixAdapter,
  http: [ip: {127, 0, 0, 1}, port: 4000],
  url: [host: "localhost"],
  secret_key_base: String.duplicate("local-development-only-", 4),
  render_errors: [formats: [html: TaegukWeb.ErrorHTML, json: TaegukWeb.ErrorJSON], layout: false],
  pubsub_server: Taeguk.PubSub

config :phoenix, :json_library, Jason

config :taeguk, ecto_repos: [Taeguk.Repo], state_enabled: false

config :taeguk, Taeguk.Repo,
  pool_size: 5,
  log: false,
  journal_mode: :wal,
  foreign_keys: :on,
  busy_timeout: 5000,
  synchronous: :full,
  default_transaction_mode: :immediate

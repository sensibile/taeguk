import Config

if config_env() == :prod do
  secret = System.fetch_env!("TAEGUK_SECRET_KEY_BASE")
  if byte_size(secret) < 64, do: raise("TAEGUK_SECRET_KEY_BASE requires at least 64 bytes")
  config :taeguk, TaegukWeb.Endpoint, secret_key_base: secret
end

if path = System.get_env("TAEGUK_DATABASE_PATH") do
  unless Path.type(path) == :absolute, do: raise("TAEGUK_DATABASE_PATH must be absolute")
  config :taeguk, :state_enabled, true
  config :taeguk, Taeguk.Repo, database: path
end

config :taeguk, :state_token, System.get_env("TAEGUK_API_TOKEN")

if port = System.get_env("TAEGUK_PORT") do
  port = String.to_integer(port)
  if port < 1 or port > 65535, do: raise("TAEGUK_PORT out of range")
  config :taeguk, TaegukWeb.Endpoint, server: true, http: [ip: {127, 0, 0, 1}, port: port]
end

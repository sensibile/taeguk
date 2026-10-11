import Config

if config_env() == :prod do
  secret = System.fetch_env!("TAEGUK_SECRET_KEY_BASE")
  if byte_size(secret) < 64, do: raise("TAEGUK_SECRET_KEY_BASE requires at least 64 bytes")
  config :taeguk, TaegukWeb.Endpoint, secret_key_base: secret
end

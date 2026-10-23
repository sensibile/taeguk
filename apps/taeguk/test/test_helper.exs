# Explicit DB runs must not report skipped state tests as a pass.
if System.get_env("TAEGUK_REQUIRE_STATE_TESTS") == "1" and
     not Application.get_env(:taeguk, :state_enabled, false),
   do: raise("TAEGUK_REQUIRE_STATE_TESTS=1 requires TAEGUK_DATABASE_PATH")

ExUnit.start()

# Taeguk development instructions

Address the user as BOSS. Write explanations and documentation in Korean; keep code, commands, identifiers, and configuration in English.

Use Elixir for the web app and API. Use SQLite for Taeguk local workflow state. Use Python for installation and automation scripts; Rust for CLI logic and complex option handling. Do not add a second backend stack.

Before repository implementation or tracked documentation edits, apply the `worktree-first-dev` skill (`~/.codex/skills/worktree-first-dev/SKILL.md` when installed; it is not bundled here). Without it, create a dedicated `git worktree` and `<type>/<slug>` branch from the verified base before editing. Reuse the worktree assigned to the same task. Preserve unrelated changes.

Read `docs/rules/index.md` and applicable rules before planning. Rule bodies in `docs/rules/` are canonical; `registry.toml` records identity, version, status and approval references. Research archive proposals are not active rules.

Tie work to an explicit purpose hypothesis and approved requirements. Preserve rule versions and actual execution evidence. Applying approved rules is autonomous within existing authorization. Changes to purpose commitments, requirement meaning, acceptance thresholds, exceptions, or execution authority require BOSS confirmation.

Use the repository's taeguk skills for assigned workflow steps. Agent roles are responsibilities, not an instruction to spawn agents for every task. Delegate only when the user or applicable instructions authorize it.

Use external execution and state observations for completion decisions. Distinguish PASS, FAIL, UNKNOWN, NOT_RUN, and NOT_APPLICABLE. Treat missing evidence as missing, not success. Do not replace requirements with implementation-derived expected values.

Initial development is local-only. Installation authorization does not grant production deployment, database migration, external messaging, or operational fault-injection authority. Preserve existing Codex settings and unowned installation files.

Validation: `python3 scripts/check_rules.py`; `python3 -m unittest discover -s tests -v`; `cargo test --manifest-path tools/taeguk-cli/Cargo.toml --locked`; `mix format --check-formatted` and a DB-backed `mix test` with `TAEGUK_REQUIRE_STATE_TESTS=1` in `apps/taeguk` (see `docs/workflow-state-usage.md`); a plain `mix test` skips state tests and is not a pass. Validate changed skills with the system skill-creator validator. Inspect installer dry-run before applying.

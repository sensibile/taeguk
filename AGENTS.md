# Taeguk development instructions

Address the user as BOSS. Write explanations and documentation in Korean; keep code, commands, identifiers, and configuration in English.

Use Elixir for the web app and API. Use PostgreSQL when persistence is required. Use Python for installation and automation scripts; Rust for CLI logic and complex option handling. Do not add a second backend stack.

Before repository implementation or tracked documentation edits, apply `/Users/tonton/.codex/skills/worktree-first-dev/SKILL.md`. Reuse the worktree assigned to the same task. Preserve unrelated changes.

Tie work to an explicit purpose hypothesis and approved requirements. Preserve rule versions and actual execution evidence. Applying approved rules is autonomous within existing authorization. Changes to purpose commitments, requirement meaning, acceptance thresholds, exceptions, or execution authority require BOSS confirmation.

Use the repository's taeguk skills for assigned workflow steps. Agent roles are responsibilities, not an instruction to spawn agents for every task. Delegate only when the user or applicable instructions authorize it.

Use external execution and state observations for completion decisions. Distinguish PASS, FAIL, UNKNOWN, NOT_RUN, and NOT_APPLICABLE. Treat missing evidence as missing, not success. Do not replace requirements with implementation-derived expected values.

Initial development is local-only. Installation authorization does not grant production deployment, database migration, external messaging, or operational fault-injection authority. Preserve existing Codex settings and unowned installation files.

Validation: `python3 -m unittest discover -s tests -v`; `cargo test --manifest-path tools/taeguk-cli/Cargo.toml --locked`; `mix test` and `mix format --check-formatted` in `apps/taeguk`. Validate changed skills with the system skill-creator validator. Inspect installer dry-run before applying.

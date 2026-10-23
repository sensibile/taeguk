# taeguk-init

Install or update the Taeguk bundle in the current project. Address the user as BOSS and explain in Korean. The installer source is `{{SOURCE}}`.

1. Resolve the project root with `git rev-parse --show-toplevel`. Outside a Git repository, ask BOSS which directory to use.
2. Preview with `python3 {{INSTALLER}} project --root <root> --agent {{AGENT}}`, keeping the installer argument exactly as written and passing `<root>` as one single-quoted shell argument (escape any `'` in it as `'\''`). Use other comma-separated agents (`codex`, `claude`) only when BOSS names them, including in the command arguments. Show the planned components, file count, source version and any warning.
3. Apply only after BOSS approves that preview: rerun with `--apply`, then with `--check`. Report the actual results.
4. Installed files under `.taeguk/`, `.claude/`, `.agents/` and `.codex/` are meant to be committed so the team shares the same version. Do not commit unless BOSS asks.
5. Offer, but do not perform without approval, one line in the project's AGENTS.md or CLAUDE.md: "Read `.taeguk/rules/index.md` before planning and apply Taeguk rules to explicitly assigned workflows." Never overwrite existing instructions.
6. If `shadowed_by_user_skills` appears, explain that user-level skills with the same name override the project copy in Claude Code and appear twice in Codex, and point to `taeguk_install.py migrate` and the legacy uninstall in `{{SOURCE}}/docs/installation.md`.

Rerunning the same command updates the project. Locally modified managed files block updates and removal; resolve them with BOSS instead of deleting. `--uninstall` removes only files recorded in the receipts.

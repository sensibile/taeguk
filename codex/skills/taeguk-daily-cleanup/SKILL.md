---
name: taeguk-daily-cleanup
description: "Run the daily cleanup of the Taeguk repository: collect evidenced cleanup candidates since the last run, keep only the items Codex agrees are needed, and deliver them through the PR flow; report restructuring signals to BOSS instead of acting on them."
---

# taeguk-daily-cleanup

Cleanup that a single task leaves out of its scope accumulates. This run picks it up once a day so it does not depend on the next task touching the same file.

## Authority

On 2026-10-29 BOSS set cleanup to run daily, with items that Codex agrees are needed carried through to merge, and restructuring to be reported and started only with BOSS approval. This applies to the Taeguk repository (`sensibile/taeguk`) only. Pilot projects need explicit BOSS authorization per project. Delivery follows `docs/repository-operations.md`; this skill grants no authority beyond it.

## Cleanup versus restructuring

Cleanup keeps behavior, contracts and responsibility boundaries unchanged: code, files or options with no remaining caller; duplication inside one module; wrong or stale comments, names and document references; leftover debug output or temporary code; broken links.

Anything that moves a responsibility, changes a contract or public interface, changes behavior, changes dependencies or CI workflows, or edits rule bodies, requirements, thresholds or authority is not cleanup. Record it as a restructuring or rule candidate and report it.

## Run

1. Fetch and record the `origin/main` SHA. Read the previous run report under `.taeguk/daily-cleanup/` in the main checkout (ignored by Git); the window starts at its recorded base SHA, or at the last 24 hours of `main` when there is none. If an open PR labeled `daily-cleanup` exists, do not start another; report it and stop.
2. Collect candidates from the window: the `## 정리 후보` sections of PRs merged into `main`, the merged diffs, and warnings or failures from the validation commands in AGENTS.md. Each candidate needs evidence that it is needed now, such as a search showing no caller, a compiler warning, or a broken reference. A candidate without evidence, or one justified only by readability or line count (DEV-002), is dropped.
3. Classify each candidate. Restructuring candidates go to the report for BOSS with the evidence and the alternatives from `$taeguk-inspect-changeability`; do not change code for them.
4. If no cleanup candidate is left, write the report and stop without a branch.
5. Otherwise create a worktree and a `chore/daily-cleanup-<date>` branch from the recorded SHA, apply the cleanup items as separate commits, and run the validation commands that apply.
6. Ask Codex whether each item is needed: run `$taeguk-codex-local-review` on the branch range with the cleanup necessity focus from its review prompts, together with the cross review focus, and the security review when code changed. Remove every item Codex judges unneeded or behavior-changing, and every item where its judgment is missing or unclear. Do not argue a rejected item again in the same run; record it. If items were removed, rerun validation and the review on the new head.
7. If any item is left, open a PR labeled `daily-cleanup` whose body lists each item with its evidence and Codex's judgment, and continue with the PR flow, including auto-merge. Apply VER-005 to review findings; escalate to BOSS when its escalation condition holds.
8. Write the run report to `.taeguk/daily-cleanup/<date>.md`: base SHA, window, candidates with classification and evidence, Codex judgments, removed items, checks and their statuses, PR link, and restructuring candidates for BOSS.

## Report

Tell BOSS in Korean what was merged or opened, what Codex rejected, and which restructuring candidates wait for approval. A run that found nothing says so; it is not PASS for code quality. A run stopped by a missing CLI, failed login or failing check is NOT_RUN or FAIL with the cause, never a silent skip.

---
name: taeguk-encode-mistake-class
description: "Turn a mistake that agents repeated in this repository into a structure, mechanical check or test that makes it fail loudly, proven against the real past mistake, and keep the ledger that pairs each lesson with what enforces it; prose is the last resort. Use when BOSS corrects the same mistake again, when harvested lessons are routed to the ledger, or when a ledger lesson that only a document enforces recurs."
---

# taeguk-encode-mistake-class

A lesson written only in a document, a skill or an agent's memory fails silently: the next agent that misses it repeats the mistake and nothing stops it. This skill moves a recurring mistake as high on the enforcement ladder as it will go and proves the result catches the mistake that actually happened.

## Authority

BOSS asked for this on 2026-10-30, adapted from the pstack `correct` skill (MIT, Lauren Tan). The two-occurrence threshold, the ladder order, the repeat escalation, the ledger, and reading a class selection as approval to wire its check into the existing suite are proposal defaults in [lesson intake and enforcement proposal](https://github.com/sensibile/taeguk/blob/main/docs/rules/proposals/lesson-enforcement.md) until BOSS confirms them. Until then, report the classes with their proposed level to BOSS and implement only the classes BOSS selects. Implementation goes through the repository worktree and PR flow like any change. A check or test placed in the existing test suite runs as soon as it merges (Python tests under `tests/` in `pre-push` and CI; Rust and Elixir test modules in CI only), so BOSS selecting the class covers that connection; when reporting a class, ask BOSS explicitly to approve that connection along with the selection. A new hook step, a new CI job, an exception, or recording the lesson as a rule in `docs/rules/` needs separate BOSS confirmation (GOV-002, [mechanical checks](https://github.com/sensibile/taeguk/blob/main/docs/rules/mechanical-checks.md)). Never weaken an existing check or expected value to make a class pass (GOV-004).

## Where it runs

The Taeguk repository (`sensibile/taeguk`), whose ledger is `docs/enforcement-ledger.md`. An installed project runs it only after recording BOSS approval and a ledger location in its own operations document; there, its own validation commands, worktree rules and PR flow replace the Taeguk ones.

## Find the classes

1. Read the inputs: `## 원장 후보` rows of `.taeguk/lessons/` reports, the BOSS correction at hand, and the ledger. Without a given class, scan recent commits, reverts, PR review adjudications, worker reports and agent memory files of this repository.
2. Record each occurrence with its date and evidence (commit, PR, review thread, report path or the quoted correction). Memory text is a claim; confirm it from the repository or GitHub before counting it.
3. Group occurrences into classes by the mistake, not the file. Report a class with one occurrence as observed once and propose a ledger row for it; a class with two or more is a candidate. Ledger rows change only in a PR for a class BOSS selected or a ledger update BOSS approved.
4. A candidate already in the ledger at the document level that occurred again after its row was written is a repeat: propose a level higher than before, and report it as unenforced if BOSS keeps the document level.

## Choose the level

Try the levels in order and record for each candidate why every higher level did not work:

1. **Structure.** Make the wrong path absent: one owner or helper for the operation, the old way deleted, the internal hidden so a wrong call fails. Example: one shared helper for clean git environments instead of each test building its own.
2. **Mechanical check.** A script or test in this repository that fails on the mistake with a message naming the file, helper or command to use instead. When the pattern already exists in many places, fail only on additions against the base, and list the existing ones as candidates.
3. **Behavior test.** A test that fails when the observable behavior breaks and keeps passing when only internals change.
4. **Document.** A skill, AGENTS.md or operations document sentence, only for judgment calls no mechanism can decide. Nothing fails when an agent skips it, so say so in the ledger.

## Prove it

- Run the new structure, check or test against the real past mistake: add a detached measurement worktree at the commit that contained it (`git worktree add --detach <path under taeguk-worktrees/> <commit>`), never the main checkout or another task's worktree, and remove it afterwards; or rebuild the smallest fixture from that commit and cite it. Record the command and its failing output. A check never shown failing on a real occurrence is a draft.
- Run it on the current base and record that it passes there, so it raises no false alarm on normal changes.
- For fault injection, commit or stash your own work first and restore with Git, never by hand; tests that call git must drop inherited `GIT_*` variables.
- Run the AGENTS.md validation for the changed areas.

## Deliver

One class per PR through the repository PR flow, with the evidence above in the PR body. In the same change, update the class's ledger row: level, enforcing path, proof command and result, and whether it is wired into a gate. A check outside the existing test suite that no hook or CI job runs is reported as unwired, not as enforced. When a class can no longer happen because a structure removed it, mark the row closed with the commit; do not delete it.

## Report

Tell BOSS in Korean: each class with its occurrences and evidence, the level chosen and why higher levels did not work, the proof output, the PR, and what still needs BOSS (gate wiring, exceptions, rule registration). A class left at the document level is reported as unenforced.

---
name: taeguk-test-gap
description: "Find untested behavior in the Taeguk repository by using coverage as a measuring instrument, classify each gap against approved requirements, and report test proposals with their oracle and fault check; never raise coverage as a goal. Report-only: no test edits, branch or PR."
---

# taeguk-test-gap

Coverage shows which code no test executed. It does not show whether a test checks the right thing, and a coverage number set as a target invites tests that execute code without checking behavior. This run uses coverage only to locate gaps, then judges each gap against approved requirements.

## Authority

On 2026-10-29 BOSS approved this worker as manual and report-only: BOSS starts each run, and a run writes a report and nothing else. It does not commit test or product code changes, open a branch or PR, or schedule itself; the only edit it makes is the temporary fault injection in step 6. Writing tests, opening PRs, auto-merge and scheduled runs each need BOSS confirmation first. This applies to the Taeguk repository (`sensibile/taeguk`) only; pilot projects need explicit BOSS authorization per project.

## Coverage is not a target

- Do not state a coverage percentage as a result, goal, threshold or improvement. Report gaps, not ratios.
- A gap is worth a test only when a defect there would break an approved requirement, a public contract or a safety boundary. Executing a line is never the reason.
- The expected value of a proposed test comes from a requirement, contract, rule or BOSS answer, never from what the code currently returns. A gap whose expected behavior has no such source is a question, not a test.
- A proposed test must name the fault that would make it fail. A test that would still pass with the behavior broken does not close the gap.
- When reading the code suggests the current behavior contradicts its requirement, report a suspected defect. Do not propose a test that pins the current behavior.

## Measurement

Measure every language the repository tests. Record each measurement's command, base SHA, tool version and status (VER-003). A missing tool or failed run is NOT_RUN or FAIL for that language, never "no gaps". Keep raw output outside the checkout, under `.taeguk/test-gap/<date>/` in the main checkout (ignored by Git).

| Language | Command | Known threats (MET-001) |
|---|---|---|
| Python | `coverage run --rcfile=<rc> -m unittest discover -s tests`, then `coverage combine --rcfile=<rc>` and `coverage report --rcfile=<rc> -m` | Write one rc file outside the checkout with `branch = True`, `parallel = True`, `source` listing `scripts` and every `codex/skills/*/scripts` (otherwise skill scripts vanish from the denominator) and `data_file` in the run directory. Tests run scripts in subprocesses, which read only the rc file, not the parent's command-line options: point `COVERAGE_PROCESS_START` at the rc file and put a `sitecustomize.py` calling `coverage.process_startup()` on `PYTHONPATH`. Umask tests leave data files with mode 000; make them readable before combining and record the combined, skipped and errored counts |
| Rust | `cargo llvm-cov --manifest-path tools/taeguk-cli/Cargo.toml --locked`, then `cargo llvm-cov report --manifest-path tools/taeguk-cli/Cargo.toml --show-missing-lines` for locations and `--json` for unexecuted functions (needs `llvm-tools-preview`) | The default output is a per-file summary without locations. Line and region counts differ; report uncovered functions and lines, not the region ratio |
| Elixir | DB-backed `mix test --cover` in `apps/taeguk` with `TAEGUK_REQUIRE_STATE_TESTS=1` (see `docs/workflow-state-usage.md`) | Without the state DB, state tests skip and their modules look untested; a plain run is not a measurement. Generated modules (`Router.Helpers`, error views) report 0% by construction |

Use an isolated virtual environment for `coverage`, such as `.taeguk/test-gap/venv`; do not install into a shared interpreter. Run every test suite with the same flags as the AGENTS.md validation commands so the measured run is the run that gates delivery.

## Run

1. Fetch and record the `origin/main` SHA. Measure in a clean worktree at that SHA, not in a checkout with local changes.
2. Run the measurements above and record their statuses and threats observed in this run.
3. Group uncovered lines, branches and functions into gaps by enclosing function or clause. Read the code and the tests around each gap; an uncovered line inside a function whose behavior a test already checks through another path is still a gap only if a distinct behavior is unchecked.
4. Classify each gap:
   - **requirement-linked**: a requirement, rule, contract document or installation guarantee states what must happen. Cite it.
   - **no caller**: a search shows nothing reaches it. Record the search as evidence; it becomes a daily cleanup candidate, not a test.
   - **expected behavior unknown**: no source states what it should do. Put the question to BOSS.
   - **environment-bound**: reaching it needs a condition the test environment cannot create safely (real home directory, network, signals across sessions). Record the condition and whether a fake or fixture could create it.
   - **measurement artifact**: the code runs in a test the tool could not follow. Show the test that runs it.
5. For each requirement-linked gap, write a proposal: the oracle source, the input or state, the expected observation, the fault to inject (the edit that would break the behavior) and the expected test failure. Rank proposals by the consequence of a defect there, such as overwriting unowned files, lost workflow state, an authorization bypass or a false PASS, not by line count.
6. Confirm the top proposals: inject the fault in the measurement worktree, run the suite that covers it with the validation flags, record the result, then restore the file and check `git status` is clean. The suite must pass on the unmodified tree first, and the mutant must build and run. The suite still passing confirms the gap. Only a test assertion failing on the mutant shows that another test already guards the behavior; then reclassify the gap as a measurement artifact and name that test. A build error, crash or environment failure on the mutant is UNKNOWN for that gap, not detection. This is a source edit in a local test run, not operational fault injection; it is the only change a report-only run makes, and it is never committed.
7. Write the report to `.taeguk/test-gap/<date>.md`: base SHA, measurements with commands, versions, statuses and threats, every gap with its location, classification and evidence, ranked proposals with their fault check results, questions for BOSS, cleanup candidates, and suspected defects.

## Report

Tell BOSS in Korean the top proposals, the questions that block a proposal, suspected defects and which measurements did not run. A run that found no requirement-linked gap says so; that is not PASS for test adequacy, since coverage cannot see an assertion that checks the wrong thing.

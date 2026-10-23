# 에이전트별 설치

## 구성

`scripts/taeguk_install.py`는 하나의 원본(`codex/agents`, `codex/skills`, `docs/rules`)을 에이전트별 형식으로 변환해 설치한다. 2026-10-23 BOSS 결정에 따라 사용자 레벨에는 `taeguk-init` 스킬과 CLI만 두고, 역할·스킬·규칙은 프로젝트에 설치한다. 설치본은 프로젝트 Git에 커밋하는 것을 기본으로 한다.

| 범위 | 구성요소 | 설치 위치 |
| --- | --- | --- |
| user | `init-claude` | `~/.claude/skills/taeguk-init/SKILL.md` (`disable-model-invocation`, `/taeguk-init`으로만 실행) |
| user | `init-codex` | `~/.agents/skills/taeguk-init/SKILL.md` |
| user | `cli` | `~/.local/bin/taeguk` |
| project | `rules` | `.taeguk/rules/` (`docs/rules` 전체 스냅샷) |
| project | `claude` | `.claude/skills/taeguk-*/`, `.claude/agents/taeguk-*.md` |
| project | `codex` | `.agents/skills/taeguk-*/`, `.codex/agents/taeguk_*.toml` |

한 에이전트만 제거할 때 다른 에이전트 설치가 남아 있으면 공유 구성요소(프로젝트의 `rules`, 사용자 레벨의 `cli`)는 남기고 결과의 `kept`로 표시한다. 제거 사전 검사는 receipt에 기록된 파일의 hash만 확인한다. hash 확인은 Git이 LF를 CRLF로 바꿔 checkout한 텍스트 파일(Markdown·TOML·Python 등)도 수정되지 않은 것으로 보며, CLI 같은 실행 파일은 원시 바이트로만 확인한다. 번들에는 원본 checkout에서 Git이 무시하는 파일(`__pycache__` 등)을 넣지 않는다. CLI 파일 이름에는 플랫폼 실행 파일 접미사를 붙이지만 Windows 실행은 검증하지 않았다. release CLI가 `tools/taeguk-cli`의 추적 소스보다 오래되었으면(수정 시각 기준) 설치를 거부하므로 소스가 바뀐 checkout에서는 다시 빌드한다. Windows에서는 POSIX 실행 비트를 검사하지 않는다. receipt의 역할 경로는 직속 `taeguk_*.toml`·`taeguk-*.md` 파일만, 스킬 경로는 `taeguk-*` 디렉터리 하위만 허용한다. `--root`는 이미 존재하는 디렉터리여야 하고 `--agent`에는 중복을 허용하지 않는다. 구성요소마다 receipt를 `<root>/.taeguk/receipts/<구성요소>.json`에 둔다. 프로젝트 receipt에는 절대경로 없이 원본 commit(`source_version`, 원본에 미커밋 변경이 있으면 `+dirty`)만 기록하므로 clone 위치가 달라도 `--check`가 동작한다. 적용·제거 전에 모든 구성요소의 충돌을 먼저 확인한다. 충돌 확인, 사용자 수정 보호, 쓰기 실패 복구, hash 대조 제거는 [Codex 설치](codex-installation.md)의 구현을 재사용한다. 제거 시 비게 된 `taeguk*` 디렉터리만 정리하고 공유 디렉터리는 남긴다.

발견 경로는 2026-10-23 공식 문서에서 확인했다. Claude Code는 프로젝트 `.claude/skills`, `.claude/agents`를 읽는다. 같은 이름이면 스킬은 개인 레벨이, 에이전트는 프로젝트 레벨이 우선한다. Codex는 CWD에서 저장소 루트까지의 `.agents/skills`와 프로젝트 `.codex/agents`를 읽고, 같은 이름의 스킬은 병합하지 않고 모두 노출한다.

- [Claude Code skills](https://code.claude.com/docs/en/skills), [Claude Code subagents](https://code.claude.com/docs/en/sub-agents)
- [Codex skills](https://learn.chatgpt.com/docs/build-skills), [Codex custom agents](https://learn.chatgpt.com/docs/agent-configuration/subagents)

## Claude 변환

`agents/openai.yaml`을 제외하고 Markdown의 `$taeguk-*` 호출을 `/taeguk-*`로 바꾼다. 역할 TOML은 `name`의 `_`를 `-`로, `developer_instructions`를 본문으로 옮긴다. 모델은 `gpt-6-astra`→`opus`, `gpt-6-luna`→`sonnet`(2026-10-23 BOSS 결정), verifier의 `gpt-6-sol`→`opus`(같은 날 BOSS 확인)다. 매핑에 없는 모델은 설치를 거부한다. `model_reasoning_effort`는 `effort`로 옮긴다. Codex 설치본은 원본 그대로 복사한다.

## 명령

```sh
cargo build --release --manifest-path tools/taeguk-cli/Cargo.toml --locked
python3 scripts/taeguk_install.py user
python3 scripts/taeguk_install.py user --apply
python3 scripts/taeguk_install.py project --root /path/to/project --agent claude,codex
python3 scripts/taeguk_install.py project --root /path/to/project --agent claude,codex --apply
```

모든 명령의 기본 실행은 dry-run이며 `--check`, `--uninstall`을 지원한다. `--agent`를 생략하면 설치는 `codex,claude` 전체를, `--check`·`--uninstall`은 receipt가 있는 에이전트(user 범위는 설치된 init과 CLI)만 대상으로 한다. 제거 전에는 각 receipt가 자기 구성요소 경로(예: `.claude/skills/taeguk-*`, `.taeguk/rules/`)만 기록했는지 확인하고, 그 밖의 경로가 있으면 아무것도 지우지 않는다. 사용자 레벨 설치 뒤에는 각 프로젝트에서 `/taeguk-init`을 실행한다. 스킬이 dry-run 결과를 보여주고 BOSS 승인 뒤 `--apply`, `--check`를 실행한다. AGENTS.md·CLAUDE.md에 `.taeguk/rules/index.md`를 읽도록 하는 한 줄은 승인을 받아야만 추가한다.

## 기존 전역 설치 이전

`install_codex.py`로 설치한 전역 역할·스킬은 프로젝트 설치본을 가린다. Claude Code에서는 개인 스킬이 이기고, Codex에서는 두 버전이 함께 보인다. 프로젝트 설치 결과의 `shadowed_by_user_skills`, 사용자 설치 결과의 `legacy`로 이를 표시한다.

```sh
python3 scripts/taeguk_install.py migrate
python3 scripts/taeguk_install.py migrate --apply
python3 scripts/install_codex.py --uninstall
python3 scripts/taeguk_install.py user --apply
```

`migrate`는 CLI hash가 기존 receipt와 일치할 때만 CLI 항목을 `~/.taeguk/receipts/cli.json`으로 옮긴다. 이후 `install_codex.py --uninstall`은 CLI를 남기고 전역 역할·스킬만 제거한다. 원본에 release CLI를 빌드하지 않았다면 `user --apply --no-cli`로 init만 설치하고, CLI는 이전된 receipt가 계속 관리한다. 이 경우 `--check`에도 `--no-cli`를 붙인다. 이전 뒤 `install_codex.py --check`는 receipt 불일치로 실패하는 것이 정상이다.

## 한계

설치 성공은 파일 배치와 hash 일치만 뜻한다. 2026-10-23 hoplites 작업 트리(trusted 프로젝트, codex-cli 0.163.0)에서 `.codex/config.toml`의 역할별 등록 없이 프로젝트 `.codex/agents/taeguk_*.toml` 10개가 Codex 역할 목록에 나타나고 `taeguk_designer` 호출이 성공하는 것을 관측했다. trusted가 아닌 프로젝트에서의 발견은 확인하지 않았다. 새 세션에서 발견, 모델·effort 적용, `/taeguk-init` 실행은 별도로 확인한다. 규칙 스냅샷의 `../research/archive` 링크는 프로젝트 안에서 끊기며 원본 commit의 태극 저장소를 가리킨다. `taeguk-init`은 설치 시점의 태극 저장소 절대경로를 담으므로 저장소를 옮기면 `user --apply`를 다시 실행한다. 원본에서 파일이 삭제되면 기존 receipt가 새 번들에 없는 파일을 가리켜 업데이트가 거부된다. 이 경우 먼저 제거 후 재설치한다. 제거는 receipt만으로 대상을 정하므로 원본 빌드나 현재 번들이 없어도 동작한다. 여러 구성요소 중 일부 적용 뒤 예외가 나면 앞선 구성요소는 적용된 상태로 남는다.

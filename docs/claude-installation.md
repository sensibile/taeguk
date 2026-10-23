# Claude Code 설치

## 구성

원본은 Codex와 같은 `codex/agents`, `codex/skills`다. `scripts/install_claude.py`가 설치 시점에 Claude Code 형식으로 변환한 복사본을 만든다. 변환본은 저장소에 두지 않는다. 원본 변경 후 설치 스크립트를 다시 실행한다.

| 원본 | 설치본 | 변환 |
| --- | --- | --- |
| `codex/skills/taeguk-*/` | `~/.claude/skills/taeguk-*/` | `agents/openai.yaml` 제외, Markdown의 `$taeguk-*` 호출을 `/taeguk-*`로 변경 |
| `codex/agents/taeguk_*.toml` | `~/.claude/agents/taeguk-*.md` | `name`의 `_`를 `-`로, `developer_instructions`를 본문으로, 모델·effort를 frontmatter로 |

모델 매핑은 2026-10-23 BOSS 결정에 따라 `gpt-6-astra`→`opus`, `gpt-6-luna`→`sonnet`이다. 매핑 결정에 없던 verifier의 `gpt-6-sol`은 `opus`로 두었고 BOSS 확인 대상이다. 모델이 없는 역할은 세션 모델을 상속한다. 매핑에 없는 모델은 설치를 거부한다. `model_reasoning_effort`는 `effort`로 옮긴다.

## 명령

```sh
python3 scripts/install_claude.py
python3 scripts/install_claude.py --apply
python3 scripts/install_claude.py --check
python3 scripts/install_claude.py --uninstall
```

기본 실행은 dry-run이다. receipt는 `~/.claude/taeguk-install.json`이며 Codex receipt와 별도로 관리한다. 충돌 확인, 사용자 수정 보호, 쓰기 실패 복구, hash 대조 제거는 [Codex 설치](codex-installation.md)와 같은 구현을 사용한다. `~/.claude/settings.json`, 다른 스킬·에이전트, Codex 설치본은 건드리지 않는다.

## CLI와 한계

Rust CLI `~/.local/bin/taeguk`는 Claude 설치 대상에 포함하지 않는다. Codex 설치가 이미 같은 경로를 관리하므로 두 receipt가 한 파일을 공유하지 않게 했다. 결과의 `cli` 값으로 존재 여부만 표시한다. `missing`이면 `install_codex.py --apply`로 설치하거나 빌드된 binary를 직접 실행한다.

설치 성공은 파일 배치와 hash 일치만 뜻한다. 새 Claude Code 세션에서 스킬·에이전트 발견과 실제 모델·effort 적용을 별도로 확인한다. 저장소 지침 `AGENTS.md`와 `worktree-first-dev` 스킬 경로는 Codex 기준 그대로다.

# Codex 설치

## 공식 형식과 로컬 구성

2026-10-11 공식 문서에서 개인 에이전트 TOML은 `~/.codex/agents`, 개인 스킬은 `~/.agents/skills`의 발견 경로를 확인했다. 에이전트의 `name`, `description`, `developer_instructions`, `model`, `model_reasoning_effort`와 스킬의 `name`, `description`을 사용한다. 모델은 명시된 역할에만 지정하고, 다른 역할과 전역 Codex 설정은 변경하지 않는다.

- [Custom agents](https://learn.chatgpt.com/docs/agent-configuration/subagents)
- [Build skills](https://learn.chatgpt.com/docs/build-skills)
- [Phoenix](https://phoenix.hexdocs.pm/overview.html)

소스는 `codex/agents`와 `codex/skills`에 둔다. 프로젝트 발견 경로에 같은 스킬을 중복 배치하지 않는다. 설치본은 복사본이다. 변경 후 설치 스크립트를 다시 실행한다.

## 명령

```sh
python3 scripts/install_codex.py
python3 scripts/install_codex.py --apply
python3 scripts/install_codex.py --check
python3 scripts/install_codex.py --uninstall
```

기본 실행은 dry-run이다. `--home`으로 격리된 설치 대상을 시험할 수 있다. 빌드된 Rust binary가 필요하다. 설치본은 `~/.agents/skills/taeguk-*`, `~/.codex/agents/taeguk_*.toml`, `~/.local/bin/taeguk`이며 receipt는 `~/.codex/taeguk-install.json`이다.

모든 대상의 충돌을 먼저 확인한다. 기존 비관리 파일이나 사용자가 바꾼 설치본은 덮어쓰지 않는다. 일반적인 쓰기 예외가 발생하면 이번 실행의 파일 변경을 복구한다. 설치 중 다른 프로세스가 같은 파일을 변경하지 않는 조건이다. 강제 종료·전원 차단에 대한 전체 트랜잭션 복구는 구현하지 않았다. 제거도 receipt와 실제 hash가 일치한 관리 파일만 대상으로 한다. 전역 config와 기존 simplicity 역할은 건드리지 않는다.

설치 확인은 receipt/hash 대조와 Codex 발견 확인을 구분한다. 실행 중인 대화는 새 역할·스킬을 즉시 반영하지 않을 수 있다. 새 세션에서 발견을, 실제 모델 호출을 별도로 확인하고, 실제 제품 과업의 판단 품질은 별도 평가한다. TOML 값과 설치 성공만으로 역할 발견이나 모델 호출이 증명되지는 않는다. 전역 PATH를 수정하지 않으며 필요하면 `~/.local/bin/taeguk`을 직접 실행한다.

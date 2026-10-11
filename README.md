# Taeguk

Mission 가설에서 작은 변경, 교차 검증, 실제 효과와 회고까지 연결하는 로컬 개발 구성이다. 현재는 에이전트·스킬·보조 CLI와 Elixir 웹/API의 초기 기반이다.

- [현행 규칙과 기계 검사](docs/rules/index.md)
- [진행 방향](docs/direction.md)
- [구성과 실행 계약](docs/architecture.md)
- [리서치 인덱스](docs/research/index.md)
- [Codex 설치](docs/codex-installation.md)
- [검증 기록](docs/validation.md)
- [태극 개선 루프·실행 결과](docs/self-improvement-result.md)

```sh
cargo build --release --manifest-path tools/taeguk-cli/Cargo.toml --locked
python3 scripts/install_codex.py
python3 scripts/install_codex.py --apply
python3 scripts/install_codex.py --check
cd apps/taeguk
mix deps.get
mix phx.server
```

웹은 기본 `http://127.0.0.1:4000`, API는 `/api/health`와 `/api/capabilities`다. 작업·검증 근거·실행 소유권은 선택적 SQLite 상태 API로 저장한다. [상태 API 사용](docs/workflow-state-usage.md)을 참고한다. 모델 실행·예약 실행·자동 배포·MCP는 아직 구현하지 않았다.

# Taeguk

목적·요구사항에서 작은 변경, 교차 검증, 적용 후 효과와 회고까지 연결하는 에이전트 개발 구성이다. 관측된 개발 체계의 문제는 self-improvement 루프로 수정하고 검증한다.

## 현재 구성

| 구성 | 현재 기능 |
| --- | --- |
| Codex 역할 10개 | 설계, 검증 계획·도구 제작, 구현, 도구 검증, LLM 검토, 조정·회고·규칙 변경 제안 |
| 스킬 11개 | 작업 계약, 조사, 작은 변경, 교차 검증, 효과 확인, 회고와 자체 개선 |
| Rust CLI | `measure`, `verify`, `audit`, `state` |
| Elixir/Phoenix API | 작업·검증 계획 참조, 관측·실행·자원 소유권, 명령 이벤트 |
| SQLite | 파일 기반 상태 저장, revision 충돌 방지, 재전송 중복 억제, transaction |
| 개선 루프 | 재현 → 검증 → 적용 → 새로운 효과 평가 → 완료 gate |

에이전트가 판단과 실행을 맡고 도구가 기록·순서·등록 근거를 검사한다. 백엔드가 모델을 호출하거나 상주 작업을 자동 실행하지 않는다. 근거 등록은 진실성·독립성·사용자 승인의 인증이 아니다. 규칙의 의미·임계값·예외·권한 변경은 사용자 확인을 유지한다.

## 시작하기

Python 3.11 이상, Rust 2024 edition을 지원하는 toolchain, Elixir와 Erlang/OTP가 필요하다. Elixir 버전 조건은 [mix.exs](apps/taeguk/mix.exs)를 따른다. SQLite NIF 빌드에는 환경에 따라 C 빌드 도구가 필요하다.

```sh
git clone https://github.com/sensibile/taeguk.git
cd taeguk
cargo build --release --manifest-path tools/taeguk-cli/Cargo.toml --locked
python3 scripts/install_codex.py
python3 scripts/install_codex.py --apply
python3 scripts/install_codex.py --check
```

첫 설치 명령은 dry-run이다. 기존 비관리 파일이나 사용자가 수정한 설치본은 덮어쓰지 않는다. 설치 경로·제거·발견 확인은 [Codex 설치](docs/codex-installation.md)를 참고한다. CLI는 `~/.local/bin/taeguk`에 설치되며 PATH는 자동 수정하지 않는다.

SQLite 상태 API는 저장 경로와 token을 설정한 뒤 시작한다.

```sh
mkdir -p .taeguk
export TAEGUK_DATABASE_PATH="$(pwd)/.taeguk/state.sqlite3"
export TAEGUK_API_TOKEN="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
export TAEGUK_PORT=4000
cd apps/taeguk
mix deps.get
mix ecto.migrate
mix phx.server
```

같은 token을 사용하는 클라이언트 셸에서 `TAEGUK_API_URL=http://127.0.0.1:4000`으로 연결한다. token은 Git이나 요청 JSON에 저장하지 않는다. DB 미설정 시 기본 화면과 `/api/health`, `/api/capabilities`만 사용할 수 있다. 상태 API의 요청 형식과 복구 한계는 [사용법](docs/workflow-state-usage.md)에 있다.

## 읽는 순서

1. [진행 방향](docs/direction.md), [구성과 실행 계약](docs/architecture.md)
2. [현행 규칙](docs/rules/index.md) — 승인된 규칙과 미승인 제안을 구분
3. [상태 API](docs/workflow-state-usage.md), [개선 루프 스킬](codex/skills/taeguk-self-improve/SKILL.md)
4. [자체 개선 첫 완주 결과](docs/self-improvement-result.md), [SQLite 검증 결과](docs/workflow-state-result.md)
5. [리서치 인덱스](docs/research/index.md), [이전 검증 기록](docs/validation.md)

리서치는 검토 범위를 명시한 요약과 독립 적용 자료다. 역사적 문서의 로컬 경로와 외부 프로젝트 참조는 이 저장소에 포함된 아티팩트를 뜻하지 않는다. 금융 원자료, 실험 DB, dependency 캐시와 로컬 원시 로그는 게시 대상에 포함하지 않는다.

## 검사와 확인 범위

```sh
python3 scripts/check_rules.py
python3 -m unittest discover -s tests -v
cargo test --manifest-path tools/taeguk-cli/Cargo.toml --locked
cd apps/taeguk
mix format --check-formatted
export TAEGUK_DATABASE_PATH=/private/tmp/taeguk-state-test.sqlite3
export TAEGUK_API_TOKEN=local-test-token-at-least-24-bytes
mix ecto.migrate
TAEGUK_REQUIRE_STATE_TESTS=1 mix test
```

일반 `mix test`는 DB 설정이 없으면 DB 통합 검사를 skip한다. `TAEGUK_REQUIRE_STATE_TESTS=1`이면 DB 미설정 시 바로 실패하므로 skip이 통과로 보이지 않는다. DB 경로는 `*-test.sqlite3`로 끝나야 한다. GitHub Actions는 규칙 구조, Python, Rust, DB를 포함한 Elixir 검사를 실행한다. 실제 서버를 띄운 HTTP 검증과 branch protection 필수 검사 지정은 CI 범위가 아니다.

첫 self-improvement 실험에서는 로컬 SQLite 오류 분류를 같은 두 사례에서 비교해 분류 오류 1건 → 0건을 확인했다. 사용자 시간 절감, 장기 효과, 여러 프로젝트의 전이·공통화는 아직 확인하지 않았다. 자동 배포, 운영 복구와 프로젝트별 설치 격리는 제공하지 않는다.

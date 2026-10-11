# 초기 구성 검증 기록

2026-10-11, 작업 브랜치 `feat/bootstrap`.

## 확인한 결과

- Python 설치기: 8개 테스트 통과. 비관리 파일 충돌, 사용자 수정 보호, 심볼릭 링크, 잘못된 receipt, 쓰기 실패 복구, 재설치·제거를 포함한다.
- 실제 번들: 임시 home에 39개 파일 설치·hash 검사·제거 통과. 원래 Codex 구성은 변경하지 않았다.
- Rust CLI: 9개 테스트 통과. 미확인 사건의 상·하한, 빈 입력, 수치 범위·overflow, 실제 파일 digest, 경로 탈출과 심볼릭 링크를 확인했다. release binary에서도 PASS/FAIL/UNKNOWN 사례를 확인했다.
- Elixir/Phoenix: ExUnit 3개 통과. 실제 localhost HTTP에서 `/`, `/api/health`, `/api/capabilities`의 200과 없는 API의 JSON 404를 확인했다. health는 프로세스 응답만 나타낸다.
- 스킬 10개: skill-creator의 `quick_validate.py` 통과. PyYAML은 임시 검증 가상환경에만 설치했다.
- 리서치 원본 16개: manifest SHA-256 대조 통과. 원본의 읽기 범위·검증 상태를 유지했다.

## 재현 명령

```sh
python3 -m unittest discover -s tests -v
cargo test --locked --manifest-path tools/taeguk-cli/Cargo.toml
cargo build --release --locked --manifest-path tools/taeguk-cli/Cargo.toml
cd apps/taeguk
mix deps.get
mix format --check-formatted
mix test
mix phx.server
```

Python 3.13, Rust 1.99, Elixir 1.20/OTP 29, Phoenix 1.8.15, Codex 0.162.0에서 확인했다. lockfile을 보존했다. 처음 HTTP 검증에서 명시적인 `server: false`가 개발 서버 시작을 막는 문제를 발견해 제거했다. 로컬 접속 권한이 필요한 검사에는 해당 실행 환경의 권한을 적용했다.

## 경계와 남은 검증

자체 코드 검토와 테스트는 독립된 에이전트 검증이 아니다. TOML/스킬 형식과 설치 파일의 일치 여부는 실제 제품 작업에서 에이전트가 올바르게 판단한다는 증거가 아니다. 설치 후 Codex 0.162.0의 `debug prompt-input`에서 10개 스킬 이름이 모두 발견됨을 확인했다. 역할 6개의 TOML과 설치 hash는 확인했지만 실제 역할 호출과 제품 과업의 판단 품질은 아직 검증하지 않았다. 실제 제품 하나에 적용해 평가한다.

브라우저 상호작용·접근성, 운영 배포·복구·부하, PostgreSQL, MCP, 영속 작업 상태와 자동 반복 실행은 이번 완료 범위에 포함하지 않았다. 로컬 웹/API는 초기 상태 안내만 구현한다. production 실행에는 별도의 `TAEGUK_SECRET_KEY_BASE`(64 bytes 이상)가 필요하지만 이것만으로 운영 준비가 완료되지는 않는다.

## 규칙 관리 추가 검증 — 2026-10-11

`feat/rule-registry`, 시작 commit `a6ffb551f3288444fec9c72695136ed590d91a43`에서 진행했다. 승인 규칙 10개와 제안 1개를 registry에 등록했다. 신규 규칙 검사 테스트 11개를 포함해 Python 19개, Rust 9개, Phoenix 3개가 통과했다. 규칙 문서의 형식·링크·heading과 ID·버전·상태·승인 참조를 검사했다.

로컬 pre-push hook을 활성화하고 정상 실행을 확인했다. 별도의 임시 Git 저장소에서 잘못된 registry를 넣어 hook이 exit 1로 차단함을 확인했다. 기존 실행 hook과 hooksPath는 없었으며 사용자 전역 Git 설정은 변경하지 않았다. GitHub workflow 파일은 준비했지만 remote·원격 실행·필수 체크 설정은 없다.

스크립트와 hook·CI는 자체 코드·보안 검토를 했다. 검사기는 문서와 설정을 읽고 JSON을 출력하며 shell 명령이나 규칙 본문을 실행하지 않는다. CI 권한은 contents: read이고 배포·secret 권한을 추가하지 않았다. 독립 리뷰가 완료됐다는 주장은 하지 않는다. 의미 판단·승인 인증·실제 제품 검사 실행을 자동 검증하지 않는다. 설치된 에이전트·스킬·CLI payload는 변경하지 않았다.

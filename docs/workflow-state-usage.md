# 로컬 상태 API와 CLI

태극 상태 DB는 SQLite 파일이다. 에이전트는 파일에 직접 쓰지 않고 단일 Elixir API를 이용한다. 다른 호스트에서 공유 파일에 접근하는 구성은 지원하지 않는다. 이 API는 모델을 호출하거나 프로세스를 종료하지 않는다.

## 시작

저장 경로의 상위 디렉터리를 먼저 생성한다. token은 로컬에서 생성하고 명령 JSON·문서·Git에 저장하지 않는다.

```sh
cd ~/Documents/workspace/taeguk/apps/taeguk
mkdir -p ../../.taeguk
export TAEGUK_DATABASE_PATH="$(cd ../.. && pwd)/.taeguk/state.sqlite3"
export TAEGUK_API_TOKEN="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
export TAEGUK_PORT=4000
mix deps.get
mix ecto.migrate
mix phx.server
```

클라이언트 셸에는 같은 token을 안전하게 전달하고 `TAEGUK_API_URL=http://127.0.0.1:4000`을 설정한다. listener와 CLI는 loopback HTTP를 사용한다. 로컬 신뢰 경계 안의 공유 token이며 actor 문자열이 개별 에이전트 인증을 의미하지 않는다. 외부 인터넷 연결용 인증 시스템이 아니다.

```sh
taeguk state list
taeguk state create work.json
taeguk state get WORK_UUID
taeguk state command WORK_UUID command.json
taeguk state history WORK_UUID 0
```

## 요청 예시

create 파일은 아래 형태다. UUID와 참조는 실제 값으로 교체한다. 기준은 생성 시 고정한다. purpose·검사·허용 상태를 변경하려면 새로운 work item으로 기존 작업과 연결한다. 사용자 승인 사실을 이 API가 인증하지 않는다.

```json
{
  "id": "11111111-1111-4111-8111-111111111111",
  "command_id": "22222222-2222-4222-8222-222222222222",
  "actor": "designer",
  "purpose": "중복 검증을 줄이면서 완료 근거를 보존한다",
  "requirement_ref": "docs/requirements.md#R1",
  "rule_ref": "rules:v1",
  "design_ref": "design:commit",
  "plan_ref": "plan:commit",
  "artifact_ref": "head:commit",
  "environment_ref": "local:fixture-v1",
  "checks": [{"id":"C1","method":"mixed","required":true,"oracle_ref":"plan#C1","accepted_statuses":["PASS"]}]
}
```

command의 공통 필드는 `command_id`, `expected_revision`, `actor`, `operation`이다. GET에서 현재 revision을 읽고 다음 명령에 넣는다. 각 새로운 명령마다 새로운 UUID를 만든다.

| operation | 추가 필드 |
| --- | --- |
| observe | check_id, kind(tool 또는 llm), status, artifact_ref, environment_ref, plan_ref, evidence, reason |
| reuse | observation_id, reason |
| retarget | artifact_ref, environment_ref, reason |
| start_run | label, resources(고유 문자열 배열) |
| update_run | run_id, handle |
| end_run | run_id, status(finished 또는 interrupted), cleanup_ref, result_ref |
| complete | 없음 |

`evidence`는 `[{"location":"result.log","sha256":"64자리 소문자 SHA256"}]`다. API는 파일을 읽거나 SHA를 계산하지 않는다. 모든 관측은 설명이 필요하다. 완료는 현재 plan/artifact/environment의 필수 검사에 허용된 상태가 등록되고 실행 중 run이 없는 경우에만 가능하다. mixed는 tool과 llm 두 층을 요구한다. 참조는 내용 hash로 고정하는 것을 권장한다. 단순 경로 참조의 내용이 바뀌어도 API가 감지하지 않는다.

resource 문자열은 모든 work item에 걸쳐 하나의 run만 소유할 수 있다. 동일 실제 자원에 동일 식별자를 사용해야 한다. 소유권 기록은 실제 프로세스의 실행을 강제로 막지 않는다. 프로세스 종료와 cleanup을 확인한 후 end_run으로 release한다. 시간만 지나면 자동 해제하지 않는다.

동일 command_id·동일 요청의 재전송은 **원래 ACK**를 반환한다. ACK revision이 최신이라는 뜻은 아니므로 GET으로 확인한다. 같은 ID의 다른 요청, 낡은 revision, 이미 점유한 자원은 409다. lock 대기/DB 오류는 503, 잘못된 입력은 422, token 오류는 401이다. CLI는 명령을 자동 재시도하지 않는다. 전송 실패로 결과를 모르면 먼저 조회하고 필요 시 동일 요청을 재전송한다.

## 파일과 복구

WAL, foreign keys, FULL synchronous, 5초 busy timeout을 사용한다. 쓰기 transaction은 immediate이며 여러 에이전트의 쓰기는 직렬화된다. 읽기는 deferred snapshot이다. 이 규모의 로컬 상태 관리에 맞춘 선택이며 대규모 분산 서비스 처리량은 측정하지 않았다.

실행 중 DB 파일만 복사하면 WAL의 최근 변경을 놓칠 수 있다. 백업은 SQLite backup API를 사용하거나 API를 정상 종료하고 DB 연결이 닫힌 뒤 진행한다. 이번 구현은 자동 백업/복원 명령을 제공하지 않는다. DB 디렉터리·token·로그 접근권한은 로컬 운영자가 관리한다.

검사는 전용 테스트 파일로 실행한다. 일반 `mix test`는 상태 DB 미설정 시 DB 통합 검사를 skip한다. DB 통합 검사에는 실제 별도 파일과 token이 필요하다.

```sh
TAEGUK_DATABASE_PATH=/private/tmp/taeguk-state-test.sqlite3 mix ecto.migrate
TAEGUK_DATABASE_PATH=/private/tmp/taeguk-state-test.sqlite3 TAEGUK_API_TOKEN=local-test-token-at-least-24-bytes mix test
```

설계/검증 계획: [workflow-state-plan.md](workflow-state-plan.md). SQLite transaction 설정 근거: [Ecto SQLite3](https://ecto-sqlite3.hexdocs.pm/Ecto.Adapters.SQLite3.html), [SQLite WAL](https://sqlite.org/wal.html).

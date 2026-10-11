# 태극 상태 백엔드 작업 계약·설계·검증 계획

2026-10-11. BOSS의 “요 방향으로 태극을 좀 더 발전” 요청은 작업·계획·검사·근거·실행 소유권을 공유 저장하는 로컬 백엔드와 CLI의 첫 버전으로 적용한다. 목적 가설은 수동 인계와 중복 실행·낡은 결과 덮어쓰기의 부담 감소다. 실제 시간 절감은 이번 구현 검사와 구분한다.

## 범위와 설계

- 기존 Elixir/Phoenix API에 SQLite/Ecto Repo를 선택적으로 연결한다. DB 미설정 시 기존 bootstrap API는 동작한다. DB를 설정한 상태 API는 설정된 token으로 보호하고 HTTP listener는 loopback이다. token은 환경에서 읽고 로그·이벤트에 저장하지 않는다.
- work item은 목적·요구/규칙 참조·설계 참조·검증계획 참조·현재 artifact/environment 및 검사 정의를 저장한다. 검사 정의는 tool/llm/mixed, 필수 여부, 허용 완료 상태와 oracle 출처를 포함한다. 기준은 생성 때 고정한다. 기준 변경 API나 사용자 승인 인증은 만들지 않는다.
- SQLite의 BEGIN IMMEDIATE 쓰기 transaction과 예상 revision으로 원자 갱신한다. 검사 관측·run·이벤트·자원 claim도 같은 transaction에 묶는다. 명령 ID와 동일 payload의 재전송은 원래 ACK를 반환하고 다른 payload 재사용은 충돌이다. 명령을 자동 retry하지 않는다.
- 관측은 tool/llm을 구분하고 check·plan·artifact/environment·actor·근거 위치/hash·설명에 연결해 append한다. API는 근거 파일을 읽거나 실행하지 않고 등록 사실만 저장한다. artifact/environment가 바뀌면 이전 관측은 자동 통과 근거가 되지 않는다. 재사용은 이전 관측 ID와 이유를 명시한 별도 기록으로 남긴다.
- 완료 gate는 현재 각 필수 검사의 요구 관측 층과 허용 상태를 확인하고 실행 중 run이 없음을 확인한다. 빈 필수 계획은 완료할 수 없다. NOT_APPLICABLE도 계획에서 허용해야 하며 이유가 있어야 한다. 이는 기록 완전성 판단이지 증거 진실성·독립성·사용자 효과의 인증은 아니다.
- 실행은 owner·resource·handle·상태·cleanup 참조를 기록한다. resource는 서로 다른 작업 사이에도 동시 claim할 수 없다. 완료/중단 기록과 소유권 release는 원자적이다. timeout만으로 실제 프로세스가 종료됐다고 보고 claim을 재할당하지 않는다. 백엔드는 프로세스를 실행하거나 죽이지 않는다.
- Rust CLI에 state list/get/create/command를 추가한다. 환경에서 로컬 API 주소/token, JSON 파일에서 변경 요청을 읽는다. 오류·충돌을 명확히 반환하며 redirect/비loopback 연결·자동 재시도는 허용하지 않는다. 기존 measure/verify/audit는 보존한다.
- 운영 배포, scheduler, 모델 호출, MCP, 사용자 UI, tenant/역할별 인증, 대용량 log 업로드는 제외한다. 기준·모델 구성 및 승인된 규칙은 바꾸지 않는다. 실험 DB/HTTP는 새 전용 DB 파일 및 HTTP4120에 격리한다.

## 구현 전 검증 계획

| ID | 기대 결과와 원문 연결 | 검사·교차 근거 | 시작 상태 |
| --- | --- | --- | --- |
| S01 | 기존 bootstrap 및 CLI 유지; 저장 상태는 실제 설정과 구분 | 기존 Python/Rust/Elixir 회귀, DB 미설정 API 및 상태 접근 실패 확인 | NOT_RUN |
| S02 | 작업·계획·관측·이벤트 영속 저장·재개 | 실제 파일 SQLite 통합, 새 HTTP 연결 및 서버 재시작 뒤 GET, 독립 SQL 행 대조 | NOT_RUN |
| S03 | 같은 revision의 병렬 갱신은 하나만 반영; 실패 원자성 | 별도 SQLite 연결 장벽, 한 성공/한 충돌, 작업/이벤트/관측 SQL 및 오류 뒤 재조회 | NOT_RUN |
| S04 | 같은 ID·payload 재전송은 효과1회, 다른 payload는 충돌 | ACK·revision·행 수 대조, 과거 ACK 뒤 현재 GET을 구별 | NOT_RUN |
| S05 | tool/LLM/mixed 근거 누락·FAIL/UNKNOWN·낡은 artifact는 완료 차단 | 사례별 gate 통합과 실제 CLI HTTP, 명시 reuse 후 이전 관측 보존 SQL | NOT_RUN |
| S06 | 공유 resource는 작업 간 중복 claim 불가; 다른 자원은 가능 | 별도 연결 경합, owner 오류·stale 종료 거절, release 뒤 획득, run/event/claim SQL | NOT_RUN |
| S07 | 미인증·잘못된 입력은 상태를 바꾸지 않음 | token/JSON/상태·ID·해시·미지정 필드·크기 제한 HTTP, DB 이전/이후 대조, 코드의 SQL parameterization/참조 미실행 검토 | NOT_RUN |
| S08 | 설치본 CLI와 지침을 실제 사용할 수 있음 | Rust client 응답/충돌·offline·redirect/remote 거절 검사; installer dry-run/apply/check 및 기존 설정 hash | NOT_RUN |

스키마·코드·입력의 기대값을 통과 결과에서 역산하지 않는다. 이번에 각 역할을 자동 spawn하지 않고 현재 채팅에서 설계·계획을 먼저 고정하고 구현/검증을 수행한다. 자체 코드+테스트의 공유 가정과 독립 SQL/HTTP 관찰의 한계를 최종 기록한다. 변경 후 재검증은 영향 ID를 기준으로 선택하고 하나의 결과 표에 연결한다.

## 저장소 수정

BOSS의 2026-10-11 수정에 따라 SQLite로 전환했다. 단일 로컬 API가 파일을 소유한다. WAL과 foreign keys, busy timeout, FULL synchronous를 사용하며 쓰기는 immediate, 조회는 deferred transaction이다. revision CAS와 자원 unique constraint는 유지한다. DB lock 대기는 유한하고 실패는 재시도 없이 반환한다. 여러 호스트의 공유 파일 시스템 DB는 범위 밖이다.

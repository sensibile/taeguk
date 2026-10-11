# SQLite 상태 백엔드 구현 결과

2026-10-11. 구현 버전 `3a8010c`. 출발 `ab85c2a`, 작업 브랜치 `feat/workflow-state`. BOSS의 DB 변경 지시에 따라 SQLite로 구현했고 PostgreSQL 전용 실험 컨테이너는 중지·보존했다. 제품/미다스/벨로체 DB는 변경하지 않았다.

## 구현과 실제 검증

Elixir API가 파일 SQLite를 소유한다. WAL·foreign keys·FULL synchronous·5초 busy timeout, 쓰기 immediate/읽기 deferred transaction을 사용한다. 작업 기준은 생성 때 고정하고 명령 ID 재전송, revision CAS, 관측 append와 현재 context 필터, 명시 reuse, 완료 gate, run/resource 소유권을 저장한다. Rust 상태 CLI는 token으로 loopback에 요청하며 proxy·redirect·자동 재시도를 사용하지 않는다. 기존 규칙 본문과 모델 구성은 변경하지 않았다.

| 검사 | 결과 | 실제 근거와 범위 |
| --- | --- | --- |
| S01 기존 구성 | PASS | Python26, Rust20; DB 미설정 Elixir bootstrap3 PASS(당시 통합8 skip). 규칙 registry 구조 검사 PASS. |
| S02 영속성과 복원 | PASS | CLI create/observe/get/history 후 Python sqlite3 행·revision 대조. 서버 정상 종료·재시작 후 같은 revision2 조회. |
| S03 원자 갱신 | PASS | 서로 다른 checkout 연결로 같은 revision 명령을 동시에 제출하여 성공1/충돌1. event 삽입 오류 trigger로 관측·revision 전체 rollback. 외부 연결의 BEGIN IMMEDIATE 동안 HTTP503, 변경 없음. |
| S04 재전송 | PASS | 동일 요청의 과거 ACK 재현, 다른 payload의 같은 명령 ID 거절, 효과 중복 없음. |
| S05 완료 gate | PASS | mixed 두 층, UNKNOWN/미실행 차단, 닫힌 작업 차단. artifact 변경 뒤 기존 관측 제외, 명시 reuse 새 기록과 원본 유지. 허용/미허용 N/A, 빈 필수 계획 차단. |
| S06 실행 소유권 | PASS | 두 작업·별도 연결의 동일 resource 경합 승자1. 실패한 run 삽입 rollback, actor 불일치 거절, running 중 retarget 차단, end_run 후 release. 실제 프로세스 제어는 없음. |
| S07 입력/인증 | PASS | 무token401, 잘못된 필드/층/해시/빈 설명 거절, 실제 HTTP malformed400·과대 요청413. SQL 값은 파라미터 바인딩이며 근거 참조를 fetch/execute하지 않음. 보안 감사 전체를 의미하지 않음. |
| S08 설치/클라이언트 | PASS | installer dry-run 검토→apply45files→check. Codex config SHA 동일. 설치본으로 실제 API GET 성공, offline 오류. Rust redirect/비loopback/충돌·응답 파싱 검사. |

SQLite 설정으로 Elixir14개가 통과했다(통합11+기존3). history25개 페이지 다음 조회도 검사했다. format/diff 확인을 수행했다. DB 통합 테스트는 `*-test.sqlite3` 전용 파일만 허용한다.

## 제한과 다음 사용

- 실제 다섯 역할 모델을 이번에 자동 실행하지 않았다. 기존 역할과 스킬 파일도 변경하지 않았다. 상태 API는 실행자가 명시적으로 호출하는 기반이다. Codex 사용자 역할의 새 설정 자동 로딩을 확인했다고 주장하지 않는다.
- 공유 token과 actor 라벨은 개별 에이전트 인증이 아니다. resource 등록은 동일 문자열을 사용하는 협력 실행자에게만 의미가 있다. 실제 프로세스의 배타 실행/종료를 강제하지 않는다.
- 기준 참조와 근거 SHA의 진실성·독립성·사용자 승인·제품 효과는 인증하지 않는다. 같은 모델이 구현과 native 테스트를 작성했고 별도 Python/HTTP/SQL 관찰은 층 간 교차 근거다. 독립 LLM 리뷰는 실시하지 않았다.
- 실제 태극 제품 작업에서 중복/시간이 줄었는지 UNKNOWN. 큰 부하·전원 장애·백업 복원·여러 호스트 운영도 NOT_RUN이다.
- 전용 테스트 DB `/private/tmp/taeguk-workflow-state-test.sqlite3`와 로그는 실험용으로 보존한다. 서버4120은 종료했다. 자동 상주 서비스나 자동 백업은 설치하지 않았다. [사용법](workflow-state-usage.md)에 따라 실제 저장 경로를 지정해 시작한다.

## 전환 중 수정

PostgreSQL 시도는 owner guard 컴파일 오류로 migration 실행 전 실패했다. SQLite로 전환하며 guard, 비조회 SQL의 nil rows 처리, SQLite 번호 지정 파라미터(`?1`) 바인딩을 수정했다. 최종 테스트/HTTP 재시작 검사에는 수정본을 사용했다. 최초 loopback probe는 sandbox 연결 제약으로 실패했고 승인된 로컬 접근으로 다시 수행했다. 초기 mix test 한 번은 잘못된 저장소 루트에서 실행되어 앱 디렉터리로 수정했다. 이 실패들을 성공 검사와 섞지 않는다.

로그: `/Users/tonton/Documents/study/experiments/taeguk-workflow-state-20261011/`. `sqlite-tests-final.log`, `http-probe.log`, `restart-probe.log`, `installed-http.log`, `offline-cli.log`, `python-tests.log`, `rust-tests.log`, `config-preservation.json`, installer 결과에 최종 근거가 있다.

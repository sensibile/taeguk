# 태극 self-improvement 첫 완주 결과

2026-10-11. 기준 b6209cc, 설계/검증계획4219159, 루프9247e6a, 첫 수정cd7393c, 최종 실행 버전382f30d. 프로젝트별 프리징/분기는 이번에 구현하지 않았다. 목적은 관측된 태극 문제를 회고 설명에서 끝내지 않고 수정·검증·적용·효과 관찰로 연결하는 것이다.

## 제공한 루프

새 `taeguk-self-improve` 스킬과 coordinator 진입점이 개선 작업을 이어간다. Elixir/SQLite API는 기존 work item의 선택적 improvement 계약과 phase를 검사한다. 일반 작업의 계약은 그대로다. CLI는 기존 state 명령으로 사용한다. 별도 engine·스케줄러·LLM 호출 코드를 추가하지 않았다.

계약에는 회고/source, 실제 문제, baseline artifact, rollback 참조, verification check ID, 별도 effect check ID를 고정한다. 단계는 baseline→verified→applied→evaluated이며 complete는 별도 gate다. 단계 건너뛰기, 기준 누락, 미통과 검증, 낡은 context를 거부한다. applied 뒤 작성한 새 effect가 있어야 하며 이전 effect의 reuse로 대체할 수 없다. mixed 검사는 두 층 모두 요구한다. FAIL/UNKNOWN/NOT_RUN/NOT_APPLICABLE 효과를 기록할 수 있지만 완료할 수 없다. retarget은 이후 진행을 무효화하고 기존 baseline과 이벤트를 보존한다.

실제 Git 수정/검사/설치/측정은 에이전트가 수행한다. API가 근거의 진실성·독립성·사용자 승인이나 실제 적용을 인증하지 않는다. 이것은 에이전트가 실행하고 도구가 기록·순서를 검사하는 루프이며, 사람 없이 상주 실행되는 daemon은 아니다. 공통 규칙의 의미/임계값/예외/권한 변경은 기존처럼 BOSS 확인 대상이다.

## 관측된 문제와 같은 조건의 전후 비교

직전 SQLite HTTP 실험에서 잠금 경합이 일반 SQL 실패와 같은 `database_operation_failed`로 보고됐다. 새 baseline에서도 외부 sqlite3 연결로 BEGIN IMMEDIATE를 잡아 재현했다. 일반 DB 오류는 전용 trigger로 만들어 비교했다. 제품/운영 DB가 아닌 전용 로컬 fixture다.

가설: 알려진 자원 경합을 다른 DB 실패와 구분하면 실행자가 소유자·실행 기록을 확인할 수 있다. 수정은 고정된 Exqlite의 알려진 lock/busy 오류만 `database_busy`로 분류한다. HTTP503, 5초 timeout, 자동 retry 없음, rollback/CAS/명령 재전송 동작은 유지했다. 알 수 없는 드라이버 메시지는 일반 오류로 남는다.

| 같은 두 사례 | baseline4219159 | 후보cd7393c | 적용 후382f30d |
| --- | --- | --- | --- |
| 실제 SQLite 쓰기 잠금 | 일반 오류로 보고 | database_busy | database_busy |
| 일반 SQL 오류 | database_operation_failed | 동일 | 동일 |
| 분류 오류 건수 / 사례 수 | 1 / 2 | 0 / 2 | 0 / 2 |
| 실패 시 부분 변경 없음 | PASS | PASS | PASS |
| 잠금 해제 뒤 원래 명령 성공·재전송 효과1회 | PASS | PASS | PASS |

이 효과는 로컬 API의 진단 계약 개선이다. 실제 사람 시간 절감, 재시도 감소, Mission 효과, 다른 프로젝트 전이는 UNKNOWN이다. 두 사례를 통계적 신뢰도나 전체 오류 대응률로 해석하지 않는다.

## 실제 한 작업의 완주

작업 ID `ba515e0a-d7e4-4625-ba6f-8b810100747f`. 설치된 CLI→HTTP→SQLite로 등록했다.

| revision | 실제 동작 |
| --- | --- |
| 1–2 | 고정 계약 create, 재현 근거 baseline 기록 |
| 3–5 | 후보 retarget, 실제 HTTP 검증 observe, verified |
| 6 | main 반영·설치·API 재시작 후 applied |
| 7–9 | 검토에서 발견한 N/A 상태 보존 수정으로 retarget; 영향 없는 오류 분류 HTTP 검사의 reuse 사유, 최종 native 검사, verified |
| 10 | 최종 main·설치 동일성·재시작 근거로 applied |
| 11–13 | 적용 후 동일 사례의 새로운 effect observe, evaluated PASS, complete |

최종 GET은 completed=true, phase=evaluated PASS, blockers=[]다. Python sqlite3로 event13개·revision13·target active runs0을 별도 조회했다. SQLite backup API로 실험 DB 사본을 만들었고 integrity_check 및 이 작업에 등록된 근거 파일 SHA를 모두 대조했다.

## 검증·설치

- Elixir21 PASS: 기존 상태/동시성·자원/원자성/인증14, 새 개선 단계6, 실제 외부 SQLite lock 분류1. DB 미설정 bootstrap3 PASS(당시 DB 통합17 skip).
- Python26 PASS, Rust20 PASS, 규칙 registry 구조 PASS. changed skill2개 validator PASS, format/diff 확인 완료.
- installer dry-run48files 검토 후 apply/check. coordinator9개, skill11개. 전역 Codex config SHA 동일. 새로운 모델 배치나 자동 role discovery를 검증했다고 주장하지 않는다.
- 루프·DB 오류 처리의 입력 제한/SQL parameterization/기존 auth 유지/근거 참조 미실행/재시도 미추가를 코드에서 검토했다. 별도 모델 리뷰 및 전체 보안 스캔은 수행하지 않았다. 구현·검사·실험을 같은 에이전트가 작성한 한계는 남는다.

## 실패와 남은 경계

초기 효과 조회의 nil/map boolean 처리를 수정한 뒤 단계 검사를 통과했다. 이후 N/A를 NOT_RUN으로 표시하는 경계를 발견해 상태를 보존하고 최신 verification FAIL이면 완료가 차단되는 검사도 추가했다. 이 변경 때문에 재검증/재적용한 것이며 오류 분류 HTTP는 코드·경로·클라이언트가 그대로임을 확인해 사유를 남겨 재사용했다. 적용 후 효과는 새로 실행했다. 시스템 Python의 YAML 모듈이 없어 validator dependency를 임시 경로에 설치했다. 최초 migration은 잘못된 cwd에서 실패하여 앱 경로로 바로잡았다.

자동 rollback·운영 장애 복구·백업 restore·장기 관측·프로젝트별 설치 격리·공통화는 NOT_RUN/미구현이다. 공유 token/actor 라벨은 개별 에이전트 인증이 아니다. 근거 SHA 일치는 진실성이나 승인 증명이 아니다. 로컬 설치와 실험 API 재시작만 수행했고 테스트 서버4121은 종료했다. 새 스킬은 명시적으로 요청한 태극 개선에서 사용할 수 있다.

사용 계약: [개선 스킬](../codex/skills/taeguk-self-improve/SKILL.md), [단계 요청](../codex/skills/taeguk-self-improve/references/cycle.md), [검증 계획](self-improvement-plan.md). 실험 근거와 재현 코드: `/Users/tonton/Documents/study/experiments/taeguk-self-improvement-20261011/`의 baseline.json, candidate.json, effect.json, cycle-events.json, readback.json, cycle.sqlite3, probe.py, cycle.py 및 검사 로그. 이 경로는 작성자 로컬 보관 위치이며 저장소에 포함하지 않았다. clone한 환경에서는 이 근거를 확인할 수 없다.

# Self-improvement 루프 작업 계약과 검증 계획

기준 버전 b6209cc. BOSS 요청: 회고에서 실제 개선 과제를 찾고 수정·검증·적용·효과 관찰까지 완주한다. 목적은 반복된 태극 문제를 설명으로만 남기지 않고 재현 가능한 작은 변경으로 해결하는 것이다. 규칙 개정·운영 배포·외부 메시지·자동 에이전트 호출은 범위 밖이다.

## 첫 문제와 oracle

직전 SQLite 실제 HTTP 검사에서 외부 연결이 BEGIN IMMEDIATE를 잡으면 503 database_operation_failed가 나왔다. 잠금 경합이 알려져도 일반 DB 실패와 구분하지 못했다. 이번 가설: 알려진 잠금 경합을 database_busy로 구분하면 실행자가 먼저 소유자/실행 기록을 확인할 수 있다. 자동 retry를 추가하지 않는다. 일반 SQL 오류는 여전히 database_operation_failed여야 한다. 시간 절감·실제 사람 부담 감소는 이번 검사만으로 주장하지 않는다.

## 설계

기존 work item에 선택적 improvement 계약을 추가한다. source_ref/problem_ref/baseline_ref/rollback_ref와 검증 check ID, 효과 check ID를 생성 때 고정한다. 일반 작업에는 변화가 없다. 기존 revision/CAS/재전송/append event를 사용하고 별도 engine/DB를 만들지 않는다.

에이전트는 새로운 self-improve 스킬로 발견→baseline 기록→작은 변경→검증→적용→효과 기록을 실행한다. API가 순서와 근거·현재 context를 검사한다. baseline은 PASS(재현 확인) 또는 UNKNOWN/FAIL을 기록할 수 있으나 PASS만 다음 단계로 간다. verified는 지정된 검증 check들이 현재 context에서 통과해야 한다. applied는 검증된 context를 그대로 적용한 기록이다. evaluated는 적용 이후 작성된 효과 check를 요구하고 현재 상태를 저장한다. 평가 실패/미확인은 기록 가능하지만 완료는 PASS일 때만 가능하다. 실패 시 기존 retarget으로 baseline artifact에 돌아가고 새 검증한다; API는 실제 Git/프로세스 복원을 실행하지 않는다.

retarget은 실행 중 run이 없어야 하며 candidate/applied/evaluated 기록을 무효화하고 baseline 단계로 돌아간다. 기존 이벤트는 보존한다. 관측 변경 후 완료 때 검증/효과를 다시 판단한다. 단계 기록은 증거 진실성이나 적용의 실제 발생을 인증하지 않는다.

## 검사

- I01 baseline을 실제 SQLite 잠금 HTTP로 재현하고 SQL revision/이벤트가 변하지 않는지 대조한다.
- I02 단계 건너뛰기·미등록 check·검증 누락·stale context·FAIL/UNKNOWN 효과·재전송·retarget 후 이전 stage 승계 차단을 native 통합으로 검사한다.
- I03 후보는 잠금503(database_busy), 일반 DB 오류503(database_operation_failed), 해제 후 원래 명령 성공을 실제 HTTP와 독립 Python sqlite3로 확인한다. retry 효과 중복이 없어야 한다.
- I04 검증된 후보를 설치/재시작한 뒤 동일 잠금 fixture와 일반 오류 fixture를 반복한다. 분류 오류는 baseline2case 중1→after2case 중0을 기대한다. 소요 시간이나 사용자 만족은 측정하지 않는다.
- I05 루프의 baseline/verified/applied/evaluated/complete 이벤트를 실제 CLI→HTTP→SQLite에서 관찰하고 기록한다.
- I06 기존 Python/Rust/Elixir 검사와 format, 규칙 registry, 변경 스킬 validator, 설치 dry-run/apply/check를 수행한다.

각 효과는 exact artifact/environment/plan과 근거 SHA에 연결한다. 기준을 수정해서 통과시키지 않는다. 이 단계의 코드/테스트/실험은 같은 에이전트가 작성하며 HTTP/SQLite 교차 관찰을 독립 모델 리뷰로 표현하지 않는다.

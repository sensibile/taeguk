# Taeguk 검증 계획 중심 흐름의 실행 검증 계획

## 작업 계약과 상태

BOSS가 승인한 목적은 실제 제품 작업에서 원래 목적·요구를 설계, 구현, 검증, 수정에 걸쳐 잃지 않고 확인 부담을 줄이는 것이다. 이번 변경은 그 흐름을 쓰기 위한 로컬 역할·스킬·설치 문서의 구성 보완이다. 제품 효과 개선은 아직 가설이다. 원 요구는 [설계의 WF-01~08](verification-workflow-design.md)과 이 작업의 사용자 지시이며, 설계만을 새 수용 기준으로 삼지 않는다.

계획 ID는 `VWP-2026-10-11`, 버전은 이 파일의 Git blob/commit으로 식별한다. 계획 입력은 설계 commit `dec6010e98324f0d46dbde8eb291083aed1a64e2`, 기준 소스 `37ad82416a29252631899a17773c22de56ba3373`, 승인된 규칙 registry 1.0의 GOV-001·002·004, REQ-001·006, VER-003·004, MET-001, DEV-002, REL-005다. 해당 본문은 `docs/rules/`이 원본이다. 계획 이후 구현 대상 commit, 실제 설치 대상, Codex 세션, 환경 버전은 **미정**이다. 담당자는 검사 때 정확한 commit·파일 hash·환경·증거 위치를 기입한다.

이번 계획의 모든 검사 상태는 `NOT_RUN`이다. 계획 작성은 구현, 검사 실행, 설치, 새 역할 호출, 제품 효과 관측을 뜻하지 않는다. 이 계획자와 Tool verifier의 실행 범위는 로컬 소스와 임시 home 검사다. 검토를 마친 관리 bundle의 실제 사용자 home 설치는 root 담당 단계로 분리하며 V-12에서 확인한다. 전역 Codex 설정, 앱·DB, 외부 전송·운영 변경은 포함하지 않는다. 승인된 검사 적용과 구현 기본값은 담당자가 자율 결정한다. 요구 의미·임계값·예외·실행 권한의 변경은 BOSS 확인 전까지 미결정으로 남긴다. 이 계획은 새 전역 차단 규칙이나 수치 임계값을 만들지 않는다.

## 검사와 근거

> 2026-10-25 변경: 역할별 모델 고정이 해제되어 V-02의 모델·effort 기대값은 현재 구성에 적용하지 않는다. 이 계획은 `VWP-2026-10-11` 실행 기록으로 보존한다.

각 항목은 구현 전 oracle을 고정한다. `도구`는 파일·프로세스·수치의 관측, `LLM`은 원 요구와 변경 의미의 대조, `혼합`은 해석 가설을 별도 도구·재현으로 확인하는 방식이다. 구현자는 이 계획 버전을 입력으로 기록한다. Tool verifier와 LLM reviewer는 **같은 최종 구현 commit**을 검토하고 결과마다 원시 증거 및 확인하지 못한 범위를 남긴다.

| ID·방식·담당 | 요구와 기준/근거 | 실제 확인 방법과 범위 | 예상 누락·상태 |
| --- | --- | --- | --- |
| V-01 · LLM · `taeguk_llm_reviewer` | WF-01·03, REQ-001·006: 원 사용자 지시와 설계 WF-01~08. 목적·원 요구·보존 조건을 계획 항목에 추적하고 구현 인계 전에 고정한다. | 사용자 지시, 설계, 이 계획, 구현 인계와 최종 변경을 나란히 읽어 각 WF의 주장·기준·근거·범위·누락·상태가 연결되는지 대조한다. 설계에 없는 원 요구도 확인한다. | 문서 간 일치만으로 실제 사용 효과는 알 수 없다. `NOT_RUN`: 의미 대조 미실행. |
| V-02 · 도구 · `taeguk_tool_verifier` | WF-05: BOSS의 모델 배치와 [설계의 역할 표](verification-workflow-design.md). 설정 필드는 공식 [Custom agents](https://learn.chatgpt.com/docs/agent-configuration/subagents?surface=app)의 `model`, `model_reasoning_effort`. | 최종 commit의 역할 TOML을 `tomllib` 등으로 파싱한다. designer=`gpt-6-astra`/`medium`, verifier=`gpt-6-sol`/`medium`, implementer=`gpt-6-luna`/effort 키 없음, tool verifier=`gpt-6-luna`/effort 키 없음, LLM reviewer=`gpt-6-astra`/`low`를 실제 필드 값으로 대조한다. 기존 세 역할의 모델·effort와 전역 설정 diff도 확인한다. | 정적 TOML 값은 실제 모델 라우팅·호출 성공 증거가 아니다. `NOT_RUN`: 최종 TOML 미작성. |
| V-03 · 혼합 · 두 검증자 | WF-01·02·04: 사용자 흐름과 설계 단계별 인계. 기존 10개 스킬과 evidence packet·check-selection을 재사용한다. | LLM reviewer가 역할·스킬 지침에서 `DESIGN_HANDOFF` → 구현 전 계획 → 같은 계획 버전의 구현·검사 → 이슈 ID별 수정·재검증 경로를 추적한다. Tool verifier는 최종 파일에서 참조 경로·역할명·스킬 수와 실제 존재, installer bundle 수집을 대조한다. 누락 경로는 파일 관측으로 확인한다. | 문구와 파일 존재만으로 실제 위임·순서 준수는 증명되지 않는다. `NOT_RUN`: 구현 전. |
| V-04 · LLM · `taeguk_llm_reviewer` | WF-02·03, VER-003·004, REQ-006: 도구·LLM·혼합 책임, 원래 oracle, 상태·누락, 독립성 한계를 분리한다. | 최종 지침과 이 계획을 읽어 구현 결과에서 기대값을 역산하도록 유도하지 않는지, 역할/모델 차이를 독립성으로 선언하지 않는지, `PASS`·`FAIL`·`UNKNOWN`·`NOT_RUN`·`NOT_APPLICABLE`의 근거와 실제 관측이 분리되는지 검토한다. 같은 모델이 만든 테스트의 공유 가정도 명시한다. | LLM 판단은 실행 관측이 아니다. `NOT_RUN`: 최종 변경 검토 미실행. |
| V-05 · 도구 · `taeguk_tool_verifier` | WF-07, GOV-001·002·004: 기존 승인 규칙 본문·registry, 무관 역할, 전역 설정을 보존한다. | 기준 commit과 최종 commit의 누적 diff 및 각 commit을 확인한다. `docs/rules/`, `registry.toml`, 무관 역할 TOML, CLI·API·DB·전역 config의 변경 여부를 비교한다. 변경이 있다면 승인 근거와 의미 차이를 LLM reviewer에게 보낸다. | 파일 diff에 없는 외부 설정 변화는 보지 못한다. `NOT_RUN`: 최종 diff 미존재. |
| V-06 · 도구 · `taeguk_tool_verifier` | WF-07: 현행 installer의 `taeguk_*.toml` 자동 수집, receipt/hash·비관리 충돌·사용자 수정 보호. | 최종 소스에서 `python3 scripts/install_codex.py --home <isolated-home>` dry-run으로 변경 파일 목록을 확인한다. 기존 Python 설치 테스트와, 임시 home에 한정한 apply → check → 수정/비관리 충돌 거부 → uninstall을 검사한다. 역할 TOML의 설치 사본과 소스 bytes/hash를 대조한다. 이 단계에서 실제 사용자 home은 쓰지 않고 V-12로 인계한다. | receipt/hash는 복사 상태만 증명한다. 경합·강제 종료 전체 복구와 새 세션 발견은 별도. `NOT_RUN`: 설치 검사 미실행. |
| V-07 · 도구 · `taeguk_tool_verifier` | WF-01·07: AGENTS.md의 로컬 gate 및 변경 범위 회귀. | 최종 commit에서 `python3 scripts/check_rules.py`, Python unittest, Rust locked test, `apps/taeguk`의 `mix test`·`mix format --check-formatted`를 실행해 exit와 원시 로그를 남긴다. 변경 스킬은 system skill-creator validator로 검사한다. 문서의 UTF-8/LF, H1, 마지막 개행, 공백, 닫힌 fence, 상대 링크도 검사한다. 검사 환경 오류는 제품 FAIL과 구분한다. | 통과해도 요구 의미·승인 진실성·제품 효과는 증명되지 않는다. `NOT_RUN`: gate 미실행. |
| V-08 · 혼합 · 두 검증자 | WF-06, GOV-001·002·004: 기본값·검사 적용은 승인 범위에서 자율; 의미·임계값·예외·권한은 BOSS 확인. | LLM reviewer가 지침 문장과 예시에서 승인 경계가 흐려지는 사례를 제안한다. Tool verifier가 발견한 사례의 최종 파일 위치, 기준 버전과 diff를 확인한다. 기준 변경처럼 보이면 승인 참조가 있을 때만 인정하며 없으면 `UNKNOWN` 또는 `FAIL`의 이유를 구체화한다. | TOML 지침 자체는 권한 집행 장치가 아니다. `NOT_RUN`: 최종 지침 미검토. |
| V-09 · 혼합 · 두 검증자 | WF-08: [CLI 계약](architecture.md)은 audit를 선택적 ID/hash 연결 점검으로 제한한다. | LLM reviewer가 문서·스킬의 CLI 설명에 전체 완료·실행 진실성·의미 판정 주장이 없는지 찾는다. Tool verifier는 변경된 경우 기존 audit 사례/테스트로 연결·hash 범위를 확인하고 CLI 소스 diff를 살핀다. 발견된 초과 주장은 원문 위치와 실제 CLI 출력을 대조한다. | audit 성공은 검사 실행이나 사용자 효과의 증거가 아니다. `NOT_RUN`: 최종 변경 미검토. |
| V-10 · 도구 · `taeguk_tool_verifier` | WF-04, REQ-006: 검사 결과는 정확한 대상 버전·환경에 귀속한다. | 결과 packet에 계획 commit, 구현 commit, 적용 규칙 버전, 명령, exit, 관측 값, 원시 로그 경로, 데이터·환경 버전, 한계가 있는지 대조한다. 수정 commit이 생기면 영향 항목을 다시 실행해 새 증거를 기록한다. 선택적으로 `taeguk audit`로 ID/hash 연결을 검사한다. | 자동 hash 대조는 내용·상태 진실성을 인증하지 않는다. `NOT_RUN`: 결과 packet 미생성. |
| V-11 · LLM · `taeguk_llm_reviewer` | WF-01·04, REL-005: 구성 완료와 실제 제품 효과는 다르다. | 원 요구별 충족·부분 충족·미실행을 V-01~10과 최종 산출물에 연결한다. 기존 실제 제품 파일럿의 사용 관측이 없다면 효과는 미확인으로 명시한다. 수정할 경우 이슈 ID·영향·재검증 항목을 verifier/implementer에게 돌려준다. | 이번 구성 변경만으로 시간 절감·품질 향상·역할 발견을 주장할 수 없다. `NOT_RUN`: 최종 검토 미실행. |
| V-12 · 도구 · root | WF-07, REL-005: BOSS가 지정한 설정·설치 후 같은 과제 재실행 순서. 설치 원본은 V-06과 검토가 끝난 최종 관리 bundle이다. | root가 V-06 및 검토 결과를 받은 뒤 실제 사용자 home에 대해 dry-run → apply → check를 수행하고 파일 목록·receipt/hash·exit·설치 원본 commit을 기록한다. 적용 전후 기존 전역 Codex config와 무관 파일의 보존을 확인한다. 충돌이나 변경된 관리 파일이 있으면 installer 보호 결과를 기록하고 강제 덮어쓰지 않는다. | 설치 일치는 새 세션 역할 발견·실제 모델 호출·Veloce 기능 효과의 증거가 아니다. `NOT_RUN`: root 설치 단계 전. |

V-01~12는 이번 변경을 검사하는 작업 항목이다. 특정 테스트 수나 신규 전역 필수 기준을 뜻하지 않는다. 실행 후 각 항목은 실제 관측과 이유를 붙여 `PASS`, `FAIL`, `UNKNOWN`, `NOT_RUN`, `NOT_APPLICABLE` 중 하나로 갱신한다. `NOT_APPLICABLE`에는 승인된 범위와 적용되지 않는 근거가 필요하다. `UNKNOWN`은 기준 부족, 증거 부족, 해석 충돌을 구별한다. 검사 명령이 exit 0인 것과 제품 판정 `PASS`도 구별한다.

## 관측의 순서와 재검증

1. Implementer는 이 계획의 commit/blob, 설계 commit, 적용 규칙 버전과 구현 시작 commit을 인계에 적는다. 실제 경계가 설계와 다르면 근거와 영향받는 V 항목을 반환한다. 원래 요구 의미를 바꾸는 조정은 구현자가 승인한 것으로 처리하지 않는다.
2. Tool verifier는 최종 구현 commit을 고정하고 V-02·03·05~10의 실제 명령·환경·출력·실패를 기록한다. 필요한 검사만 선택하되 미실행 항목은 이유를 남긴다. V-07의 기존 gate가 무관한 환경 문제로 막히면 원시 로그와 재현 범위를 보존한다.
3. LLM reviewer는 같은 commit과 원 사용자 요구, 설계, 계획, 도구 원시 증거를 보고 V-01·03·04·08·09·11을 판단한다. 도구 결과에 의존하는 결론은 그 결과를 받은 뒤 낸다. 서로 다른 모델 사용은 독립성 증거가 아니다. 공유 문서·가정·생성 테스트를 명시한다.
4. 불일치나 실패는 `WF-ID/V-ID`, 실제 관측, 기대 근거, 영향 범위, 제안 수정으로 반환한다. Implementer의 수정 뒤 verifier가 변경 파일과 의존 항목을 정해 재검증한다. 관련 commit·환경·기준이 바뀌면 이전 `PASS`를 자동 승계하지 않는다. 기준 변경은 BOSS 확인을 기다린다.
5. Coordinator는 요구별 결과와 설치·전달·실제 효과를 따로 보고한다. root는 검토를 마친 관리 bundle을 V-12에 따라 실제 사용자 home에 설치한다. 이 계획 자체의 `NOT_RUN` 상태에서 완료를 선언하지 않는다. BOSS가 지정한 이어지는 Veloce 같은 과제 재실행은 별도 제품 검증 계획으로 실제 기능 효과를 확인하는 후속 승인 작업이다. 새 세션의 역할 발견·모델 호출·제품 효과는 해당 단계의 실제 관측으로 확인한다.

## IMPLEMENTER_HANDOFF

입력은 사용자 원 요구, [설계](verification-workflow-design.md) commit `dec6010e98324f0d46dbde8eb291083aed1a64e2`, 이 계획의 최종 commit과 WF/V 매핑이다. 역할·스킬·문서의 작은 변경만 구현하고 기존 10개 스킬, installer 수집과 충돌 보호, 승인 규칙 의미를 보존한다. 구현 결과에는 정확한 commit, 변경 목록, 자체 검사 결과, 계획과의 편차, 미확인·후속 항목을 붙인다. 자체 검사만으로 검증자의 결론을 대신하지 않는다.

## TOOL_VERIFIER_HANDOFF

이 계획의 V-02·03·05~10을 담당한다. 최종 commit을 먼저 고정하고 계획과 구현의 버전을 함께 기록한다. TOML/installer/임시 home/기존 gate와 문서 검사를 실제로 수행하고 원시 출력·exit·환경을 남긴다. V-06의 결과와 관리 bundle 식별자를 root의 V-12 설치 단계로 인계한다. 도구 오류, 파일 동일성, 검사 판정을 구별한다. LLM 판단이 필요한 초과 주장이나 규칙 의미 변경 후보는 위치와 관측을 LLM reviewer에게 전달한다.

## LLM_REVIEWER_HANDOFF

이 계획의 V-01·03·04·08·09·11을 담당한다. 원 요구와 승인 근거에서 출발해 설계·최종 변경·도구 증거를 대조한다. 누락된 답, 기대값의 사후 변경, 공유 가정, 승인 경계, 수정·재검증 경로를 검토한다. 해석은 해석으로 표기하고 재현이 필요한 주장은 V-03·08·09의 도구 관측에 연결한다. 제품 효과와 실제 역할 실행은 증거가 없으면 미확인으로 남긴다.

## ROOT_INSTALL_HANDOFF

V-06과 최종 검토의 결과를 받은 root가 V-12를 수행한다. 검토된 bundle의 정확한 commit과 기존 사용자 home의 관리 상태를 연결해 dry-run → apply → check, 전역 config·무관 파일 보존을 기록한다. 설치 결과와 새 세션 발견·모델 실행을 구분한다. 이어지는 Veloce 재실행의 실제 기능 효과는 별도 제품 검증 계획에서 원 요구와 관측 조건을 정해 확인한다.

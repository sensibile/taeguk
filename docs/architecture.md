# 구성과 실행 계약

## 책임

에이전트는 맥락·대안·다음 행동, 스킬은 반복 절차, 도구는 계산·실행 결과·상태 조회를 맡는다. CLI는 계산·증거 대조와 로컬 상태 API 요청을 수행한다. SQLite API는 작업 revision과 근거·실행 소유권을 저장한다. 프로세스의 실제 실행·종료나 모델 호출을 관리하는 workflow engine은 구현하지 않았다.

흐름 역할은 `taeguk_coordinator` → `taeguk_designer` + `taeguk_verification_builder`(설계 중 검증 계획·도구 범위) → `taeguk_verifier`(계획 검토·확정) → `taeguk_implementer`와 필요한 검증 도구 제작 → `taeguk_tool_verifier`와 `taeguk_llm_reviewer` → 수정 및 verifier가 정한 재검증이다. 구현자와 검토자는 같은 버전의 계획과 정확한 산출물 버전을 사용한다. `taeguk_structure_reviewer`, `taeguk_outcome_observer`, `taeguk_rule_reviewer`는 해당 작업에서 계속 선택해 사용한다. 구성이 역할 생성을 자동 예약하거나 위임하는 것은 아니다.

Designer는 `gpt-6-astra` medium, verifier는 `gpt-6-sol` medium, implementer와 tool verifier는 `gpt-6-luna`(effort 미지정), LLM reviewer는 `gpt-6-astra` low를 사용한다. 나머지 역할과 전역 Codex 설정은 기존 설정을 상속한다. 역할 설정은 모델 지정이며 실제 호출이나 권한 강제를 증명하지 않는다.

검증 계획은 원 요구와 승인된 oracle을 각 검사, 방법(도구·LLM·혼합), 근거, 범위, 누락, 상태에 연결한다. 도구 관측, 의미 판단, 재현이 필요한 추론을 구분한다. 계획 참조와 CLI 증거 연결 검사는 기준의 진실성이나 전체 완료를 판정하지 않는다.

## 교환할 정보

목적 가설, 승인 요구·규칙 버전, 대상 버전, 작업 범위·보존 조건, 기대값 근거, 실행 기록·원시 증거, 외부 효과, 한계와 다음 행동을 함께 전달한다. 변경이나 재빌드 뒤 낡은 결과를 재사용하지 않는다.

## CLI

`taeguk measure counts.json`은 `good`, `bad`, `unknown` 건수와 선택적인 `target`(0~1)을 읽어 전체와 가능한 성공률 범위, 판정 이유를 JSON으로 반환한다. 수집기 누락을 자동 발견하는 기능은 아니다. 입력 대상 집합의 신뢰성은 별도 증거가 필요하다. 총 건수는 정확한 정수 변환 범위인 2^53-1 이하로 제한하고 JSON 입력은 1 MiB까지만 읽는다.

`taeguk verify packet.json root`는 상대 경로 아티팩트의 SHA-256과 검사 상태 목록을 대조한다. 이는 로컬 파일 동일성 검사이며 승인 진실성, 결과의 독립성, 실제 배포물 동일성을 보장하지 않는다. root 밖 경로와 심볼릭 링크를 거부한다. 신뢰할 수 있는 로컬 디렉터리가 검사 중 바뀌지 않는 조건이며, 악의적인 동시 파일 교체를 막는 보안 경계는 아니다.

CLI 오류는 JSON stderr와 exit 2, 계산된 FAIL은 정상 JSON stdout과 exit 0이다. 실행 오류와 검사 판정은 서로 다르다. 빈 검사 목록이나 효과 미확인에는 PASS를 주지 않는다.

`taeguk audit review.json root`는 요구·검사 ID와 실제 파일 참조를 연결하는 선택적 점검이다. 전체 완료 판정을 출력하지 않는다. [입력과 한계](evidence-review.md)를 확인한다.

## 로컬 저장 경계

SQLite 상태 API는 고정된 작업/검증 계획과 현재 artifact/environment, 관측·run·명령 이벤트를 transaction으로 저장한다. 저장 파일은 단일 API가 소유하며 agent는 Rust CLI로 접근한다. [상태 API 계약](workflow-state-plan.md)과 [사용법](workflow-state-usage.md)을 따른다. 문서나 모델의 ‘완료’는 외부 효과의 확정을 대신하지 않는다.

개선 작업은 선택적 고정 계약과 baseline/verified/applied/evaluated 단계를 사용한다. 실제 수정과 측정은 에이전트, 순서·현재 근거·완료 gate는 API가 맡는다. [첫 완주 결과](self-improvement-result.md)에 구현 범위와 근거를 보존한다.

## 검증 수단과 재사용 경계

설계 인계는 검사별 기대값 출처·관측 방법과 기존 도구의 부족분을 포함한다. `taeguk_verification_builder`는 필요한 도구만 제작하고 정상·알려진 실패·경계 사례로 도구를 점검한다. 실행은 `taeguk_tool_verifier`, 증거 충분성 판단은 `taeguk_verifier`의 책임이다. 도구 자체 검증은 제품 검증을 대신하지 않는다.

`taeguk_structure_reviewer`는 새 요구 때문에 검증된 실행 영역까지 반복 수정하는 지점을 찾는다. 변하는 책임과 유지되는 계약에서 적합한 경계를 탐색한다. Policy·Workflow·Executor·Adapter는 예시이며 상태 전이, 입력 검증, 데이터 변환, 저장, 알고리즘, UI 등 다른 경계도 실제 변경 근거에 따라 검토한다. 특정 패턴 목록에 맞추거나 같은 모양의 기능을 무리하게 통합하지 않는다. 계약을 유지하는 영역의 재사용과 새 조합의 검증을 함께 계획하며, 영구 불변이나 자동 안전을 주장하지 않는다.

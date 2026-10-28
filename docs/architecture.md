# 구성과 실행 계약

## 책임

에이전트는 맥락·대안·다음 행동, 스킬은 반복 절차, 도구는 계산·실행 결과·상태 조회를 맡는다. CLI는 계산·증거 대조와 로컬 상태 API 요청을 수행한다. SQLite API는 작업 revision과 근거·실행 소유권을 저장한다. 프로세스의 실제 실행·종료나 모델 호출을 관리하는 workflow engine은 구현하지 않았다.

작업은 다음 단계로 진행한다. 2026-10-28 BOSS가 Phase 0~3 흐름을 제안했고, Phase 3 뒤 재검증, 반복 종료 조건, Phase 4를 보완한 안을 "오키 제안 괜찮네"로 확인했다. 단계 판단은 `taeguk_coordinator`가 맡는다.

| 단계 | 내용 | 역할 | 끝나는 조건 |
|---|---|---|---|
| Phase 0 아이디어 | 목적 가설과 의도·요구사항 기록(REQ-001, REQ-007) | `taeguk_coordinator` | 요구사항 의미를 BOSS가 확정 |
| Phase 1 계획 강화 (반복) | 설계와 검증 방안 확보, 부족한 검증 도구의 제작 범위(VER-004) | `taeguk_designer`, `taeguk_verification_builder`, `taeguk_verifier` | verifier가 모든 요구사항을 oracle·검사에 연결한 계획 버전을 확정 |
| Phase 1.5 구현 | 확정한 계획 버전에 맞춘 구현과 필요한 검증 도구 제작 | `taeguk_implementer`, `taeguk_verification_builder` | 정확한 산출물 버전을 검증·리뷰 역할에 인계 |
| Phase 2 검증·다각화 리뷰·수정 (반복) | 도구 검사, LLM 리뷰, 로컬 Codex 교차·보안 리뷰, 지적 수정과 영향 검사 재검증 | `taeguk_tool_verifier`, `taeguk_llm_reviewer`, `taeguk_codex_cross_reviewer`, `taeguk_codex_security_reviewer`, `taeguk_implementer` | 필수 검사 PASS, P0·P1 지적 0건, P2 이하는 반영하거나 반영하지 않은 이유를 기록 |
| Phase 3 코드 정리·구조 개선 (반복) | 실제 동반 수정 근거가 있는 경계만 개선(DEV-002) | `taeguk_structure_reviewer`, `taeguk_implementer` | 변경이 생기면 Phase 2로 돌아가 영향받는 검사를 재검증한 뒤 종료 |
| Phase 4 전달·효과 관측 | 머지·설치 등 전달 뒤 실제 사용자 효과를 목적 가설과 비교(REL-005), 규칙 개정 거리는 제안으로 분리 | `taeguk_outcome_observer`, `taeguk_rule_reviewer` | 관측 결과와 미확인 효과를 기록하고, 새 작업은 Phase 0에서 시작 |

반복 단계에는 종료 조건과 함께 상향 조건을 둔다. 같은 지적이 두 번 되풀이되거나 수정이 다른 지적을 계속 낳으면 반복을 멈추고 BOSS에게 상황과 선택지를 보고한다. 반복 횟수 상한 같은 수치 기준은 정하지 않았으며, 정하려면 BOSS 확인이 필요하다. Phase 3의 구조 변경은 Phase 2 증거를 이전 버전의 것으로 만들므로(REQ-006), 재검증 없이 전달하지 않는다. Phase 1 이후 요구사항 의미가 바뀌면 Phase 0으로 돌아가 BOSS 확인을 다시 받는다.

구현자와 검토자는 같은 버전의 계획과 정확한 산출물 버전을 사용한다. 표의 역할은 해당 작업에서 필요한 것만 선택한다. 문서만 바뀐 변경의 보안 리뷰처럼 대상이 없으면 NOT_APPLICABLE로 남긴다. 구성이 역할 생성을 자동 예약하거나 위임하는 것은 아니다.

역할 TOML은 `model`과 `model_reasoning_effort`를 지정하지 않는다. 모든 역할은 실행 환경의 모델과 전역 Codex 설정을 상속한다. 2026-10-25 BOSS 지시로 역할별 고정 배치를 해제했다. 고정 배치의 관측 이점이 일정하지 않았기 때문이다. 역할 정의는 실제 호출이나 권한 강제를 증명하지 않는다.

검증 계획은 원 요구와 승인된 oracle을 각 검사, 방법(도구·LLM·혼합), 근거, 범위, 누락, 상태에 연결한다. 도구 관측, 의미 판단, 재현이 필요한 추론을 구분한다. 계획 참조와 CLI 증거 연결 검사는 기준의 진실성이나 전체 완료를 판정하지 않는다.

## 로컬 Codex 검토

`taeguk_codex_cross_reviewer`와 `taeguk_codex_security_reviewer`는 [taeguk-codex-local-review](../codex/skills/taeguk-codex-local-review/SKILL.md) 절차로 로컬 `codex exec review`를 read-only sandbox에서 정확한 변경 범위에 실행한다. 2026-10-28 BOSS 지시로 이 CLI 호출은 `gpt-6-astra`, `medium` effort를 명령 인수로 지정한다. 역할 TOML의 모델 비고정 원칙은 그대로다. 교차 검토는 다른 에이전트가 구현한 변경의 정확성·요구 불일치를, 보안 검토는 실행 동작에 영향을 주는 변경의 취약점과 악용 시나리오를 본다. Codex 출력은 LLM 판단으로 분류하고 검토 역할이 각 지적을 재현·타당·기각·재현 필요로 평가한다. 지적 없음은 PASS가 아니다. 2026-10-28 GitHub의 `codex-review` gate를 없앤 뒤로는 이 로컬 리뷰가 PR 리뷰 단계다. 절차는 [저장소 운영](repository-operations.md)에 있다.

CLI 실행은 diff와 저장소 맥락을 OpenAI로 보낸다. 2026-10-28 BOSS가 Claude Code 세션에서 이 전송을 "태극 저장소 작업에선 상시 허용"으로 선택했다. AGENTS.md상 실행 권한 변경은 BOSS 확인 사항이며 이 선택이 그 확인이다. 범위는 `sensibile/taeguk` 작업뿐이고, 시험 프로젝트를 포함한 다른 저장소는 작업별 BOSS 승인이 없으면 NOT_RUN으로 남긴다. 전송 전 diff에 비밀값이 있으면 실행하지 않는다.

## 교환할 정보

목적 가설, 승인 요구·규칙 버전, 대상 버전, 작업 범위·보존 조건, 기대값 근거, 실행 기록·원시 증거, 외부 효과, 한계와 다음 행동을 함께 전달한다. 변경이나 재빌드 뒤 낡은 결과를 재사용하지 않는다.

## CLI

`taeguk measure counts.json`은 `good`, `bad`, `unknown` 건수와 선택적인 `target`(0~1)을 읽어 전체와 가능한 성공률 범위, 판정 이유를 JSON으로 반환한다. 수집기 누락을 자동 발견하는 기능은 아니다. 입력 대상 집합의 신뢰성은 별도 증거가 필요하다. 총 건수는 정확한 정수 변환 범위인 2^53-1 이하로 제한하고 JSON 입력은 1 MiB까지만 읽는다.

`taeguk verify packet.json root`는 상대 경로 아티팩트의 SHA-256과 검사 상태 목록을 대조한다. 이는 로컬 파일 동일성 검사이며 승인 진실성, 결과의 독립성, 실제 배포물 동일성을 보장하지 않는다. root 밖 경로와 심볼릭 링크를 거부한다. 신뢰할 수 있는 로컬 디렉터리가 검사 중 바뀌지 않는 조건이며, 악의적인 동시 파일 교체를 막는 보안 경계는 아니다.

CLI 오류는 JSON stderr와 exit 2, 계산된 FAIL은 정상 JSON stdout과 exit 0이다. 실행 오류와 검사 판정은 서로 다르다. 빈 검사 목록이나 효과 미확인에는 PASS를 주지 않는다.

`taeguk state verify-evidence WORK_ID root`는 상태 API에 등록된 현재 evidence의 SHA-256을 로컬 파일과 대조한다. 파일 동일성 검사이며 근거의 진실성이나 완료를 판정하지 않는다.

`taeguk audit review.json root`는 요구·검사 ID와 실제 파일 참조를 연결하는 선택적 점검이다. 전체 완료 판정을 출력하지 않는다. [입력과 한계](evidence-review.md)를 확인한다.

## 로컬 저장 경계

SQLite 상태 API는 고정된 작업/검증 계획과 현재 artifact/environment, 관측·run·명령 이벤트를 transaction으로 저장한다. 저장 파일은 단일 API가 소유하며 agent는 Rust CLI로 접근한다. [상태 API 계약](workflow-state-plan.md)과 [사용법](workflow-state-usage.md)을 따른다. 문서나 모델의 ‘완료’는 외부 효과의 확정을 대신하지 않는다.

개선 작업은 선택적 고정 계약과 baseline/verified/applied/evaluated 단계를 사용한다. 실제 수정과 측정은 에이전트, 순서·현재 근거·완료 gate는 API가 맡는다. [첫 완주 결과](self-improvement-result.md)에 구현 범위와 근거를 보존한다.

## 검증 수단과 재사용 경계

설계 인계는 검사별 기대값 출처·관측 방법과 기존 도구의 부족분을 포함한다. `taeguk_verification_builder`는 필요한 도구만 제작하고 정상·알려진 실패·경계 사례로 도구를 점검한다. 실행은 `taeguk_tool_verifier`, 증거 충분성 판단은 `taeguk_verifier`의 책임이다. 도구 자체 검증은 제품 검증을 대신하지 않는다.

`taeguk_structure_reviewer`는 새 요구 때문에 검증된 실행 영역까지 반복 수정하는 지점을 찾는다. 변하는 책임과 유지되는 계약에서 적합한 경계를 탐색한다. Policy·Workflow·Executor·Adapter는 예시이며 상태 전이, 입력 검증, 데이터 변환, 저장, 알고리즘, UI 등 다른 경계도 실제 변경 근거에 따라 검토한다. 특정 패턴 목록에 맞추거나 같은 모양의 기능을 무리하게 통합하지 않는다. 계약을 유지하는 영역의 재사용과 새 조합의 검증을 함께 계획하며, 영구 불변이나 자동 안전을 주장하지 않는다.

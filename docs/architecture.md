# 구성과 실행 계약

## 책임

에이전트는 맥락·대안·다음 행동, 스킬은 반복 절차, 도구는 계산·실행 결과·상태 조회를 맡는다. CLI는 계산·증거 대조와 로컬 상태 API 요청을 수행한다. SQLite API는 작업 revision과 근거·실행 소유권을 저장한다. 프로세스의 실제 실행·종료나 모델 호출을 관리하는 workflow engine은 구현하지 않았다.

작업은 Phase 0 아이디어, Phase 1 계획 강화(반복), Phase 1.5 구현, Phase 2 검증·다각화 리뷰·수정(반복), Phase 3 변경 범위 정리(반복), Phase 4 전달·효과 관측으로 진행한다. 단계별 역할, 종료·상향 조건, 되돌아가기는 설치되는 [작업 단계](../codex/skills/taeguk-work-contract/references/phases.md)에 있다. 2026-10-28 BOSS가 확인했다. 작업 범위 밖의 정리는 매일 도는 데일리 정리가, 구조 개선은 시그널이 관측될 때 BOSS 승인을 받아 시작하는 작업이 맡는다(2026-10-29 BOSS 확인). 테스트가 비는 곳은 매일 정리 다음에 도는 테스트 보강이 커버리지로 찾아, 결함 주입으로 확인된 곳에만 테스트를 더한다(2026-10-29 BOSS 확인). 두 주기 작업은 승인을 기록한 적용 프로젝트(archon부터)에서도 돈다.

구현자와 검토자는 같은 버전의 계획과 정확한 산출물 버전을 사용한다. 단계 표의 역할은 해당 작업에서 필요한 것만 선택한다. 문서만 바뀐 변경의 보안 리뷰처럼 대상이 없으면 NOT_APPLICABLE로 남긴다. 구성이 역할 생성을 자동 예약하거나 위임하는 것은 아니다.

역할 TOML은 `model`과 `model_reasoning_effort`를 지정하지 않는다. 모든 역할은 실행 환경의 모델과 전역 Codex 설정을 상속한다. 2026-10-25 BOSS 지시로 역할별 고정 배치를 해제했다. 고정 배치의 관측 이점이 일정하지 않았기 때문이다. 역할 정의는 실제 호출이나 권한 강제를 증명하지 않는다.

검증 계획은 원 요구와 승인된 oracle을 각 검사, 방법(도구·LLM·혼합), 근거, 범위, 누락, 상태에 연결한다. 도구 관측, 의미 판단, 재현이 필요한 추론을 구분한다. 계획 참조와 CLI 증거 연결 검사는 기준의 진실성이나 전체 완료를 판정하지 않는다.

## 로컬 Codex 검토

`taeguk_codex_cross_reviewer`와 `taeguk_codex_security_reviewer`는 [taeguk-codex-local-review](../codex/skills/taeguk-codex-local-review/SKILL.md) 절차로 로컬 `codex exec review`를 read-only sandbox에서 정확한 변경 범위에 실행한다. 2026-10-29 BOSS 지시로 이 CLI 호출은 `gpt-6-sol`, `high` effort를 명령 인수로 지정한다(비용 때문에 2026-10-28의 `gpt-6-astra`, `medium`을 대체). 역할 TOML의 모델 비고정 원칙은 그대로다. 2026-10-29 BOSS 지시로 Codex 리뷰는 작업 세션의 자체 리뷰 에이전트가 반복 리뷰해 더 고칠 것이 없게 된 head에만 보낸다. 교차 검토는 다른 에이전트가 구현한 변경의 정확성·요구 불일치를, 보안 검토는 실행 동작에 영향을 주는 변경의 취약점과 악용 시나리오를 본다. 2026-10-30부터 프롬프트에 목적 맥락과 이전 판정을 넣고 프롬프트 파일을 증거로 남긴다. 목적 이의는 BOSS에게 넘기고, 목적이나 정답 기준이 기록에 없으면 NOT_RUN이다. Codex 출력은 LLM 판단으로 분류하고 검토 역할이 각 지적을 재현·타당·기각·재현 필요로 평가한다. 지적 없음은 PASS가 아니다. 2026-10-28 GitHub의 `codex-review` gate를 없앤 뒤로는 이 로컬 리뷰가 PR 리뷰 단계다. 절차는 [저장소 운영](repository-operations.md)에 있다.

CLI 실행은 diff와 저장소 맥락을 OpenAI로 보낸다. 2026-10-28 BOSS가 Claude Code 세션에서 이 전송을 "태극 저장소 작업에선 상시 허용"으로 선택했다. AGENTS.md상 실행 권한 변경은 BOSS 확인 사항이며 이 선택이 그 확인이다. 범위는 `sensibile/taeguk` 작업이다. 2026-10-29 BOSS가 `sensibile/archon` 작업에도 같은 조건의 상시 허용을 골랐다. 그 밖의 저장소(다른 시험 프로젝트 포함)는 작업별 BOSS 승인이 없으면 NOT_RUN으로 남긴다. 전송 전 diff에 비밀값이 있으면 실행하지 않는다.

## 교환할 정보

목적 가설, 승인 요구·규칙 버전, 대상 버전, 작업 범위·보존 조건, 기대값 근거, 실행 기록·원시 증거, 외부 효과, 한계와 다음 행동을 함께 전달한다. 변경이나 재빌드 뒤 낡은 결과를 재사용하지 않는다.

## CLI

`taeguk measure counts.json`은 `good`, `bad`, `unknown` 건수와 선택적인 `target`(0~1)을 읽어 전체와 가능한 성공률 범위, 판정 이유를 JSON으로 반환한다. 수집기 누락을 자동 발견하는 기능은 아니다. 입력 대상 집합의 신뢰성은 별도 증거가 필요하다. 총 건수는 정확한 정수 변환 범위인 2^53-1 이하로 제한하고 JSON 입력은 1 MiB까지만 읽는다.

`taeguk verify packet.json root`는 상대 경로 아티팩트의 SHA-256과 검사 상태 목록을 대조한다. 이는 로컬 파일 동일성 검사이며 승인 진실성, 결과의 독립성, 실제 배포물 동일성을 보장하지 않는다. root 밖 경로와 심볼릭 링크를 거부한다. 신뢰할 수 있는 로컬 디렉터리가 검사 중 바뀌지 않는 조건이며, 악의적인 동시 파일 교체를 막는 보안 경계는 아니다.

CLI 오류는 JSON stderr와 exit 2, 계산된 FAIL은 정상 JSON stdout과 exit 0이다. 실행 오류와 검사 판정은 서로 다르다. 빈 검사 목록이나 효과 미확인에는 PASS를 주지 않는다.

`taeguk state verify-evidence WORK_ID root`는 상태 API에 등록된 현재 evidence의 SHA-256을 로컬 파일과 대조한다. 파일 동일성 검사이며 근거의 진실성이나 완료를 판정하지 않는다.

`taeguk graph build ROOT --out GRAPH.json [--lang elixir|go|python|rust]`는 언어별 기존 도구로 정적 의존 그래프를 만든다. Elixir는 기존 빌드의 compile manifest를 `elixir`로 읽고(`mix`와 `mix.exs`는 실행하지 않음), Go는 `go list -e -deps -json`, Rust는 `cargo metadata --no-deps`와 `syn` 기반 `mod`·`use`·경로 해석, Python은 표준 라이브러리 `ast`를 쓴다. 결과에는 commit, 미커밋 변경 경로, 도구 버전·명령, 해석하지 못한 사례(gap), 재실행 비교용 digest가 들어간다. 분석기 하나라도 실패하면 그래프는 FAIL이고 그 언어의 노드를 넣지 않는다. 대상 저장소에 compile·의존성 설치를 하지 않으며, 네트워크 조회도 끈다. `taeguk graph impact GRAPH.json ROOT (--changed PATH... | --git-diff BASE) [--depth N]`는 변경 파일에 의존하는 노드를 거리와 함께 반환한다. 이 그래프는 구조 분석 보조 도구라 impact는 통과·불통과를 판정하지 않는다. 그래프가 현재 HEAD가 아니거나, dirty 상태에서 만들어졌거나, 질의 시점 작업 트리가 HEAD와 다르거나, FAIL이거나, 변경 파일이 그래프에 없으면 `warnings`에 이유를 남긴다. edge가 없다는 것은 영향이 없다는 증거가 아니다. 분석기가 해석하지 못한 지점은 gap으로 남고, impact는 영향 범위 밖 파일의 gap을 확인해 볼 곳(`check_also`)으로 낸다. gap 판정은 허용 목록 방식이다. 분석기는 대상 프로젝트 코드를 실행하지 않고, 각 분석기는 자식 프로세스로 격리되어 시간 제한 안에서 실행되므로, 대상 저장소 내용이 분석기를 멈추거나 비정상 종료시켜도 그 언어의 FAIL로 끝난다. 이 도구의 변경·리뷰에서 무엇을 결함으로 볼지는 [코드 그래프 작업 계약](code-graph-plan.md)의 결함 판정 기준을 따른다. 설계와 검증 근거는 [코드 그래프 작업 계약](code-graph-plan.md)에 있다.

`taeguk audit review.json root`는 요구·검사 ID와 실제 파일 참조를 연결하는 선택적 점검이다. 전체 완료 판정을 출력하지 않는다. [입력과 한계](evidence-review.md)를 확인한다.

## 로컬 저장 경계

SQLite 상태 API는 고정된 작업/검증 계획과 현재 artifact/environment, 관측·run·명령 이벤트를 transaction으로 저장한다. 저장 파일은 단일 API가 소유하며 agent는 Rust CLI로 접근한다. [상태 API 계약](workflow-state-plan.md)과 [사용법](workflow-state-usage.md)을 따른다. 문서나 모델의 ‘완료’는 외부 효과의 확정을 대신하지 않는다.

개선 작업은 선택적 고정 계약과 baseline/verified/applied/evaluated 단계를 사용한다. 실제 수정과 측정은 에이전트, 순서·현재 근거·완료 gate는 API가 맡는다. [첫 완주 결과](self-improvement-result.md)에 구현 범위와 근거를 보존한다.

## 검증 수단과 재사용 경계

설계 인계는 검사별 기대값 출처·관측 방법과 기존 도구의 부족분을 포함한다. `taeguk_verification_builder`는 필요한 도구만 제작하고 정상·알려진 실패·경계 사례로 도구를 점검한다. 실행은 `taeguk_tool_verifier`, 증거 충분성 판단은 `taeguk_verifier`의 책임이다. 도구 자체 검증은 제품 검증을 대신하지 않는다.

`taeguk_structure_reviewer`는 새 요구 때문에 검증된 실행 영역까지 반복 수정하는 지점을 찾는다. 변하는 책임과 유지되는 계약에서 적합한 경계를 탐색한다. Policy·Workflow·Executor·Adapter는 예시이며 상태 전이, 입력 검증, 데이터 변환, 저장, 알고리즘, UI 등 다른 경계도 실제 변경 근거에 따라 검토한다. 특정 패턴 목록에 맞추거나 같은 모양의 기능을 무리하게 통합하지 않는다. 계약을 유지하는 영역의 재사용과 새 조합의 검증을 함께 계획하며, 영구 불변이나 자동 안전을 주장하지 않는다.

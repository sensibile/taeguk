---
name: taeguk-inspect-changeability
description: "Assess a concrete change burden and choose bounded tidying; use for repeated cochanges or blocked testable boundaries."
---

# taeguk-inspect-changeability

Start from an actual change need or repeated cost. Compare maintain, move responsibilities and narrow contract. Choose first, after, later or no tidying. Keep behavior changes separate from structural intent. State minimum scope, preservation evidence and stopping condition. Avoid numeric complexity gates and speculative future options.

Read [evidence packet](references/evidence-packet.md) when exchanging results or resuming work. Apply within the user's existing scope and authority; this skill does not authorize external effects or criteria changes.

## 검증된 영역을 재사용하기 위한 스멜

읽기 어려움보다 새 요구가 이미 검증된 영역까지 다시 수정하게 만드는 실제 변경 부담을 찾는다. 최근 diff·변경 이력 또는 현재의 구체적 요구에서 변하는 판단/정의와 유지할 실행 계약을 구분한다. LLM이 읽거나 수정하기 어렵다는 추정, 줄 수, 가상의 미래 요구만으로 개선을 제안하지 않는다.

| 후보 경계 | 관측할 시그널 | 주로 변할 부분 |
| --- | --- | --- |
| Policy Engine | 동일한 실행에서 허용 조건만 반복 변경되거나 판단이 중복됨 | 정책·결정 테이블 |
| Workflow | 같은 작업의 순서·분기·승인 조합 변경이 내부 실행 코드까지 전파됨 | 흐름 정의 |
| Executor | 여러 흐름에서 타임아웃·취소·재시도·실행 기록을 중복 구현함 | 작업과 실행 옵션 |
| Adapter | 외부 연동 변경이 업무 판단과 흐름까지 전파됨 | 연동 구현 |

이 표는 엔진 도입 명령이 아니다. 현상 유지, 함수/테이블 분리, 좁은 계약, 기존 도구 재사용을 비교하고 현재 문제를 해결하는 최소 범위를 선택한다. 증거가 없으면 관측 후보로만 남긴다.

검토 결과에는 변경 시그널과 근거, 변하는 부분, 보존할 영역과 계약 버전, 기존 검증 증거, 대안·비용, 필요한 계약/조합 검사, 중단 조건을 담는다. 새 조합은 부품 테스트만으로 통과시키지 않는다. 계약·구현·환경 변경 시 영향 검사를 다시 선택하고, 기존 증거 재사용의 근거와 한계를 기록한다. 검증된 영역도 영구 불변이 아니며 결함 수정이나 계약 변화가 있으면 수정할 수 있다.

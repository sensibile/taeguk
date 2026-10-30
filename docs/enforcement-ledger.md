# 실수 유형 원장

에이전트가 이 저장소에서 반복한 실수 유형과, 그 실수를 지금 무엇이 막고 있는지를 짝지어 둔다. 갱신 절차는 [`taeguk-encode-mistake-class`](../codex/skills/taeguk-encode-mistake-class/SKILL.md) 스킬, 원장에 올릴 후보 수집은 [`taeguk-harvest-lessons`](../codex/skills/taeguk-harvest-lessons/SKILL.md) 스킬이 맡는다. 이 원장의 운영 기준(두 번 발생, 강제 수단 순서, 반복 시 상향)은 [DEV-004](rules/development.md#dev-004) 1.0이다(2026-10-30 승인).

## 읽는 법

- **수준**: 실수를 막는 수단. 구조(잘못된 길이 없음) → 기계 검사(실패 메시지가 고칠 방법을 알려 줌) → 동작 테스트 → 문서(지키지 않아도 아무것도 실패하지 않음) 순으로 강하다.
- **연결**: 기계 검사나 테스트가 `pre-push`나 CI에서 실제로 도는지. 기존 테스트 경로에 넣은 것은 머지되면 연결된다. 그 밖에 있어 어떤 hook이나 CI job도 돌리지 않으면 미연결이며 강제된 것으로 보지 않는다.
- **상태**: 관찰 1회, 후보(두 번 이상), 강제됨, 닫힘(구조로 실수가 불가능해짐). 행은 지우지 않는다.
- 발생 근거는 커밋, PR, 리뷰 기록, 워커 보고, 세션 기록, BOSS 정정으로 확인한 것만 센다. 수집 보고의 행만 있는 것은 관찰 1회로 본다. 에이전트 메모리의 서술은 확인 전까지 주장으로 본다.

## 원장

| 유형 | 발생 근거 | 수준 | 강제 수단 | 연결 | 상태 |
|---|---|---|---|---|---|
| 영어로 쓴 스킬에 리뷰어가 "한국어로 작성" P1을 냄 | [sensibile/taeguk#21](https://github.com/sensibile/taeguk/pull/21) (2026-10-28), [sensibile/taeguk#30](https://github.com/sensibile/taeguk/pull/30) (2026-10-29) GitHub Codex 리뷰 | 문서 | AGENTS.md 3행 언어 규약(#30). GitHub Codex 리뷰어가 읽는 입력이 AGENTS.md라 저장소 쪽 검사로는 리뷰어의 출력을 막을 수 없다 | 해당 없음 | 후보. 2026-10-30 시범 처리: 규약 추가 뒤 #31~#41에서 같은 지적 0건을 확인하고 문서 수준을 유지. 다시 나오면 반복으로 보고 리뷰 프롬프트 맥락 등 다른 수단을 찾는다 |
| PR 제목·본문·댓글의 `@codex` 문자열이 리뷰 대신 Codex 클라우드 작업을 부름 | sensibile/taeguk#13~#16 (2026-10-25). #15를 닫을 때 남긴 댓글에도 응답함 | 기계 검사 | `scripts/check_pr_text.py`. PR 플로우 3단계가 올리기 전 실행을 요구한다. #13~#15 본문·댓글과 #16 첫 판에서 FAIL, 한글에 붙은 멘션도 FAIL, #17~#41 본문에서 오탐 0건 | 미연결. 실행은 PR을 쓰는 세션의 절차에 달려 있다(스크립트 자체의 테스트는 `tests/`에 있음) | 후보. 기계 검사는 작성했으나 미연결이라 강제됨이 아니다. 다음은 `gh pr create`·`gh pr comment` 전에 도는 에이전트 hook 연결이며 BOSS 확인 사항 |
| git을 부르는 테스트가 hook의 `GIT_*` 환경을 물려받아 실제 저장소를 바꿈 | [sensibile/taeguk#34](https://github.com/sensibile/taeguk/pull/34) 첫 push (2026-10-29), `core.bare=true` 기록 | 구조(부분) | `scripts/taeguk_install.py`의 `git_env()`, `tests/test_check_worker_scope.py`의 `ENV`. 두 곳이 따로 구현했고 새 테스트가 이를 쓰는지 검사하지 않음 | 해당 없음 | 관찰 1회 |

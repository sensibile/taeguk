# 리모트 저장소 운영

이 문서는 GitHub 저장소 `sensibile/taeguk`의 개발 플로우를 Claude Code에 위임한 범위와 PR 절차를 기록한다. 규칙 문서가 아니며 [현행 규칙](rules/index.md)의 ID·버전 체계에 등록하지 않는다. 문서 자체가 GitHub 권한을 부여하거나 제한하지 않는다. 실제 강제는 아래 GitHub 설정이 담당한다.

## 위임 근거

2026-10-23 BOSS가 Claude Code 세션에서 "개발 플로우 전체를 네가 컨트롤 하고 필요시 브랜치 설정 같은걸 변경하는 범위"를 요청했고, 이어서 "제안한 위임 범위로 확정해"라고 지시했다. AGENTS.md상 실행 권한 변경은 BOSS 확인 사항이며 이 지시가 그 확인이다. 위임 대상은 Claude Code이며 다른 에이전트로 확장한 것은 아니다.

사용 자격 증명은 BOSS 계정의 `gh` OAuth token이다. 관측 시점 scope는 `repo`, `workflow`, `read:org`, `gist`이고 저장소 권한은 ADMIN이다. `delete_repo`, `project`, `write:packages`, `admin:org`가 필요한 작업은 현재 token으로 수행할 수 없다.

## 위임 범위

| 구분 | 내용 |
|---|---|
| 자율 처리 | 작업 브랜치 생성, commit, push, PR 생성, CI 확인과 수정, 검증을 통과한 PR의 squash 머지, 머지된 브랜치 정리 |
| 자율 처리 후 보고 | 브랜치 보호와 ruleset, 머지 방식 설정, label과 milestone, workflow 수정 |
| BOSS 승인 필요 | `main` force push와 히스토리 재작성, 보호 규칙 완전 해제, 릴리스 태그와 GitHub Release, collaborator와 권한 변경, secret 변경, visibility 변경과 저장소 삭제, 외부 메시징 |

이 위임은 production 배포, database migration, 운영 장애 주입 권한을 포함하지 않는다. AGENTS.md의 local-only 개발 원칙과 규칙 변경 절차(GOV-002)는 그대로 적용한다.

## PR 플로우

1. 작업 worktree와 브랜치에서 변경하고 AGENTS.md의 검증 명령 중 해당하는 것을 실행한다.
2. push 전에 로컬에서 `codex review --base main`과 Claude Code `/security-review`를 실행하고 지적 사항을 반영하거나 반영하지 않은 이유를 남긴다.
3. PR을 만들면 Codex가 자동으로 리뷰한다. 자동 리뷰가 시작되지 않으면 `@codex review` 댓글로 요청한다.
4. `Codex review gate` workflow가 PR head 커밋에 대한 Codex 리뷰 결과를 commit status `codex-review`로 남긴다. 근거는 `chatgpt-codex-connector[bot]` 요약 댓글의 `codex-review-state` 상태다. head 커밋에 대한 리뷰가 `completed`이고 미해소 지적이 없으면 success, 미해소 지적이 있으면 failure, 그 밖에는 pending이다. 새 커밋을 push하면 pending으로 돌아가고, 이전 커밋을 리뷰한 상태이면 workflow가 head마다 한 번 `@codex review`를 요청한다. 요청 댓글에는 head SHA 표식을 남겨 중복 요청을 막는다. Codex 요약 댓글이 삭제되거나 PR base가 바뀌면 이전 리뷰를 무효로 표시하고 다시 요청한다. 무효 표시 이후 Codex가 요약을 갱신해야 다시 판정한다.
5. `codex-review`가 success가 되면 workflow가 squash auto-merge를 켠다. 머지는 리뷰한 head 커밋으로 고정(`--match-head-commit`)되고, 리뷰 스레드 해소와 나머지 필수 체크가 통과하면 GitHub가 머지한다. draft PR이나 `no-auto-merge` label이 붙은 PR은 auto-merge를 켜지 않고, 이미 켜져 있으면 해제한다. label은 GitHub의 머지 조건이 아니어서 이 처리는 workflow가 실행된 뒤에 적용된다. 그 사이 마지막 필수 체크가 끝나면 머지될 수 있다. 확실히 멈추려면 PR을 draft로 바꾸거나 auto-merge를 직접 해제한다. draft PR은 GitHub가 머지를 막는다.

요약 댓글의 상태 표식은 Codex의 공개 API가 아니다. 형식이 바뀌어 읽을 수 없으면 gate는 pending으로 남고 머지는 진행되지 않는다. 이 경우 Codex 결과를 직접 확인하고 수동으로 머지한다. gate workflow는 `pull_request_target`과 `issue_comment`로만 실행된다. 두 이벤트 모두 기본 브랜치의 workflow와 스크립트를 실행하므로 PR이 자기 판정 코드를 바꿀 수 없다. 다만 push 권한이 있는 사람은 다른 workflow로 `GITHUB_TOKEN` status를 게시할 수 있다. gate는 저장소 writer를 신뢰하는 전제의 자동화이며, writer에 대한 보안 경계는 아니다. fork PR과 기본 브랜치가 아닌 base로 향하는 PR은 gate가 처리하지 않는다. commit status는 PR이 아니라 커밋 SHA에 붙으므로, 같은 head를 공유하는 다른 base PR의 결과가 `main` PR 판정을 덮어쓰지 않게 하기 위해서다.

CodeQL은 Elixir를 지원하지 않는다. `apps/taeguk` 변경은 2단계의 로컬 보안 리뷰로 보완한다. 리뷰 도구의 결과는 검토 보조 증거이며 요구사항 충족 판정을 대신하지 않는다.

## GitHub 설정

2026-10-23에 적용하고 API 조회로 반영을 확인했다.

- ruleset `main protection`(id 25693048): 기본 브랜치에 적용한다. PR 필수, 필요 승인 0, 리뷰 스레드 해소 필수, squash 머지만 허용, `rules` 체크 통과 필수, CodeQL error 또는 high 이상 보안 경고 시 머지 차단, 브랜치 삭제와 force push 금지, bypass 대상 없음.
- 머지 방식: squash만 허용한다. 커밋 제목은 PR 제목, 본문은 PR 본문을 쓴다. 머지 후 브랜치를 자동 삭제한다. auto-merge를 허용한다.
- Codex 리뷰 gate: `.github/workflows/codex-review-gate.yml`과 `scripts/codex_review_gate.py`. ruleset 필수 체크에 commit status `codex-review`를 추가한다. workflow가 기본 브랜치에 들어간 뒤 지정한다. 기본 브랜치에 없는 workflow는 댓글 이벤트에 반응하지 않기 때문이다.
- 보안: Dependabot alerts와 security updates, CodeQL default setup(actions, python, rust, 주간 스캔과 PR 분석), secret scanning과 push protection.

필요 승인을 0으로 둔 이유는 현재 작업자와 PR 작성자가 같은 계정이고 GitHub가 자기 PR 승인을 허용하지 않기 때문이다. 필수 체크 이름 `rules`는 `.github/workflows/rules.yml`의 job 이름에, `codex-review`는 gate 스크립트의 status context에 의존한다. 이름을 바꾸면 ruleset도 함께 갱신해야 한다. auto-merge는 workflow의 `GITHUB_TOKEN`으로 켜므로, 그 머지로 생긴 `main` push는 `on: push` workflow를 다시 실행하지 않는다. PR 단계의 검사가 같은 내용을 이미 검증한다.

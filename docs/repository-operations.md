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

2026-10-28 BOSS가 "github origin 쪽에 codex 리뷰를 통한 게이트는 이제 제거해줘. 검증이 완료되면 자동머지 하는 형태로 그걸 위해 리뷰를 아래로 내렸어"라고 지시했다. 그래서 GitHub의 Codex 리뷰 gate(`codex-review` status)를 없앴다. 리뷰는 push 전 로컬 단계에서 하고, 머지 조건은 CI 검증이다. 이전 gate의 설계와 관측 근거는 Git 이력의 이 문서와 PR #6~#20에 남아 있다.

1. 작업 worktree와 브랜치에서 변경하고 AGENTS.md의 검증 명령 중 해당하는 것을 실행한다.
2. push 전에 PR 누적 범위(`origin/main`과의 merge-base부터 head까지)를 로컬에서 리뷰한다. 코드나 실행 동작에 영향을 주는 변경은 `taeguk_codex_cross_reviewer`와 `taeguk_codex_security_reviewer`를 `taeguk-codex-local-review` 절차(read-only sandbox, `gpt-6-astra`, `medium` effort)로 실행한다. 문서만 바뀐 변경은 보안 리뷰를 NOT_APPLICABLE로 남긴다. 지적은 반영하거나 반영하지 않은 이유를 남긴다. CodeQL이 지원하지 않는 Elixir(`apps/taeguk`) 변경은 이 보안 리뷰로 보완한다.
3. PR 본문에 검증 결과와 로컬 리뷰 결과(대상 SHA, 모델과 effort, 지적과 처리)를 적는다. 제목, 본문, 댓글에는 `@codex` 문자열을 쓰지 않는다. 코드 표기 안의 것도 Codex 클라우드 작업을 부른다(PR #13~#16).
4. PR을 만든 세션이 바로 `gh pr merge <번호> --squash --auto --match-head-commit <리뷰한 head SHA>`로 auto-merge를 켠다. 필수 체크가 통과하고 리뷰 thread가 모두 해소되고 PR이 최신 `main`을 포함하면 GitHub가 머지한다. draft PR이나 `no-auto-merge` label이 붙은 PR에는 켜지 않는다. 이 opt-out은 세션이 지키는 운영 규칙이며 GitHub 머지 조건이 아니다.
5. 새 커밋을 올릴 때는 먼저 `gh pr merge <번호> --disable-auto`로 auto-merge를 끈다. GitHub는 쓰기 권한자가 push해도 auto-merge를 유지하므로, 끄지 않으면 리뷰하지 않은 head가 머지될 수 있다. push한 뒤 2단계의 로컬 리뷰를 새 범위로 다시 하고 4단계로 다시 켠다.
6. `main`이 앞서 나가 PR이 뒤처지면 세션은 먼저 `gh pr merge <번호> --disable-auto`로 auto-merge를 끄고, `gh pr view <번호> --json autoMergeRequest`가 `null`인지 확인한 뒤 `gh pr update-branch`로 갱신한다. `--match-head-commit`은 켤 때의 head만 확인할 뿐 이후 head 변경을 막지 않기 때문이다. 갱신 뒤에는 PR 누적 diff가 같아도 2단계의 로컬 리뷰를 새 head로 다시 한 뒤 4단계로 켠다. 새 `main`이 PR 코드가 쓰는 함수나 권한 검사를 바꿨다면, diff가 같아도 합친 결과의 동작은 다를 수 있기 때문이다.

GitHub의 Codex 자동 리뷰는 저장소가 아니라 Codex 설정에서 켜져 있어 계속 댓글을 달 수 있다. 머지 조건은 아니지만 ruleset이 리뷰 thread 해소를 요구하므로, 세션은 thread를 판정해 반영하거나 이유를 답한 뒤 해소한다. 리뷰 도구의 결과는 검토 보조 증거이며 요구사항 충족 판정을 대신하지 않는다. 로컬 리뷰 실행 여부는 GitHub가 강제하지 않는다. 세션이 지키는 절차다.

## GitHub 설정

2026-10-23에 적용하고 API 조회로 반영을 확인했다.

- ruleset `main protection`(id 25693048): 기본 브랜치에 적용한다. PR 필수, 필요 승인 0, 리뷰 스레드 해소 필수, squash 머지만 허용, `rules`·`rust`·`elixir` 체크 통과 필수, 머지 전 최신 `main` 포함 필수(`strict_required_status_checks_policy`), CodeQL error 또는 high 이상 보안 경고 시 머지 차단, 브랜치 삭제와 force push 금지, bypass 대상 없음. 2026-10-28 `codex-review` 필수 체크를 빼고 `rust`, `elixir`를 추가했다.
- 머지 방식: squash만 허용한다. 커밋 제목은 PR 제목, 본문은 PR 본문을 쓴다. 머지 후 브랜치를 자동 삭제한다. auto-merge를 허용한다.
- 보안: Dependabot alerts와 security updates, CodeQL default setup(actions, python, rust, 주간 스캔과 PR 분석), secret scanning과 push protection.

필요 승인을 0으로 둔 이유는 현재 작업자와 PR 작성자가 같은 계정이고 GitHub가 자기 PR 승인을 허용하지 않기 때문이다. 필수 체크 이름 `rules`, `rust`, `elixir`는 `.github/workflows/rules.yml`의 job 이름에 의존한다. 이름을 바꾸면 ruleset도 함께 갱신해야 한다. 최신 base 요구 때문에 PR이 최신 `main`을 포함해야만 머지되므로, squash 결과는 PR 단계에서 검사한 merge ref와 내용이 같다.

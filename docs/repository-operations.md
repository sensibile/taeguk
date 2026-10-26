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
3. PR은 제목과 본문에 `@codex` 문자열 없이 연다. 그러면 Codex가 PR 생성 리뷰를 스스로 시작한다. 세션은 2분 동안 그 head에 대한 리뷰 요약 댓글(`codex-review-state`)이 붙는지 확인하고, 붙지 않을 때만 BOSS 계정으로 `@codex review` 댓글을 남긴다(2026-10-25 BOSS 결정). 근거: 본문에 `@codex`가 없던 PR #9~#12는 생성 후 10~20초 안에 리뷰 요약이 붙었다. 본문에 있던 PR #13~#16은 리뷰 요약 대신 멘션으로 호출된 Codex 클라우드 작업의 반응이 붙었다. Codex 클라우드 환경이 없던 PR #13, #14에는 "create an environment for this repo" 안내가, 환경을 만든 뒤인 PR #15에는 👀 반응만(10분 가까이 진행 없음), PR #16에는 AGENTS.md를 따르는 작업 응답이 붙었다. 어느 쪽도 gate가 판정할 요약과 결과 표식(`codex-review-receipt`)을 남기지 않았다. 리뷰(PR 생성, 새 커밋 push, `@codex review` 댓글)는 환경이 없던 PR #9~#12에서도 돌았으므로, Codex 리뷰에는 클라우드 환경이 필요하지 않다. 환경은 멘션으로 호출되는 작업에만 쓰인다. PR #15에서 BOSS 계정으로 요청하자 13초 뒤 시작해 약 45초 만에 요약과 결과 댓글을 남겼다. Codex는 코드 표기(백틱) 안의 `@codex`도 멘션으로 읽는다. PR #15를 닫으며 남긴 댓글에 그 문자열이 있자 Codex가 작업 응답을 달았다. "Codex review"나 `codex-review` 같은 문구는 트리거가 아니다(PR #9, #11). 그래서 PR 제목, 본문, 댓글에는 리뷰를 요청하는 댓글 말고는 `@codex` 문자열을 쓰지 않는다. 가리킬 때는 "Codex 리뷰 요청"처럼 풀어 쓴다.
4. `Codex review gate` workflow는 PR head 커밋에 대한 Codex 리뷰 결과를 commit status `codex-review`로 남기는 판정만 한다. 리뷰를 요청하거나 머지하지 않는다. 근거는 `chatgpt-codex-connector[bot]` 요약 댓글의 `codex-review-state` 상태다. head 커밋에 대한 리뷰가 `completed`이고 미해소 지적이 없으면(`has_unresolved_findings: false`) success, 미해소 지적이 있으면 failure, 그 밖에는 pending이다. Codex가 지적 없이 끝난 리뷰에 `has_unresolved_findings: null`을 남기는 경우가 있다(PR #3, `cf3a08f`). 2026-10-24 BOSS 승인에 따라, 이때는 두 조건을 모두 만족할 때만 success로 본다. 같은 head 커밋에 대한 Codex의 최신 결과 댓글이 "Didn't find any major issues"이고, Codex가 시작한 미해결 리뷰 thread가 0개여야 한다. 둘 중 하나라도 확인할 수 없으면 pending이다. 리뷰 thread를 해결하는 이벤트로는 gate가 다시 실행되지 않으므로, thread를 해결한 뒤에는 다음 이벤트가 있어야 다시 판정한다. 시간이 지나도 판정은 바뀌지 않는다. Codex가 그 head를 리뷰하지 않으면 status는 pending에 머물고 PR은 머지되지 않는다. 댓글이나 status 조회가 실패해 근거를 읽지 못하면 gate는 pending을 게시한 뒤 실패로 끝난다. 같은 head에 남아 있던 이전 success가 바뀐 리뷰 대신 머지 조건을 채우지 않게 하기 위해서다(acropolis PR #9 Codex 지적). pending 게시마저 실패하면 이전 status가 남으므로, gate job이 실패하면 세션이 status를 다시 확인한다. 새 커밋을 push하면 pending으로 돌아간다. Codex 요약 댓글이 삭제되거나 PR base가 바뀌면 gate가 이전 리뷰를 무효로 표시하는 댓글(head 표식 포함)을 남긴다. 이 댓글을 남기지 못하면 `record` job이 pending을 게시한 뒤 실패로 끝난다. `record`가 실패하면 `gate` job은 건너뛰므로, 그대로 두면 이전 success가 남기 때문이다(acropolis 세션의 Codex 리뷰 지적). 그래서 `record` job에도 `statuses: write` 권한이 있다. 이때 남긴 pending status("Codex review evidence was invalidated but could not be recorded")는 무효 표식 댓글을 대신한다. gate는 `github-actions[bot]`이 남긴 이 status가 Codex 요약의 마지막 갱신과 같은 초이거나 그보다 나중이면 pending으로 판정한다. 시각을 읽을 수 없는 경우도 마찬가지다. 댓글 표식도 같은 초면 무효로 본다. 무효는 요약이 확실히 나중에 갱신돼야 풀린다. 그래서 다음 gate 실행이나 동시에 돌던 gate가 바뀌지 않은 요약을 읽고 success로 되돌리지 못한다. gate는 success를 게시한 뒤 근거를 한 번 더 읽고, 그사이 생긴 무효가 있으면 pending으로 바꾼다. 마지막으로 읽은 뒤 무효가 생기는 아주 짧은 구간은 남는다(acropolis Codex 보안 리뷰 AGENT-002). 무효 표시 이후 Codex가 요약을 갱신해야 다시 판정한다. 2026-10-25 BOSS 결정으로 시간 기준 규칙(10분 간격 재요청, 3회 무응답 시 통과), gate의 `@codex review` 요청, 10분 주기 `schedule` 실행, gate의 auto-merge를 없앴다. Codex가 `github-actions[bot]`의 요청에 응답하지 않아(PR #3, archon PR #12) 시간 기준 규칙이 사실상 "판정이 멈추면 약 40분 뒤 리뷰 없이 통과"로 작동했기 때문이다.
5. 리뷰 요청은 PR을 맡은 Claude Code 세션이 BOSS 계정(세션의 `gh` 인증 계정)으로 한다(2026-10-25 BOSS 결정). Codex는 사람 계정 요청에는 20초에서 2분 안에 응답했다(PR #3, #6, #13, archon PR #12). 세션은 PR을 만들고 2분 안에 리뷰 요약이 붙지 않았을 때(3단계), 새 커밋을 push했을 때, gate가 무효 표시 댓글을 남겼을 때 그 head에 `@codex review`를 한 번 남긴다. 그 head에 대한 Codex 결과 댓글이 이미 있거나 리뷰가 `running`이면 남기지 않는다. 요청 뒤 10분 넘게 Codex가 시작하지 않거나 `running`에서 멈추면 한 번 더 요청하고, 그래도 응답이 없으면 BOSS에게 보고한다. 응답이 없다고 리뷰 없이 머지하지 않는다.
6. 머지도 PR을 맡은 세션이 한다. 세션은 PR head의 `codex-review`가 success이고 그 head에 미해결 리뷰 thread가 없을 때 `gh pr merge <번호> --squash --auto --match-head-commit <리뷰한 head SHA>`로 auto-merge를 켠다. 머지는 리뷰한 커밋으로 고정되고, 나머지 필수 체크가 통과하면 GitHub가 머지한다. draft PR이나 `no-auto-merge` label이 붙은 PR에는 켜지 않는다. 이 opt-out은 세션이 지키는 운영 규칙이며 GitHub 머지 조건이 아니다. 이미 켠 뒤에 멈추려면 PR을 draft로 바꾸거나 auto-merge를 직접 해제한다. Auto-fix가 켜져 있으면 Codex 결과 댓글과 CI 결과가 세션에 전달되어 세션이 이어서 판단한다. 이 전달은 관측된 동작이며 보장된 기능이 아니다. 세션이 없는 PR은 `codex-review`가 success여도 사람이 머지할 때까지 대기한다.

요약 댓글의 상태 표식은 Codex의 공개 API가 아니다. 형식이 바뀌어 읽을 수 없으면 gate는 pending으로 남고 필수 체크가 막혀 머지되지 않는다. 이 경우 Codex 결과를 직접 확인하고 BOSS 확인을 받아 처리한다. gate workflow는 `pull_request_target`과 `issue_comment`로만 실행된다. 두 이벤트 모두 기본 브랜치의 workflow와 스크립트를 실행하므로 PR이 자기 판정 코드를 바꿀 수 없다. 다만 push 권한이 있는 사람은 다른 workflow로 `GITHUB_TOKEN` status를 게시할 수 있다. gate는 저장소 writer를 신뢰하는 전제의 자동화이며, writer에 대한 보안 경계는 아니다. fork PR과 기본 브랜치가 아닌 base로 향하는 PR은 gate가 처리하지 않는다. commit status는 PR이 아니라 커밋 SHA에 붙으므로, 같은 head를 공유하는 다른 base PR의 결과가 `main` PR 판정을 덮어쓰지 않게 하기 위해서다.

CodeQL은 Elixir를 지원하지 않는다. `apps/taeguk` 변경은 2단계의 로컬 보안 리뷰로 보완한다. 리뷰 도구의 결과는 검토 보조 증거이며 요구사항 충족 판정을 대신하지 않는다.

## GitHub 설정

2026-10-23에 적용하고 API 조회로 반영을 확인했다.

- ruleset `main protection`(id 25693048): 기본 브랜치에 적용한다. PR 필수, 필요 승인 0, 리뷰 스레드 해소 필수, squash 머지만 허용, `rules` 체크 통과 필수, CodeQL error 또는 high 이상 보안 경고 시 머지 차단, 브랜치 삭제와 force push 금지, bypass 대상 없음.
- 머지 방식: squash만 허용한다. 커밋 제목은 PR 제목, 본문은 PR 본문을 쓴다. 머지 후 브랜치를 자동 삭제한다. auto-merge를 허용한다.
- Codex 리뷰 gate: `.github/workflows/codex-review-gate.yml`과 `scripts/codex_review_gate.py`. ruleset 필수 체크에 commit status `codex-review`를 추가하고, 머지 전에 PR 브랜치가 최신 `main`을 포함하도록 요구한다(`strict_required_status_checks_policy`). 두 설정 모두 workflow가 기본 브랜치에 들어간 뒤 지정한다. 기본 브랜치에 없는 workflow는 댓글 이벤트에 반응하지 않기 때문이다.
- 보안: Dependabot alerts와 security updates, CodeQL default setup(actions, python, rust, 주간 스캔과 PR 분석), secret scanning과 push protection.

필요 승인을 0으로 둔 이유는 현재 작업자와 PR 작성자가 같은 계정이고 GitHub가 자기 PR 승인을 허용하지 않기 때문이다. 필수 체크 이름 `rules`는 `.github/workflows/rules.yml`의 job 이름에, `codex-review`는 gate 스크립트의 status context에 의존한다. 이름을 바꾸면 ruleset도 함께 갱신해야 한다. 최신 base 요구는 gate가 auto-merge를 켜던 때(workflow `GITHUB_TOKEN`의 머지는 `on: push` workflow를 다시 실행하지 않음) 도입했고, 세션이 auto-merge를 켜는 지금도 유지한다. PR이 최신 `main`을 포함해야만 머지되므로, squash 결과는 PR 단계에서 검사한 merge ref와 내용이 같다. `main`이 앞서 나가 PR이 뒤처지면 auto-merge는 대기한다. 이때 세션이 `gh pr update-branch`로 사용자 token을 써서 갱신한다. 갱신하면 head가 바뀌므로 5단계대로 Codex 리뷰를 다시 요청하고, 새 head에 대해 6단계부터 다시 진행한다. workflow는 job 두 개로 나뉜다. `record` job은 직렬화하지 않으며, 리뷰를 무효로 만드는 이벤트(Codex 요약 삭제, base 변경)를 PR 댓글 표식으로 남기는 일만 한다. `gate` job은 PR별로 직렬화되어 판정을 차례로 게시하고, 이벤트 내용이 아니라 PR의 현재 상태(댓글, head, base)로만 판정한다. GitHub concurrency는 대기 중인 job을 나중 job으로 교체하지만, 교체한 job은 더 늦게 대기열에 들어왔으므로 같거나 더 새로운 상태를 읽는다. 그래서 이벤트를 잃지 않는다. 판정은 success를 게시하기 직전에 한 번 더 확인한다.

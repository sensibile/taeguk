#!/usr/bin/env python3
"""Publish the Codex review result for a PR head as the `codex-review` commit status."""
import argparse
import json
import re
import subprocess
import sys

CONTEXT = 'codex-review'
CODEX_LOGIN = 'chatgpt-codex-connector[bot]'
CODEX_APP = 'chatgpt-codex-connector'
ACTIONS_LOGIN = 'github-actions[bot]'
NO_AUTO_MERGE_LABEL = 'no-auto-merge'
# Undocumented marker inside Codex's summary comment; a format change must
# leave the gate pending, never successful.
SUMMARY_MARKERS = ('codex-pull-request-review-summary', 'codex-review-state')
STATE = re.compile(r'<!--\s*codex-review-state:v1\s+(\{.*?\})\s*-->', re.DOTALL)


def gh(*args):
    result = subprocess.run(['gh', *args], capture_output=True, text=True, check=True)
    return result.stdout


def is_codex(comment):
    user = comment.get('user') or {}
    app = comment.get('performed_via_github_app') or {}
    return user.get('login') == CODEX_LOGIN and user.get('type') == 'Bot' and app.get('slug') == CODEX_APP


def codex_summary(comments):
    summaries = [comment for comment in comments
                 if is_codex(comment) and any(marker in (comment.get('body') or '') for marker in SUMMARY_MARKERS)]
    # Never fall back to an older summary: the newest one is the current verdict.
    return max(summaries, key=lambda comment: comment.get('updated_at') or '') if summaries else None


def codex_state(comments):
    """Return the state from the newest Codex summary comment, or {} if it is unreadable."""
    newest = codex_summary(comments)
    if newest is None:
        return None
    match = STATE.search(newest.get('body') or '')
    try:
        state = json.loads(match.group(1)) if match else None
    except json.JSONDecodeError:
        state = None
    return state if isinstance(state, dict) else {}


def evaluate(state, head_sha):
    """Map Codex state to a commit status; anything unproven stays pending."""
    if state is None:
        return 'pending', 'Codex review has not reported yet'
    review = state.get('code_review')
    if not isinstance(review, dict):
        return 'pending', 'Codex review state is unreadable'
    if review.get('commit_sha') != head_sha:
        return 'pending', f'Codex review has not covered head {head_sha[:7]}'
    if review.get('status') != 'completed':
        return 'pending', f'Codex review is {review.get("status") or "not started"}'
    unresolved = review.get('has_unresolved_findings')
    if unresolved is False:
        return 'success', 'Codex review completed with no unresolved findings'
    if unresolved is True:
        return 'failure', 'Codex review has unresolved findings'
    return 'pending', 'Codex review finding state is unknown'


def request_marker(head_sha):
    return f'<!-- codex-review-gate-request:{head_sha} -->'


def invalidation_marker(head_sha):
    return f'<!-- codex-review-gate-invalidated:{head_sha} -->'


def invalidated(comments, head_sha):
    """True when the gate voided the review evidence for this head after Codex last updated it."""
    summary = codex_summary(comments)
    reviewed_at = (summary or {}).get('updated_at') or ''
    marker = invalidation_marker(head_sha)
    return any((comment.get('user') or {}).get('login') == ACTIONS_LOGIN and marker in (comment.get('body') or '')
               and (comment.get('created_at') or '') > reviewed_at for comment in comments)


def opted_out(pr):
    return bool(pr.get('draft')) or NO_AUTO_MERGE_LABEL in {label.get('name') for label in pr.get('labels') or []}


def needs_review_request(state, comments, head_sha):
    """Ask again only after Codex reviewed an older head; it reviews new PRs by itself."""
    review = (state or {}).get('code_review')
    if not isinstance(review, dict) or not review.get('commit_sha') or review.get('commit_sha') == head_sha:
        return False
    marker = request_marker(head_sha)
    return not any((comment.get('user') or {}).get('login') == ACTIONS_LOGIN and marker in (comment.get('body') or '')
                   for comment in comments)


def run(repo, number, request_review=False, auto_merge=False, invalidate=False, call=gh):
    pr = json.loads(call('api', f'repos/{repo}/pulls/{number}'))
    head_repo = (pr.get('head') or {}).get('repo') or {}
    base = pr.get('base') or {}
    if pr.get('state') != 'open' or head_repo.get('full_name') != repo:
        return {'pr': number, 'skipped': 'closed or cross-repository pull request'}
    # A commit status belongs to the SHA, not the PR. GitHub allows one open PR
    # per head and base, so gating only default-branch PRs keeps verdicts apart.
    if base.get('ref') != (base.get('repo') or {}).get('default_branch'):
        return {'pr': number, 'skipped': 'pull request does not target the default branch'}
    head = pr['head']['sha']
    actions = []
    if invalidate:
        # A deleted summary or a retargeted base voids the evidence; the marker
        # keeps later runs pending until Codex reviews again.
        call('api', '-X', 'POST', f'repos/{repo}/issues/{number}/comments', '-f', f'body=@codex review\n\n{invalidation_marker(head)}')
        actions.append('invalidated_codex_review')
    pages = json.loads(call('api', '--paginate', '--slurp', f'repos/{repo}/issues/{number}/comments'))
    comments = [comment for page in pages for comment in page]
    state = codex_state(comments)
    if invalidate or invalidated(comments, head):
        status, description = 'pending', 'Codex review evidence was invalidated; waiting for a new review'
    else:
        status, description = evaluate(state, head)
    if auto_merge and opted_out(pr) and pr.get('auto_merge'):
        # Before the status: a success could otherwise merge the PR first. A
        # failed disable raises here, so no status is published.
        call('pr', 'merge', str(number), '--repo', repo, '--disable-auto')
        actions.append('disabled_auto_merge')
    call('api', '-X', 'POST', f'repos/{repo}/statuses/{head}', '-f', f'state={status}',
         '-f', f'context={CONTEXT}', '-f', f'description={description}', '-f', f'target_url={pr["html_url"]}')
    if request_review and not invalidate and status == 'pending' and needs_review_request(state, comments, head):
        call('api', '-X', 'POST', f'repos/{repo}/issues/{number}/comments', '-f', f'body=@codex review\n\n{request_marker(head)}')
        actions.append('requested_codex_review')
    if auto_merge and status == 'success' and not opted_out(pr) and not pr.get('auto_merge'):
        # Re-read just before enabling to narrow the window for a new opt-out or
        # push; draft is the only opt-out GitHub itself enforces at merge time.
        latest = json.loads(call('api', f'repos/{repo}/pulls/{number}'))
        if not opted_out(latest) and latest['head']['sha'] == head and not latest.get('auto_merge'):
            # Bind the merge to the reviewed commit so a later push cannot ride along.
            call('pr', 'merge', str(number), '--repo', repo, '--auto', '--squash', '--match-head-commit', head)
            actions.append('enabled_auto_merge')
    return {'pr': number, 'head': head, 'status': status, 'description': description, 'actions': actions}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', required=True)
    parser.add_argument('--pr', required=True, type=int)
    parser.add_argument('--request-review', action='store_true', help='comment @codex review once per head after Codex reviewed an older head')
    parser.add_argument('--auto-merge', action='store_true', help='enable squash auto-merge once the gate succeeds')
    parser.add_argument('--invalidate', action='store_true', help='void the current review evidence and request a new review')
    args = parser.parse_args()
    try:
        result = run(args.repo, args.pr, args.request_review, args.auto_merge, args.invalidate)
    except subprocess.CalledProcessError as error:
        print(error.stderr, file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False))
    return 0


if __name__ == '__main__':
    sys.exit(main())

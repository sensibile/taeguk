---
name: taeguk-plan-cross-verification
description: "Select checks and cross-observations by change attributes; use when preparing or reviewing a verification plan."
---

# taeguk-plan-cross-verification

Identify behavior, state, concurrency, contract, resource, security and delivery attributes actually changed. Select a primary check and a different observation where needed. Explain independence and exclusions. Contract plus stored effect, model plus real parallel execution, or load plus resource evidence are examples. Do not require every tool for every task.

When planning a software change, read the original request and design before implementation. Give the plan an exact version and connect each check to its requirement, approved oracle, expected result source, method (tool, LLM or mixed), owner, artifact/environment, scope, evidence location, gaps and status. Mixed claims that arise from inference need a separate observation or reproduction. Keep checks NOT_RUN until executed. The implementer and reviewers use the same plan version; after a repair, name the affected checks for re-verification. Do not infer acceptance criteria from implementation output or create new global thresholds.

Read [evidence packet](references/evidence-packet.md) when exchanging results or resuming work. Apply within the user's existing scope and authority; this skill does not authorize external effects or criteria changes.

For repairs or shared-resource execution, use [re-verification and handoff](references/reverification.md). Keep fixed criteria while selecting affected checks, documenting evidence reuse and returning resources promptly.

Use [check selection](references/check-selection.md) to choose applicable observation pairs and return an executable plan.

분석·제품 과업에서는 원래 질문에 대한 답, 처리·계산 정확성, 전달과 실제 사용 효과 중 어떤 부분을 각 검사가 확인하는지 나눈다. 목적에 대한 완료 검토는 $taeguk-work-contract의 참고를 사용한다. 같은 원본의 합계 일치로 해석의 타당성이나 사용자 부담 감소까지 확인했다고 보고하지 않는다.

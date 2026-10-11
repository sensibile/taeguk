defmodule Taeguk.Improvement do
  alias Taeguk.{Repo, StateInput}

  def validate!(contract, work) do
    StateInput.fields!(
      contract,
      ~w(source_ref problem_ref baseline_ref rollback_ref verification_checks effect_check)
    )

    Enum.each(
      ~w(source_ref problem_ref baseline_ref rollback_ref effect_check),
      &StateInput.text!(contract[&1])
    )

    ids = Enum.map(work["checks"], & &1["id"])

    StateInput.ensure!(
      Enum.find(work["checks"], &(&1["id"] == contract["effect_check"]))["accepted_statuses"] == [
        "PASS"
      ]
    )

    checks = contract["verification_checks"]
    StateInput.ensure!(is_list(checks) and checks != [] and Enum.uniq(checks) == checks)
    StateInput.ensure!(Enum.all?(checks, &(&1 in ids)) and contract["effect_check"] in ids)

    StateInput.ensure!(
      contract["effect_check"] not in checks and contract["baseline_ref"] == work["artifact_ref"]
    )

    StateInput.ensure!(
      Enum.all?(checks ++ [contract["effect_check"]], fn id ->
        Enum.find(work["checks"], &(&1["id"] == id))["required"]
      end)
    )
  end

  def transition!(w, p, revision, observations) do
    d = w["document"]
    contract = d["improvement"] || Repo.rollback({:invalid, "not_improvement_work"})
    context = Map.take(d, ~w(artifact_ref environment_ref plan_ref))

    unless Enum.all?(context, fn {key, value} -> p[key] == value end),
      do: Repo.rollback({:conflict, "stale_improvement_context"})

    old = d["improvement_progress"] || %{}
    phase = p["phase"]

    permitted =
      case phase do
        "baseline" ->
          old["phase"] in [nil, "baseline"] and d["artifact_ref"] == contract["baseline_ref"]

        "verified" ->
          old["phase"] == "baseline" and old["baseline"]["status"] == "PASS"

        "applied" ->
          old["phase"] == "verified"

        "evaluated" ->
          old["phase"] in ["applied", "evaluated"]
      end

    unless permitted, do: Repo.rollback({:conflict, "improvement_phase_out_of_order"})

    if phase in ~w(verified applied) and
         (p["status"] != "PASS" or not ready?(d, contract["verification_checks"], observations)),
       do: Repo.rollback({:conflict, "improvement_verification_missing"})

    if phase == "evaluated" and
         p["status"] != effect_status(d, observations, old["applied_revision"]),
       do: Repo.rollback({:conflict, "effect_status_mismatch"})

    entry =
      Map.take(p, ~w(status evidence reason artifact_ref environment_ref plan_ref actor))
      |> Map.put("revision", revision)

    progress = old |> Map.put("phase", phase) |> Map.put(phase, entry)

    progress =
      if phase == "applied", do: Map.put(progress, "applied_revision", revision), else: progress

    {Map.put(d, "improvement_progress", progress),
     %{operation: "improvement", phase: phase, status: p["status"]}}
  end

  def reset(document) do
    case document["improvement_progress"] do
      %{"baseline" => baseline} ->
        Map.put(document, "improvement_progress", %{"phase" => "baseline", "baseline" => baseline})

      _ ->
        Map.delete(document, "improvement_progress")
    end
  end

  def blockers(document, observations) do
    if contract = document["improvement"] do
      progress = document["improvement_progress"] || %{}

      if progress["phase"] == "evaluated" and progress["evaluated"]["status"] == "PASS" and
           ready?(document, contract["verification_checks"], observations) and
           effect_status(document, observations, progress["applied_revision"]) == "PASS",
         do: [],
         else: [%{reason: "improvement_effect_not_confirmed"}]
    else
      []
    end
  end

  defp ready?(d, ids, observations) do
    Enum.all?(ids, fn id ->
      c = Enum.find(d["checks"], &(&1["id"] == id))

      Enum.all?(kinds(c), fn kind ->
        o = find(observations, id, kind)
        o && o["document"]["status"] in c["accepted_statuses"]
      end)
    end)
  end

  defp effect_status(d, observations, applied_revision) do
    c = Enum.find(d["checks"], &(&1["id"] == d["improvement"]["effect_check"]))

    statuses =
      Enum.map(kinds(c), fn kind ->
        o = find(observations, c["id"], kind)

        cond do
          is_nil(o) or not is_integer(applied_revision) -> "NOT_RUN"
          o["revision"] <= applied_revision or o["document"]["reused_from"] -> "NOT_RUN"
          o["document"]["status"] in c["accepted_statuses"] -> "PASS"
          true -> o["document"]["status"]
        end
      end)

    cond do
      Enum.all?(statuses, &(&1 == "PASS")) -> "PASS"
      "FAIL" in statuses -> "FAIL"
      "UNKNOWN" in statuses -> "UNKNOWN"
      "NOT_RUN" in statuses -> "NOT_RUN"
      true -> "NOT_APPLICABLE"
    end
  end

  defp kinds(c), do: if(c["method"] == "mixed", do: ~w(tool llm), else: [c["method"]])

  defp find(observations, id, kind),
    do:
      Enum.find(
        observations,
        &(&1["document"]["check_id"] == id and &1["document"]["kind"] == kind)
      )
end

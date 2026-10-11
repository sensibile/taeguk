defmodule Taeguk.StateInput do
  @statuses ~w(PASS FAIL UNKNOWN NOT_RUN NOT_APPLICABLE)
  def create!(p) do
    fields!(
      p,
      ~w(id command_id actor purpose requirement_ref rule_ref design_ref plan_ref artifact_ref environment_ref checks)
    )

    uuid!(p["id"])
    uuid!(p["command_id"])

    Enum.each(
      ~w(actor purpose requirement_ref rule_ref design_ref plan_ref artifact_ref environment_ref),
      &text!(p[&1])
    )

    checks = p["checks"]
    ensure!(is_list(checks) and length(checks) in 1..64)

    Enum.each(checks, fn c ->
      fields!(c, ~w(id method required oracle_ref accepted_statuses))
      text!(c["id"])
      text!(c["oracle_ref"])
      ensure!(c["method"] in ~w(tool llm mixed) and is_boolean(c["required"]))
      accepted = c["accepted_statuses"]

      ensure!(
        is_list(accepted) and accepted != [] and
          Enum.all?(accepted, &(&1 in ~w(PASS NOT_APPLICABLE)))
      )
    end)

    ensure!(length(Enum.uniq_by(checks, & &1["id"])) == length(checks))
    p
  end

  def command!(p) when is_map(p) do
    fields =
      case p["operation"] do
        "observe" ->
          ~w(check_id kind status artifact_ref environment_ref plan_ref evidence reason)

        "reuse" ->
          ~w(observation_id reason)

        "retarget" ->
          ~w(artifact_ref environment_ref reason)

        "start_run" ->
          ~w(label resources)

        "update_run" ->
          ~w(run_id handle)

        "end_run" ->
          ~w(run_id status cleanup_ref result_ref)

        "complete" ->
          []

        _ ->
          invalid!()
      end

    fields!(p, ~w(command_id expected_revision actor operation) ++ fields)
    uuid!(p["command_id"])
    ensure!(is_integer(p["expected_revision"]) and p["expected_revision"] > 0)
    text!(p["actor"])

    case p["operation"] do
      "observe" ->
        Enum.each(~w(check_id artifact_ref environment_ref plan_ref reason), &text!(p[&1]))
        ensure!(p["kind"] in ~w(tool llm) and p["status"] in @statuses)
        evidence!(p["evidence"])

      "reuse" ->
        uuid!(p["observation_id"])
        text!(p["reason"])

      "retarget" ->
        Enum.each(~w(artifact_ref environment_ref reason), &text!(p[&1]))

      "start_run" ->
        text!(p["label"])
        rs = p["resources"]
        ensure!(is_list(rs) and length(rs) in 1..8 and length(Enum.uniq(rs)) == length(rs))
        Enum.each(rs, &text!/1)

      "update_run" ->
        uuid!(p["run_id"])
        text!(p["handle"])

      "end_run" ->
        uuid!(p["run_id"])
        ensure!(p["status"] in ~w(finished interrupted))
        text!(p["cleanup_ref"])
        text!(p["result_ref"])

      "complete" ->
        :ok
    end

    p
  end

  def command!(_), do: invalid!()

  def uuid!(s) do
    case Ecto.UUID.cast(s) do
      {:ok, ^s} -> s
      _ -> invalid!()
    end
  end

  defp evidence!(items) do
    ensure!(is_list(items) and length(items) in 1..16)

    Enum.each(items, fn e ->
      fields!(e, ~w(location sha256))
      text!(e["location"])
      ensure!(is_binary(e["sha256"]) and Regex.match?(~r/\A[0-9a-f]{64}\z/, e["sha256"]))
    end)
  end

  defp fields!(p, fields) do
    ensure!(is_map(p) and Enum.sort(Map.keys(p)) == Enum.sort(fields))
    ensure!(byte_size(Jason.encode!(p)) <= 65_536)
  end

  defp text!(s), do: ensure!(is_binary(s) and byte_size(s) in 1..8192 and String.trim(s) != "")
  defp ensure!(true), do: :ok
  defp ensure!(_), do: invalid!()
  defp invalid!, do: raise(ArgumentError, "invalid_state_input")
end

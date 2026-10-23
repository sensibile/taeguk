defmodule Taeguk.State do
  alias Taeguk.{Repo, StateInput}

  def create(input) do
    safe(fn ->
      p = StateInput.create!(input)

      Repo.transaction(fn ->
        rows =
          q(
            "INSERT INTO work_items(id,revision,document) VALUES(?1,1,?2) ON CONFLICT DO NOTHING RETURNING id",
            [uuid(p["id"]), Map.drop(p, ~w(id actor command_id))]
          )

        cond do
          rows != [] -> event!(p["id"], 1, p, %{operation: "create"})
          prior(p["id"], p["command_id"]) -> replay!(p["id"], p)
          true -> Repo.rollback({:conflict, "work_exists"})
        end
      end)
    end)
  end

  def command(id, input) do
    safe(fn ->
      StateInput.uuid!(id)
      p = StateInput.command!(input)

      Repo.transaction(fn ->
        w = work!(id)

        case prior(id, p["command_id"]) do
          nil ->
            if w["revision"] != p["expected_revision"],
              do: Repo.rollback({:conflict, "stale_revision"})

            if w["completed"] and p["operation"] != "retarget",
              do: Repo.rollback({:conflict, "work_completed"})

            revision = w["revision"] + 1
            detail = apply!(w, p, revision)

            q(
              "UPDATE work_items SET revision=?2, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",
              [
                uuid(id),
                revision
              ]
            )

            event!(id, revision, p, detail)

          _ ->
            replay!(id, p)
        end
      end)
    end)
  end

  def get(id) do
    safe(fn ->
      StateInput.uuid!(id)

      Repo.transaction(
        fn ->
          w = work!(id)
          observations = current_observations(w)

          runs =
            maps(
              "SELECT id,owner,label,handle,status,cleanup_ref,result_ref,context,started_at,ended_at FROM state_runs WHERE work_id=?1 ORDER BY started_at DESC,id LIMIT 100",
              [uuid(id)]
            )

          %{
            work: w,
            observations: observations,
            runs: runs,
            blockers: blockers(w, observations),
            events:
              maps(
                "SELECT command_id,revision,ack,created_at FROM state_events WHERE work_id=?1 ORDER BY revision DESC LIMIT 100",
                [uuid(id)]
              ),
            limits: [
              "registered metadata, not evidence authenticity or human approval",
              "latest current observations and last 100 runs/events; use history for requests"
            ]
          }
        end,
        mode: :deferred
      )
    end)
  end

  def list do
    safe(fn ->
      {:ok,
       maps(
         "SELECT id,revision,completed,updated_at,json_extract(document,'$.purpose') AS purpose FROM work_items ORDER BY updated_at DESC,id LIMIT 100"
       )}
    end)
  end

  def history(id, after_revision) do
    safe(fn ->
      StateInput.uuid!(id)
      unless is_integer(after_revision) and after_revision >= 0, do: raise(ArgumentError)

      Repo.transaction(
        fn ->
          work!(id)

          maps(
            "SELECT command_id,revision,request,ack,created_at FROM state_events WHERE work_id=?1 AND revision>?2 ORDER BY revision LIMIT 25",
            [uuid(id), after_revision]
          )
        end,
        mode: :deferred
      )
    end)
  end

  defp apply!(w, %{"operation" => "observe"} = p, revision) do
    d = w["document"]
    c = check!(w, p["check_id"])

    if c["method"] != "mixed" and c["method"] != p["kind"],
      do: Repo.rollback({:invalid, "wrong_observation_kind"})

    unless Enum.all?(~w(artifact_ref environment_ref plan_ref), &(p[&1] == d[&1])),
      do: Repo.rollback({:conflict, "stale_observation_context"})

    doc =
      Map.take(
        p,
        ~w(check_id kind status artifact_ref environment_ref plan_ref evidence reason actor)
      )

    put_observation(w["id"], revision, doc)
  end

  defp apply!(w, %{"operation" => "reuse"} = p, revision) do
    old =
      case q("SELECT document FROM observations WHERE id=?1 AND work_id=?2", [
             uuid(p["observation_id"]),
             uuid(w["id"])
           ]) do
        [[doc]] -> doc
        _ -> Repo.rollback({:not_found, "observation_not_found"})
      end

    check!(w, old["check_id"])
    doc = Map.merge(old, Map.take(w["document"], ~w(artifact_ref environment_ref plan_ref)))

    doc =
      Map.merge(doc, %{
        "reused_from" => p["observation_id"],
        "reuse_reason" => p["reason"],
        "source_actor" => old["actor"],
        "actor" => p["actor"]
      })

    put_observation(w["id"], revision, doc)
  end

  defp apply!(w, %{"operation" => "retarget"} = p, _) do
    no_running!(w["id"])

    doc =
      Map.merge(w["document"], Map.take(p, ~w(artifact_ref environment_ref)))
      |> Taeguk.Improvement.reset()

    q("UPDATE work_items SET document=?2,completed=0 WHERE id=?1", [uuid(w["id"]), doc])
    %{operation: "retarget"}
  end

  defp apply!(w, %{"operation" => "start_run"} = p, _) do
    id = Ecto.UUID.generate()
    context = Map.take(w["document"], ~w(artifact_ref environment_ref plan_ref))

    q(
      "INSERT INTO state_runs(id,work_id,owner,label,status,context) VALUES(?1,?2,?3,?4,'running',?5)",
      [uuid(id), uuid(w["id"]), p["actor"], p["label"], context]
    )

    Enum.each(Enum.sort(p["resources"]), fn resource ->
      if q(
           "INSERT INTO resource_claims(resource,run_id) VALUES(?1,?2) ON CONFLICT DO NOTHING RETURNING resource",
           [resource, uuid(id)]
         ) == [],
         do: Repo.rollback({:conflict, "resource_busy"})
    end)

    %{operation: "start_run", run_id: id}
  end

  defp apply!(w, %{"operation" => "update_run"} = p, _) do
    owned_run!(w["id"], p)
    q("UPDATE state_runs SET handle=?2 WHERE id=?1", [uuid(p["run_id"]), p["handle"]])
    %{operation: "update_run", run_id: p["run_id"]}
  end

  defp apply!(w, %{"operation" => "end_run"} = p, _) do
    owned_run!(w["id"], p)

    q(
      "UPDATE state_runs SET status=?2,cleanup_ref=?3,result_ref=?4,ended_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1",
      [uuid(p["run_id"]), p["status"], p["cleanup_ref"], p["result_ref"]]
    )

    q("DELETE FROM resource_claims WHERE run_id=?1", [uuid(p["run_id"])])
    %{operation: "end_run", run_id: p["run_id"]}
  end

  defp apply!(w, %{"operation" => "improvement"} = p, revision) do
    no_running!(w["id"])
    {document, detail} = Taeguk.Improvement.transition!(w, p, revision, current_observations(w))
    q("UPDATE work_items SET document=?2 WHERE id=?1", [w["id"], document])
    detail
  end

  defp apply!(w, %{"operation" => "complete"}, _) do
    if blockers(w, current_observations(w)) != [],
      do: Repo.rollback({:conflict, "completion_evidence_missing"})

    no_running!(w["id"])
    q("UPDATE work_items SET completed=1 WHERE id=?1", [uuid(w["id"])])
    %{operation: "complete", scope: "registered_required_observations_only"}
  end

  defp current_observations(w) do
    d = w["document"]

    maps(
      """
      SELECT id,revision,document FROM (
        SELECT id,revision,document,ROW_NUMBER() OVER (
          PARTITION BY json_extract(document,'$.check_id'),json_extract(document,'$.kind')
          ORDER BY revision DESC) AS position
        FROM observations WHERE work_id=?1 AND json_extract(document,'$.artifact_ref')=?2
          AND json_extract(document,'$.environment_ref')=?3 AND json_extract(document,'$.plan_ref')=?4
      ) WHERE position=1 ORDER BY json_extract(document,'$.check_id'),json_extract(document,'$.kind')
      """,
      [uuid(w["id"]), d["artifact_ref"], d["environment_ref"], d["plan_ref"]]
    )
  end

  defp blockers(w, observations) do
    check_blockers(w, observations) ++ Taeguk.Improvement.blockers(w["document"], observations)
  end

  defp check_blockers(w, observations) do
    required = Enum.filter(w["document"]["checks"], & &1["required"])

    if required == [] do
      [%{reason: "no_required_checks"}]
    else
      Enum.flat_map(required, fn c ->
        kinds = if c["method"] == "mixed", do: ~w(tool llm), else: [c["method"]]

        Enum.flat_map(kinds, fn kind ->
          o =
            Enum.find(observations, fn o ->
              o["document"]["check_id"] == c["id"] and o["document"]["kind"] == kind
            end)

          status = if o, do: o["document"]["status"], else: "NOT_RUN"

          if o && status in c["accepted_statuses"],
            do: [],
            else: [%{check_id: c["id"], kind: kind, status: status}]
        end)
      end)
    end
  end

  defp put_observation(work_id, revision, doc) do
    id = Ecto.UUID.generate()

    q("INSERT INTO observations(id,work_id,revision,document) VALUES(?1,?2,?3,?4)", [
      uuid(id),
      uuid(work_id),
      revision,
      doc
    ])

    %{operation: "observe", observation_id: id}
  end

  defp owned_run!(id, p) do
    actor = p["actor"]

    case q("SELECT owner,status FROM state_runs WHERE id=?1 AND work_id=?2", [
           uuid(p["run_id"]),
           uuid(id)
         ]) do
      [[owner, "running"]] when owner == actor -> :ok
      [] -> Repo.rollback({:not_found, "run_not_found"})
      _ -> Repo.rollback({:conflict, "run_not_owned_or_not_running"})
    end
  end

  defp no_running!(id) do
    if q("SELECT 1 FROM state_runs WHERE work_id=?1 AND status='running' LIMIT 1", [uuid(id)]) !=
         [],
       do: Repo.rollback({:conflict, "run_still_active"})
  end

  defp check!(w, id),
    do:
      Enum.find(w["document"]["checks"], &(&1["id"] == id)) ||
        Repo.rollback({:invalid, "check_not_in_plan"})

  defp work!(id) do
    case maps(
           "SELECT id,revision,document,completed,updated_at FROM work_items WHERE id=?1",
           [uuid(id)]
         ) do
      [w] -> w
      _ -> Repo.rollback({:not_found, "work_not_found"})
    end
  end

  defp prior(id, command_id) do
    case q("SELECT request,ack FROM state_events WHERE work_id=?1 AND command_id=?2", [
           uuid(id),
           uuid(command_id)
         ]) do
      [[request, ack]] -> {request, ack}
      _ -> nil
    end
  end

  defp replay!(id, p) do
    case prior(id, p["command_id"]) do
      {^p, ack} -> ack
      _ -> Repo.rollback({:conflict, "command_id_reused"})
    end
  end

  defp event!(id, revision, p, detail) do
    ack = Map.merge(%{work_id: id, revision: revision}, detail)

    q(
      "INSERT INTO state_events(work_id,command_id,revision,request,ack) VALUES(?1,?2,?3,?4,?5)",
      [uuid(id), uuid(p["command_id"]), revision, p, ack]
    )

    # ACK types must be identical on initial request and JSON-backed replay.
    ack |> Jason.encode!() |> Jason.decode!()
  end

  defp query(sql, params) do
    params =
      Enum.map(params, fn
        value when is_map(value) or is_list(value) -> Jason.encode!(value)
        value -> value
      end)

    result = Repo.query!(sql, params)

    rows =
      Enum.map(result.rows || [], fn row ->
        Enum.zip(result.columns, row)
        |> Enum.map(fn
          {column, value} when column in ["document", "request", "ack", "context"] ->
            Jason.decode!(value)

          {"completed", value} ->
            value == 1

          {_, value} ->
            value
        end)
      end)

    %{result | rows: rows}
  end

  defp maps(sql, params \\ []) do
    result = query(sql, params)
    Enum.map(result.rows, &(Enum.zip(result.columns, &1) |> Map.new()))
  end

  defp q(sql, params), do: query(sql, params).rows
  defp uuid(id), do: id

  defp safe(fun) do
    if Application.get_env(:taeguk, :state_enabled) do
      try do
        fun.()
      rescue
        ArgumentError ->
          {:error, {:invalid, "invalid_state_input"}}

        KeyError ->
          {:error, {:invalid, "invalid_state_input"}}

        DBConnection.ConnectionError ->
          {:error, {:unavailable, "database_unavailable"}}

        error in Exqlite.Error ->
          reason =
            if error.message in [
                 "Database is busy",
                 "database is locked",
                 "database table is locked"
               ],
               do: "database_busy",
               else: "database_operation_failed"

          {:error, {:unavailable, reason}}
      end
    else
      {:error, {:unavailable, "state_not_configured"}}
    end
  end
end

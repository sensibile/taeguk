defmodule Taeguk.ImprovementTest do
  use ExUnit.Case, async: false
  alias Taeguk.State
  @moduletag skip: !Application.compile_env(:taeguk, :state_enabled, false)
  setup_all do
    path = Application.fetch_env!(:taeguk, Taeguk.Repo)[:database]
    unless String.ends_with?(path, "-test.sqlite3"), do: raise("dedicated test database required")
    :ok
  end

  defp fixture do
    p = %{
      "id" => Ecto.UUID.generate(),
      "command_id" => Ecto.UUID.generate(),
      "actor" => "improver",
      "purpose" => "clear failure diagnostics",
      "requirement_ref" => "req1",
      "rule_ref" => "rules1",
      "design_ref" => "design1",
      "plan_ref" => "plan1",
      "artifact_ref" => "before",
      "environment_ref" => "fixture",
      "checks" =>
        Enum.map(~w(verify effect), fn id ->
          %{
            "id" => id,
            "method" => "tool",
            "required" => true,
            "oracle_ref" => "oracle:" <> id,
            "accepted_statuses" => ["PASS"]
          }
        end),
      "improvement" => %{
        "source_ref" => "retrospective1",
        "problem_ref" => "baseline.json",
        "baseline_ref" => "before",
        "rollback_ref" => "git:before",
        "verification_checks" => ["verify"],
        "effect_check" => "effect"
      }
    }

    assert {:ok, _} = State.create(p)
    p
  end

  defp send_command(p, operation, fields) do
    {:ok, %{work: w}} = State.get(p["id"])

    c =
      Map.merge(
        %{
          "command_id" => Ecto.UUID.generate(),
          "expected_revision" => w["revision"],
          "actor" => "improver",
          "operation" => operation
        },
        fields
      )

    {State.command(p["id"], c), c}
  end

  defp evidence, do: [%{"location" => "actual.log", "sha256" => String.duplicate("a", 64)}]

  defp context(p) do
    {:ok, %{work: w}} = State.get(p["id"])
    Map.take(w["document"], ~w(artifact_ref environment_ref plan_ref))
  end

  defp phase(p, phase, status \\ "PASS"),
    do:
      send_command(
        p,
        "improvement",
        Map.merge(context(p), %{
          "phase" => phase,
          "status" => status,
          "reason" => "actual observation",
          "evidence" => evidence()
        })
      )

  defp observe(p, check, status \\ "PASS"),
    do:
      send_command(
        p,
        "observe",
        Map.merge(context(p), %{
          "check_id" => check,
          "kind" => "tool",
          "status" => status,
          "reason" => "fixture observation",
          "evidence" => evidence()
        })
      )

  defp retarget(p, head),
    do:
      send_command(p, "retarget", %{
        "artifact_ref" => head,
        "environment_ref" => "fixture",
        "reason" => "install new version"
      })

  test "full loop needs post-application effect, records failures, and completes on actual PASS" do
    p = fixture()
    assert {{:error, {:conflict, "improvement_phase_out_of_order"}}, _} = phase(p, "verified")
    assert {{:ok, _}, _} = phase(p, "baseline")
    assert {{:ok, _}, _} = retarget(p, "after")
    assert {{:error, {:conflict, "improvement_verification_missing"}}, _} = phase(p, "verified")
    assert {{:ok, _}, _} = observe(p, "verify")
    assert {{:ok, _}, _} = observe(p, "effect")
    assert {{:ok, _}, _} = phase(p, "verified")
    assert {{:ok, _}, _} = phase(p, "applied")
    assert {{:error, {:conflict, "effect_status_mismatch"}}, _} = phase(p, "evaluated")
    assert {{:ok, _}, _} = phase(p, "evaluated", "NOT_RUN")
    assert {{:error, {:conflict, _}}, _} = send_command(p, "complete", %{})
    assert {{:ok, _}, _} = observe(p, "effect", "FAIL")
    assert {{:ok, _}, _} = phase(p, "evaluated", "FAIL")
    assert {{:error, {:conflict, _}}, _} = send_command(p, "complete", %{})
    assert {{:ok, _}, _} = observe(p, "effect", "UNKNOWN")
    assert {{:error, {:conflict, "effect_status_mismatch"}}, _} = phase(p, "evaluated")
    assert {{:ok, _}, _} = phase(p, "evaluated", "UNKNOWN")
    assert {{:ok, _}, _} = observe(p, "effect")
    assert {{:ok, _}, _} = phase(p, "evaluated")
    assert {{:ok, _}, _} = send_command(p, "complete", %{})
  end

  test "replay is idempotent and retarget invalidates progress without deleting evidence" do
    p = fixture()
    assert {{:ok, ack}, command} = phase(p, "baseline")
    assert {:ok, ^ack} = State.command(p["id"], command)
    assert {{:ok, _}, _} = retarget(p, "after")
    assert {{:ok, _}, _} = observe(p, "verify")
    assert {{:ok, _}, _} = phase(p, "verified")
    assert {{:ok, _}, _} = phase(p, "applied")
    assert {{:ok, _}, _} = observe(p, "effect")
    assert {{:ok, _}, _} = phase(p, "evaluated")
    assert {{:ok, _}, _} = retarget(p, "third")
    assert {:ok, %{work: w}} = State.get(p["id"])
    assert w["document"]["improvement_progress"]["phase"] == "baseline"
    assert w["document"]["improvement_progress"]["baseline"]["evidence"] == evidence()
    assert {{:error, {:conflict, _}}, _} = phase(p, "applied")
    assert {{:error, {:conflict, _}}, _} = send_command(p, "complete", %{})
    assert {:ok, events} = State.history(p["id"], 0)
    assert Enum.count(events, &(&1["request"]["phase"] == "baseline")) == 1
  end

  test "old effect cannot be reused as new measurement" do
    p = fixture()
    phase(p, "baseline")
    observe(p, "verify")
    assert {{:ok, ack}, _} = observe(p, "effect")
    phase(p, "verified")
    phase(p, "applied")

    assert {{:ok, _}, _} =
             send_command(p, "reuse", %{
               "observation_id" => ack["observation_id"],
               "reason" => "reuse old effect"
             })

    assert {{:error, {:conflict, "effect_status_mismatch"}}, _} = phase(p, "evaluated")
  end

  test "baseline unknown, changed verification and stale stage cannot advance" do
    p = fixture()
    assert {{:ok, _}, _} = phase(p, "baseline", "UNKNOWN")
    assert {{:error, {:conflict, _}}, _} = phase(p, "verified")
    phase(p, "baseline")
    observe(p, "verify")
    phase(p, "verified")
    observe(p, "verify", "FAIL")
    assert {{:error, {:conflict, "improvement_verification_missing"}}, _} = phase(p, "applied")

    fields =
      Map.merge(context(p), %{
        "phase" => "applied",
        "status" => "PASS",
        "reason" => "stale",
        "evidence" => evidence(),
        "artifact_ref" => "wrong"
      })

    assert {{:error, {:conflict, "stale_improvement_context"}}, _} =
             send_command(p, "improvement", fields)
  end

  test "invalid contract and stage rejected, normal tasks retain old contract" do
    p = fixture()

    for change <- [
          %{"effect_check" => "missing"},
          %{"effect_check" => "verify"},
          %{"verification_checks" => []},
          %{"extra" => true}
        ] do
      invalid =
        p
        |> Map.put("id", Ecto.UUID.generate())
        |> Map.put("command_id", Ecto.UUID.generate())
        |> Map.update!("improvement", &Map.merge(&1, change))

      assert {:error, {:invalid, _}} = State.create(invalid)
    end

    fields =
      Map.merge(context(p), %{
        "phase" => "magic",
        "status" => "PASS",
        "reason" => "invalid",
        "evidence" => evidence()
      })

    assert {{:error, {:invalid, _}}, _} = send_command(p, "improvement", fields)

    normal =
      Map.drop(p, ["improvement"])
      |> Map.put("id", Ecto.UUID.generate())
      |> Map.put("command_id", Ecto.UUID.generate())

    assert {:ok, _} = State.create(normal)
    assert {{:error, {:invalid, "not_improvement_work"}}, _} = phase(normal, "baseline")
  end

  test "non-applicable effect remains distinct and latest failure reopens completion gate" do
    p = fixture()
    phase(p, "baseline")
    observe(p, "verify")
    phase(p, "verified")
    phase(p, "applied")
    observe(p, "effect", "NOT_APPLICABLE")
    assert {{:ok, _}, _} = phase(p, "evaluated", "NOT_APPLICABLE")
    assert {{:error, {:conflict, _}}, _} = send_command(p, "complete", %{})
    observe(p, "effect")
    phase(p, "evaluated")
    observe(p, "verify", "FAIL")
    assert {{:error, {:conflict, _}}, _} = send_command(p, "complete", %{})
  end
end

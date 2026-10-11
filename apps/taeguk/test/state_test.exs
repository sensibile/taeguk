defmodule Taeguk.StateTest do
  use ExUnit.Case, async: false
  alias Taeguk.{Repo, State}
  @moduletag skip: !Application.compile_env(:taeguk, :state_enabled, false)

  setup_all do
    path = Application.fetch_env!(:taeguk, Repo)[:database]

    unless is_binary(path) and String.ends_with?(path, "-test.sqlite3"),
      do: raise("state tests require a dedicated *-test.sqlite3 file")

    :ok
  end

  defp fixture(method \\ "mixed", required \\ true) do
    p = %{
      "id" => Ecto.UUID.generate(),
      "command_id" => Ecto.UUID.generate(),
      "actor" => "test",
      "purpose" => "prevent repeated work",
      "requirement_ref" => "req:1",
      "rule_ref" => "rules:1",
      "design_ref" => "design:1",
      "plan_ref" => "plan:1",
      "artifact_ref" => "head:1",
      "environment_ref" => "env:1",
      "checks" => [
        %{
          "id" => "C1",
          "method" => method,
          "required" => required,
          "oracle_ref" => "oracle:1",
          "accepted_statuses" => ["PASS"]
        }
      ]
    }

    assert {:ok, _} = State.create(p)
    p
  end

  defp cmd(revision, operation, fields \\ %{}) do
    Map.merge(
      %{
        "command_id" => Ecto.UUID.generate(),
        "expected_revision" => revision,
        "actor" => "test",
        "operation" => operation
      },
      fields
    )
  end

  defp observation(revision, kind \\ "tool", status \\ "PASS") do
    cmd(revision, "observe", %{
      "check_id" => "C1",
      "kind" => kind,
      "status" => status,
      "artifact_ref" => "head:1",
      "environment_ref" => "env:1",
      "plan_ref" => "plan:1",
      "reason" => "actual fixture evidence",
      "evidence" => [%{"location" => "fixture.log", "sha256" => String.duplicate("a", 64)}]
    })
  end

  defp count(table, id),
    do: Repo.query!("SELECT count(*) FROM #{table} WHERE work_id=?", [id]).rows |> hd() |> hd()

  test "create replay and conflicting reuse preserve history" do
    p = fixture()
    assert {:ok, %{"revision" => 1}} = State.create(p)

    assert {:error, {:conflict, "command_id_reused"}} =
             State.create(%{p | "purpose" => "different"})

    o = observation(1)
    assert {:ok, ack} = State.command(p["id"], o)
    assert {:ok, _} = State.command(p["id"], observation(2, "llm"))
    assert {:ok, ^ack} = State.command(p["id"], o)

    assert {:error, {:conflict, "command_id_reused"}} =
             State.command(p["id"], %{o | "status" => "FAIL"})

    assert count("observations", p["id"]) == 2
    assert {:ok, %{work: %{"revision" => 3}}} = State.get(p["id"])
    assert {:ok, events} = State.history(p["id"], 1)
    assert length(events) == 2
  end

  test "mixed completion requires both layers and accepted current statuses" do
    p = fixture()

    assert {:error, {:conflict, "completion_evidence_missing"}} =
             State.command(p["id"], cmd(1, "complete"))

    assert {:ok, _} = State.command(p["id"], observation(1))
    assert {:ok, _} = State.command(p["id"], observation(2, "llm", "UNKNOWN"))
    assert {:error, {:conflict, _}} = State.command(p["id"], cmd(3, "complete"))
    assert {:ok, _} = State.command(p["id"], observation(3, "llm"))
    assert {:ok, _} = State.command(p["id"], cmd(4, "complete"))
    assert {:error, {:conflict, "work_completed"}} = State.command(p["id"], observation(5))
    assert {:ok, %{work: %{"completed" => true}}} = State.get(p["id"])
  end

  test "retarget invalidates old observations and explicit reuse appends provenance" do
    p = fixture("tool")
    assert {:ok, ack} = State.command(p["id"], observation(1))

    assert {:ok, _} =
             State.command(
               p["id"],
               cmd(2, "retarget", %{
                 "artifact_ref" => "head:2",
                 "environment_ref" => "env:1",
                 "reason" => "changed head"
               })
             )

    assert {:ok, %{observations: [], blockers: [_]}} = State.get(p["id"])

    assert {:error, {:conflict, "stale_observation_context"}} =
             State.command(p["id"], observation(3))

    assert {:ok, _} =
             State.command(
               p["id"],
               cmd(3, "reuse", %{
                 "observation_id" => ack["observation_id"],
                 "reason" => "unaffected path verified"
               })
             )

    assert {:ok, %{observations: [o], blockers: []}} = State.get(p["id"])
    assert o["document"]["reused_from"] == ack["observation_id"]
    assert count("observations", p["id"]) == 2
  end

  test "two actual connections with same revision have one winner" do
    p = fixture("tool")
    parent = self()

    tasks =
      for _ <- 1..2 do
        Task.async(fn ->
          Repo.checkout(fn ->
            send(parent, {:ready, self()})

            receive do
              :go -> State.command(p["id"], observation(1))
            end
          end)
        end)
      end

    workers =
      for _ <- 1..2 do
        receive do
          {:ready, pid} -> pid
        after
          5000 -> flunk("missing connection")
        end
      end

    Enum.each(workers, &send(&1, :go))
    results = Enum.map(tasks, &Task.await(&1, 10000))
    assert Enum.count(results, &match?({:ok, _}, &1)) == 1
    assert Enum.count(results, &match?({:error, {:conflict, "stale_revision"}}, &1)) == 1
    assert count("state_events", p["id"]) == 2
    assert count("observations", p["id"]) == 1
  end

  test "resource collision rolls back run and owner controls release" do
    p = fixture()
    other = fixture()
    resource = "db:" <> Ecto.UUID.generate()
    start = cmd(1, "start_run", %{"label" => "integration", "resources" => [resource]})
    assert {:ok, ack} = State.command(p["id"], start)
    assert {:error, {:conflict, "resource_busy"}} = State.command(other["id"], start)
    assert count("state_runs", other["id"]) == 0
    assert count("state_events", other["id"]) == 1

    ending =
      cmd(2, "end_run", %{
        "run_id" => ack["run_id"],
        "status" => "finished",
        "cleanup_ref" => "cleanup.log",
        "result_ref" => "result.log"
      })

    assert {:error, {:conflict, "run_not_owned_or_not_running"}} =
             State.command(p["id"], %{ending | "actor" => "someone-else"})

    assert {:error, {:conflict, "run_still_active"}} =
             State.command(
               p["id"],
               cmd(2, "retarget", %{
                 "artifact_ref" => "head:2",
                 "environment_ref" => "env:1",
                 "reason" => "changed"
               })
             )

    assert {:ok, _} = State.command(p["id"], ending)
    assert {:ok, _} = State.command(other["id"], start)
  end

  test "event insertion failure rolls back observation and revision" do
    p = fixture("tool")

    Repo.query!(
      "CREATE TRIGGER injected_failure BEFORE INSERT ON state_events WHEN json_extract(NEW.request,'$.actor')='fault' BEGIN SELECT RAISE(ABORT,'injected'); END"
    )

    try do
      assert {:error, {:unavailable, _}} =
               State.command(p["id"], %{observation(1) | "actor" => "fault"})

      assert count("observations", p["id"]) == 0
      assert count("state_events", p["id"]) == 1
      assert {:ok, %{work: %{"revision" => 1}}} = State.get(p["id"])
    after
      Repo.query!("DROP TRIGGER injected_failure")
    end
  end

  test "invalid fields, kind, hash and unaccepted N/A cannot complete" do
    p = fixture("tool")

    assert {:error, {:invalid, _}} =
             State.command(p["id"], Map.put(observation(1), "extra", true))

    assert {:error, {:invalid, _}} = State.command(p["id"], observation(1, "llm"))
    assert {:error, {:invalid, _}} = State.command(p["id"], %{observation(1) | "reason" => ""})

    assert {:error, {:invalid, _}} =
             State.command(p["id"], %{
               observation(1)
               | "evidence" => [%{"location" => "log", "sha256" => "bad"}]
             })

    assert {:ok, _} = State.command(p["id"], observation(1, "tool", "NOT_APPLICABLE"))
    assert {:error, {:conflict, _}} = State.command(p["id"], cmd(2, "complete"))
    empty = fixture("tool", false)
    assert {:error, {:conflict, _}} = State.command(empty["id"], cmd(1, "complete"))
  end

  test "API authentication and JSON validation" do
    assert Plug.Test.conn(:get, "/api/work-items")
           |> TaegukWeb.Router.call(TaegukWeb.Router.init([]))
           |> Map.fetch!(:status) == 401

    token = Application.fetch_env!(:taeguk, :state_token)

    conn =
      Plug.Test.conn(:post, "/api/work-items", Jason.encode!(%{"unexpected" => true}))
      |> Plug.Conn.put_req_header("content-type", "application/json")
      |> Plug.Conn.put_req_header("x-taeguk-token", token)

    assert TaegukWeb.Router.call(conn, TaegukWeb.Router.init([])).status == 422
  end

  test "different work items contend for one resource on separate connections" do
    first = fixture()
    second = fixture()
    resource = "parallel:" <> Ecto.UUID.generate()
    parent = self()

    tasks =
      for p <- [first, second] do
        Task.async(fn ->
          Repo.checkout(fn ->
            send(parent, {:ready, self()})

            receive do
              :go ->
                State.command(
                  p["id"],
                  cmd(1, "start_run", %{"label" => "parallel", "resources" => [resource]})
                )
            end
          end)
        end)
      end

    workers =
      for _ <- 1..2 do
        receive do
          {:ready, pid} -> pid
        after
          5000 -> flunk("missing connection")
        end
      end

    Enum.each(workers, &send(&1, :go))
    results = Enum.map(tasks, &Task.await(&1, 10000))
    assert Enum.count(results, &match?({:ok, _}, &1)) == 1
    assert Enum.count(results, &match?({:error, {:conflict, "resource_busy"}}, &1)) == 1

    assert Repo.query!("SELECT count(*) FROM resource_claims WHERE resource=?", [resource]).rows ==
             [[1]]

    assert count("state_runs", first["id"]) + count("state_runs", second["id"]) == 1
  end

  test "allowed N/A with reason can complete" do
    p = fixture("tool")

    other = %{
      p
      | "id" => Ecto.UUID.generate(),
        "command_id" => Ecto.UUID.generate(),
        "checks" => Enum.map(p["checks"], &Map.put(&1, "accepted_statuses", ["NOT_APPLICABLE"]))
    }

    assert {:ok, _} = State.create(other)
    assert {:ok, _} = State.command(other["id"], observation(1, "tool", "NOT_APPLICABLE"))
    assert {:ok, _} = State.command(other["id"], cmd(2, "complete"))
  end

  test "history is paged without dropping later events" do
    p = fixture("tool")
    for revision <- 1..27, do: assert({:ok, _} = State.command(p["id"], observation(revision)))
    assert {:ok, page1} = State.history(p["id"], 0)
    assert length(page1) == 25
    assert {:ok, page2} = State.history(p["id"], 25)
    assert Enum.map(page2, & &1["revision"]) == [26, 27, 28]
  end

  test "known SQLite contention is distinct from general SQL failure" do
    p = fixture("tool")
    path = Application.fetch_env!(:taeguk, Repo)[:database]
    {:ok, connection} = Exqlite.Sqlite3.open(path)

    try do
      :ok = Exqlite.Sqlite3.execute(connection, "BEGIN IMMEDIATE")
      assert {:error, {:unavailable, "database_busy"}} = State.command(p["id"], observation(1))
      :ok = Exqlite.Sqlite3.execute(connection, "ROLLBACK")
      assert {:ok, _} = State.command(p["id"], observation(1))
    after
      Exqlite.Sqlite3.close(connection)
    end
  end
end

defmodule Taeguk.Repo.Migrations.CreateWorkflowState do
  use Ecto.Migration

  def change do
    execute("""
    CREATE TABLE work_items (
      id text PRIMARY KEY,
      revision integer NOT NULL CHECK (revision > 0),
      document text NOT NULL,
      completed integer NOT NULL DEFAULT 0 CHECK (completed IN (0,1)),
      updated_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    )
    """, "DROP TABLE work_items")

    execute("""
    CREATE TABLE state_events (
      work_id text NOT NULL REFERENCES work_items(id),
      command_id text NOT NULL,
      revision integer NOT NULL,
      request text NOT NULL,
      ack text NOT NULL,
      created_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
      PRIMARY KEY(work_id, command_id),
      UNIQUE(work_id, revision)
    )
    """, "DROP TABLE state_events")

    execute("""
    CREATE TABLE observations (
      id text PRIMARY KEY,
      work_id text NOT NULL REFERENCES work_items(id),
      revision integer NOT NULL,
      document text NOT NULL
    )
    """, "DROP TABLE observations")

    execute("CREATE INDEX observations_work_id_revision_idx ON observations(work_id, revision DESC)", "DROP INDEX observations_work_id_revision_idx")

    execute("""
    CREATE TABLE state_runs (
      id text PRIMARY KEY,
      work_id text NOT NULL REFERENCES work_items(id),
      owner text NOT NULL,
      label text NOT NULL,
      context text NOT NULL,
      handle text,
      status text NOT NULL CHECK (status IN ('running','finished','interrupted')),
      cleanup_ref text,
      result_ref text,
      started_at text NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
      ended_at text
    )
    """, "DROP TABLE state_runs")

    execute("""
    CREATE TABLE resource_claims (
      resource text PRIMARY KEY,
      run_id text NOT NULL REFERENCES state_runs(id)
    )
    """, "DROP TABLE resource_claims")
  end
end

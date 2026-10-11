"""Small counterexamples for the book review, not production implementations.

Run: python3 experiments/book_review_probes.py
Uses only the standard library and disposable databases in a temporary directory.
"""

import itertools
import json
import os
from pathlib import Path
import platform
import sqlite3
import subprocess
import sys
import tempfile


def uniqueness_probe():
    # Three individually plausible properties miss an unwanted extra element.
    weak_passes = 0
    strong_rejections = 0
    count = 0
    for size in range(5):
        for values in itertools.product(range(3), repeat=size):
            result = sorted(set(values) | {99}, reverse=True)
            weak = (
                all(x in result for x in values)
                and len(result) == len(set(result))
                and result == sorted(result, reverse=True)
            )
            strong = weak and set(result) == set(values)
            count += 1
            weak_passes += weak
            strong_rejections += not strong
    assert weak_passes == strong_rejections == count
    return dict(cases=count, wrong_outputs_accepted_by_weak=weak_passes,
                wrong_outputs_rejected_by_strong=strong_rejections)


def conversion_probe():
    # Own example: an accidental integer conversion in an extracted boundary.
    def extracted(value):
        return int(value)

    return dict(whole_number_hides_error=extracted(12.0) == 12.0,
                fractional_number_detects_error=extracted(12.5) != 12.5)


def check_then_write_probe():
    # Enumerate schedules preserving read-before-write in each participant.
    # Abstract shared-memory model; this is NOT a DB isolation implementation.
    violations = []
    valid_schedules = 0
    for order in itertools.permutations(("A.read", "A.write", "B.read", "B.write")):
        if any(order.index(who + ".read") > order.index(who + ".write")
               for who in ("A", "B")):
            continue
        valid_schedules += 1
        active = {"A": True, "B": True}
        allowed = {}
        for event in order:
            who, action = event.split(".")
            if action == "read":
                allowed[who] = sum(active.values()) > 1
            elif allowed[who]:
                active[who] = False
        if not any(active.values()):
            violations.append(order)
    assert len(violations) == 4 and valid_schedules == 6
    return dict(model="read-check-write without a shared atomic boundary",
                schedules=valid_schedules, invariant_violations=len(violations),
                example=violations[0])


def create_db(path):
    with sqlite3.connect(path) as conn:
        conn.executescript("""
            CREATE TABLE processed (request_id TEXT PRIMARY KEY);
            CREATE TABLE balance (id INTEGER PRIMARY KEY, amount INTEGER NOT NULL);
            INSERT INTO balance VALUES (1, 0);
        """)


def worker(path, mode, stop):
    conn = sqlite3.connect(path, isolation_level=None)
    conn.execute("PRAGMA synchronous=FULL")
    conn.execute("BEGIN IMMEDIATE")
    if conn.execute("SELECT 1 FROM processed WHERE request_id='r1'").fetchone():
        conn.execute("COMMIT")
        return
    if stop == "before_marker":
        os._exit(73)
    conn.execute("INSERT INTO processed VALUES ('r1')")
    if mode == "split":
        conn.execute("COMMIT")
        if stop == "after_marker":
            os._exit(73)
        conn.execute("BEGIN IMMEDIATE")
    elif stop == "after_marker":
        os._exit(73)
    conn.execute("UPDATE balance SET amount=amount+7 WHERE id=1")
    if stop == "after_effect":
        os._exit(73)
    conn.execute("COMMIT")
    if stop == "after_commit":
        os._exit(73)
    conn.close()


def run_worker(path, mode, stop):
    proc = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "--worker", str(path), mode, stop],
        capture_output=True, text=True, timeout=10,
    )
    expected = 0 if stop == "none" else 73
    if proc.returncode != expected:
        raise RuntimeError(f"Worker exit {proc.returncode}: {proc.stderr}")


def read_state(path):
    with sqlite3.connect(path) as conn:
        return dict(markers=conn.execute("SELECT COUNT(*) FROM processed").fetchone()[0],
                    amount=conn.execute("SELECT amount FROM balance WHERE id=1").fetchone()[0])


def replay_probe():
    results = []
    with tempfile.TemporaryDirectory(prefix="book-review-") as tmp:
        for stop in ("before_marker", "after_marker", "after_effect", "after_commit"):
            path = Path(tmp) / (stop + ".sqlite")
            create_db(path)
            run_worker(path, "atomic", stop)
            interrupted = read_state(path)
            run_worker(path, "atomic", "none")
            run_worker(path, "atomic", "none")
            final = read_state(path)
            assert final == dict(markers=1, amount=7)
            results.append(dict(stop=stop, interrupted=interrupted, after_two_replays=final))
        path = Path(tmp) / "split.sqlite"
        create_db(path)
        run_worker(path, "split", "after_marker")
        run_worker(path, "split", "none")
        broken = read_state(path)
        assert broken == dict(markers=1, amount=0)
    return dict(atomic=results, split_transaction_counterexample=broken,
                limits="One local SQLite DB; process exit, not power loss; no broker or external effects; sequential replay.")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--worker":
        worker(*sys.argv[2:])
    else:
        report = dict(
            python=platform.python_version(), sqlite=sqlite3.sqlite_version,
            uniqueness=uniqueness_probe(), conversion=conversion_probe(),
            interleavings=check_then_write_probe(), idempotent_replay=replay_probe(),
        )
        print(json.dumps(report, ensure_ascii=False, indent=2))

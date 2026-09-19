#!/usr/bin/env python3
"""Synthetic, content-free activity ledger generator (S0-T6).

Produces JSONL rows shaped like `activity_events` (see docs/PERSONAL_AI_OS.md §7.1) for a
persona over N weeks. With --drift, the final two weeks shift the routine: later work end,
less reading, fewer exercise sessions, fewer family messages, more postponed personal tasks.

Usage:
  python3 scripts/synth_ledger.py --weeks 6 --drift > /tmp/ledger.jsonl
  python3 scripts/synth_ledger.py --weeks 4 --user 11111111-1111-1111-1111-111111111111

Load into Postgres with: cargo run --bin load_ledger -- /tmp/ledger.jsonl  (S2-T3)
"""
import argparse, datetime as dt, hashlib, hmac, json, random, sys, uuid

WORK_AGENTS = ["productivity", "email", "team_comms", "project"]
HOME_AGENTS = ["family", "personal_productivity", "health_wellness", "entertainment"]
KEY = b"synthetic-ledger-key"


def h(subject: str) -> str:
    return hmac.new(KEY, subject.encode(), hashlib.sha256).hexdigest()[:24]


def ev(user, ts, domain, agent, kind, effect=None, duration_ms=None, subject=None, meta=None, trace=None):
    row = {
        "user_id": user,
        "ts": ts.isoformat(timespec="seconds") + "Z",
        "domain": domain,
        "agent_id": agent,
        "kind": kind,
        "effect": effect,
        "duration_ms": duration_ms,
        "subject_hash": h(subject) if subject else None,
        "meta": meta or {},
        "trace_id": trace,
    }
    return json.dumps(row, separators=(",", ":"))


def day_events(user, day, rng, drift):
    rows = []
    weekend = day.weekday() >= 5
    base = dt.datetime.combine(day, dt.time(0, 0))
    trace = str(uuid.uuid4())

    if not weekend:
        start_h = rng.gauss(9.0, 0.25)
        end_h = rng.gauss(19.3 if drift else 17.5, 0.35)
        work_minutes = int((end_h - start_h) * 60 - 45)
        # work blocks: tool calls spread through the day, attributed to work agents
        t = base + dt.timedelta(hours=start_h)
        rows.append(ev(user, t, "work", "mother", "intent", meta={"kind": "briefing"}, trace=trace))
        n_calls = max(8, int(work_minutes / 25))
        for i in range(n_calls):
            t = base + dt.timedelta(hours=start_h + (end_h - start_h) * (i + 0.5) / n_calls)
            agent = rng.choice(WORK_AGENTS)
            effect = "read" if rng.random() < 0.8 else "write_local"
            rows.append(ev(user, t, "work", agent, "tool_call", effect, int(rng.uniform(400, 4000)),
                           subject=f"thread-{rng.randint(1, 40)}", meta={"tool_class": effect}, trace=trace))
        rows.append(ev(user, base + dt.timedelta(hours=end_h), "work", "productivity", "session_focus",
                       duration_ms=work_minutes * 60_000, meta={"minutes": work_minutes}))
        # family messages (fewer under drift)
        for _ in range(max(0, int(rng.gauss(2.0 if drift else 3.5, 1.0)))):
            t = base + dt.timedelta(hours=rng.uniform(18.5, 22.0))
            rows.append(ev(user, t, "home", "family", "tool_call", "read", 300,
                           subject=f"family-{rng.randint(1, 6)}", meta={"channel": "chat"}))
        # personal task postponed (more under drift)
        if rng.random() < (0.55 if drift else 0.2):
            rows.append(ev(user, base + dt.timedelta(hours=18.2), "home", "personal_productivity", "task_postponed",
                           subject=f"task-{rng.randint(1, 5)}", meta={"postponed_count": rng.randint(1, 5)}))
    else:
        for _ in range(int(rng.gauss(5, 1.5))):
            t = base + dt.timedelta(hours=rng.uniform(10, 21))
            rows.append(ev(user, t, "home", rng.choice(HOME_AGENTS), "tool_call", "read", 500,
                           meta={"weekend": True}))
        for _ in range(max(0, int(rng.gauss(3 if drift else 5, 1.2)))):
            t = base + dt.timedelta(hours=rng.uniform(9, 21))
            rows.append(ev(user, t, "home", "family", "tool_call", "read", 300,
                           subject=f"family-{rng.randint(1, 6)}", meta={"channel": "chat"}))

    # reading (evening); drift cuts it sharply
    read_min = max(0, rng.gauss(9 if drift else 30, 6))
    if read_min > 2:
        rows.append(ev(user, base + dt.timedelta(hours=rng.uniform(20.5, 22.5)), "shared", "reading_knowledge",
                       "reading", duration_ms=int(read_min * 60_000), subject=f"article-{rng.randint(1, 200)}",
                       meta={"minutes": round(read_min, 1), "topic_id": rng.randint(1, 12)}))
    # exercise: ~3/wk normal, ~1/wk drift
    if rng.random() < ((1 / 7) if drift else (3 / 7)):
        rows.append(ev(user, base + dt.timedelta(hours=rng.choice([7.0, 18.0])), "home", "health_wellness",
                       "exercise", duration_ms=int(rng.uniform(30, 60) * 60_000), meta={"kind": "session"}))
    # sleep (previous night), hours as duration
    sleep_h = rng.gauss(6.5 if drift else 7.1, 0.4)
    rows.append(ev(user, base + dt.timedelta(hours=7.0), "home", "health_wellness", "sleep",
                   duration_ms=int(sleep_h * 3_600_000), meta={"hours": round(sleep_h, 2)}))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--weeks", type=int, default=6)
    ap.add_argument("--drift", action="store_true", help="shift routine in the final two weeks")
    ap.add_argument("--user", default="00000000-0000-0000-0000-000000000001")
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--end", default=None, help="last day YYYY-MM-DD (default: today)")
    a = ap.parse_args()

    rng = random.Random(a.seed)
    end = dt.date.fromisoformat(a.end) if a.end else dt.date.today()
    days = a.weeks * 7
    drift_from = days - 14 if a.drift else days + 1
    out = sys.stdout
    n = 0
    for i in range(days):
        day = end - dt.timedelta(days=days - 1 - i)
        for row in day_events(a.user, day, rng, drift=i >= drift_from):
            out.write(row + "\n")
            n += 1
    print(f"# wrote {n} events for {days} days (drift={'yes' if a.drift else 'no'})", file=sys.stderr)


if __name__ == "__main__":
    main()

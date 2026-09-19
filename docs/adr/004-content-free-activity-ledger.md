# ADR-004 — The activity ledger is append-only and content-free

**Status:** Accepted · **Date:** 2026-09-19 · **Sprint:** S0 (decision) · S2 (implementation) · **Concept:** `docs/PERSONAL_AI_OS.md` §7.1

## Context

The intelligence layer (patterns, baseline, balance, behaviour) needs a record of *what happened* across
both worlds. Storing message bodies or document content would turn a self-awareness feature into
surveillance and multiply the privacy surface.

## Decision

`activity_events` records who (agent), what kind of thing (intent, tool call, card resolve, approval,
observation, …), which domain, which effect class, how long, and a **keyed hash** of the subject — never
the subject itself. `meta` is a small JSON object restricted to counts, categories and durations and is
schema-checked before write. Raw events are retained 180 days; aggregates longer. The user can delete
the ledger.

## Consequences

- Follow-up tracking works on hashed thread ids; balance and baseline work on counts and durations.
- Logs and dumps of the ledger cannot leak content.
- Local-first mode (§15) is possible later because nothing in the ledger requires cloud storage.

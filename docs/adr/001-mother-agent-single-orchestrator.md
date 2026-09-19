# ADR-001 — The Mother Agent is the single orchestrator

**Status:** Accepted · **Date:** 2026-09-19 · **Sprint:** S0 · **Concept:** `docs/PERSONAL_AI_OS.md` §3

## Context

Phase 1 routes an intent with an `LlmConditionalAgent` (`src/agents/router.rs`) to one of seven
scenario keys, streams that scenario's workflow, and asks a separate summarizer (`src/agents/suzy.rs`)
for closing prose. Two agents share the "coordinator" role and neither holds the user's context,
delegates across life domains, or arbitrates between agents.

## Decision

One **Mother Agent** (`src/mother/`) owns intake → context assembly → delegation → arbitration →
synthesis → action gate. Suzy remains its voice. The Mother Agent has **no MCP tools**; its toolset is
internal (`get_context`, `delegate`, `read_memory`, `propose_memory`, `read_observations`, `compose`).
The existing router becomes the first hop of intake; the existing summarizer becomes the synthesis step.
All entry points (intent route, chat route, voice tools, `/awp/a2a`) dispatch through the Mother Agent.

## Consequences

- A wrong routing decision can only mean "wrong agent asked", never "wrong email sent".
- Multi-target intents (work + home) are possible in one turn; the seven Phase 1 scenarios remain valid
  delegation targets so the demo tour keeps working.
- Latency budget: intake and synthesis use the small model; delegation runs targets in parallel.
- Deterministic fallbacks (keyword intake, template synthesis) keep the system usable without an API key.

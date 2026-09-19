# ADR-003 — Authority is Observe / Suggest / Automate, enforced by tool effect class

**Status:** Accepted · **Date:** 2026-09-19 · **Sprint:** S0 (types) · S2 (gate) · **Concept:** `docs/PERSONAL_AI_OS.md` §10

## Context

Phase 1 limits *which* tools an agent can see (`mcp_allowlists.toml`, `FilteredToolset`) but not
*what it may do* with them. "Commit" acknowledges a label without executing or auditing.

## Decision

Each agent has a **mode**: `observe`, `suggest` or `automate` (user-overridable per agent and per tool).
Each allowlisted tool has an **effect class**: `read`, `write_local`, `schedule_with_others`,
`send_external`, `publish_public`, `financial`, `delete`. A `PermissionGate` toolset wrapper decides by
mode × effect: pass through (and log), enqueue a **pending action**, or deny. `publish_public` and
`financial` are never automatable. Missing effects are a boot failure from S2 onward (a warning in S0).

## Consequences

- New MCP servers slot in by classifying their tools; the gate needs no new rules.
- Approvals, audit and undo live outside the LLM loop, so a prompt-injected instruction can at most
  create a pending action.
- External AWP callers are capped at Suggest regardless of the user's own modes.

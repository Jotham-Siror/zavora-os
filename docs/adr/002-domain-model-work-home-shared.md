# ADR-002 — Every agent, card, event and memory item carries a domain

**Status:** Accepted · **Date:** 2026-09-19 · **Sprint:** S0 · **Concept:** `docs/PERSONAL_AI_OS.md` §2.5, §4, §5

## Context

The Personal AI OS separates a Work World from a Home World and reconciles them only in the Mother
Agent and the Balance Agent. Phase 1 has only a hint of this split (People rail work/family).

## Decision

Introduce `Domain { Work, Home, Shared }` (`src/domain.rs`). It is a mandatory column on every Phase 2
table (`activity_events`, `memory_items`, `pending_actions`, `audit_log`, …) and a serialized field on
`CardRecord`, `AgentRecord` and the `card_spawn` SSE event (default `shared` for backward compatibility).
Agents declare their world in `mcp_allowlists.toml`; memory scopes are prefixed by domain (`work.*`,
`home.*`, `shared.*`). Cross-domain reads are only allowed through the Mother/Balance scoped API.

## Consequences

- Isolation is enforceable in code (scope checks) and visible in the UI (domain colour).
- The activity ledger can compute Work vs Home attention because every event is tagged.
- Existing JSON blobs deserialize unchanged thanks to `#[serde(default)]`.

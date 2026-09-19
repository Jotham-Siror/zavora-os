//! The Mother Agent — single orchestrator (ADR-001).
//!
//! Pipeline: intake → context → delegation → arbitration → synthesis → action gate.
//! S0 declares the module; S1 fills `intake`, `agent`, `delegate`, `synth` and wires every
//! entry point (intent, chat, voice, `/awp/a2a`) through it.

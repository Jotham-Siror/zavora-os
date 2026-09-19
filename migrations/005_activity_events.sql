-- Phase 2 · S2-T1: content-free activity ledger (ADR-004).
-- No message bodies, subjects or document content — subjects are keyed hashes, meta holds
-- counts, categories and durations only. Retention: 180 days raw (S11 purge job).
CREATE TABLE IF NOT EXISTS activity_events (
    id           BIGSERIAL PRIMARY KEY,
    user_id      TEXT        NOT NULL,
    ts           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    domain       TEXT        NOT NULL CHECK (domain IN ('work', 'home', 'shared')),
    agent_id     TEXT        NOT NULL,
    kind         TEXT        NOT NULL,
    effect       TEXT,
    duration_ms  INTEGER,
    subject_hash TEXT,
    meta         JSONB       NOT NULL DEFAULT '{}',
    trace_id     TEXT
);
CREATE INDEX IF NOT EXISTS idx_activity_events_user_ts ON activity_events (user_id, ts);
CREATE INDEX IF NOT EXISTS idx_activity_events_user_domain_kind ON activity_events (user_id, domain, kind, ts);

-- Phase 2 · S2-T6: authority modes, pending actions, audit log (ADR-003).
CREATE TABLE IF NOT EXISTS agent_permissions (
    user_id        TEXT NOT NULL,
    agent_id       TEXT NOT NULL,
    mode           TEXT NOT NULL CHECK (mode IN ('observe', 'suggest', 'automate')),
    tool_overrides JSONB NOT NULL DEFAULT '{}',   -- { "send_draft": "observe", ... }
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, agent_id)
);

CREATE TABLE IF NOT EXISTS pending_actions (
    id           UUID PRIMARY KEY,
    user_id      TEXT NOT NULL,
    session_id   TEXT,
    agent_id     TEXT NOT NULL,
    domain       TEXT NOT NULL,
    tool         TEXT NOT NULL,
    effect       TEXT NOT NULL,
    args         JSONB NOT NULL DEFAULT '{}',     -- encrypted at rest from S3 when sensitive
    summary      TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected', 'expired', 'failed')),
    trace_id     TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at   TIMESTAMPTZ NOT NULL,
    resolved_at  TIMESTAMPTZ,
    result       JSONB
);
CREATE INDEX IF NOT EXISTS idx_pending_actions_user_status ON pending_actions (user_id, status, created_at);

CREATE TABLE IF NOT EXISTS audit_log (
    id           UUID PRIMARY KEY,
    user_id      TEXT NOT NULL,
    session_id   TEXT,
    agent_id     TEXT NOT NULL,
    domain       TEXT NOT NULL,
    tool         TEXT NOT NULL,
    effect       TEXT NOT NULL,
    decision     TEXT NOT NULL,                   -- allowed | approved | rejected | denied | queued | failed
    approval_id  UUID,
    recipe_id    UUID,
    mode         TEXT NOT NULL,
    summary      TEXT NOT NULL,
    undo_token   TEXT,
    undone_at    TIMESTAMPTZ,
    trace_id     TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_audit_log_user_ts ON audit_log (user_id, created_at);

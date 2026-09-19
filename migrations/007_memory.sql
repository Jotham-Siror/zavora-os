-- Phase 2 · S3-T1: personal memory (known · assumed · recommended) and minimal consents.
CREATE TABLE IF NOT EXISTS memory_items (
    id           UUID PRIMARY KEY,
    user_id      TEXT NOT NULL,
    domain       TEXT NOT NULL CHECK (domain IN ('work', 'home', 'shared')),
    category     TEXT NOT NULL,
    key          TEXT NOT NULL,
    value        JSONB NOT NULL,                 -- {"enc":"v1","data":"…"} when sensitivity ≥ sensitive
    kind         TEXT NOT NULL CHECK (kind IN ('known', 'assumed', 'recommended')),
    confidence   REAL,
    sensitivity  TEXT NOT NULL DEFAULT 'normal' CHECK (sensitivity IN ('normal', 'sensitive', 'health', 'financial')),
    source_agent TEXT NOT NULL,
    provenance   JSONB NOT NULL DEFAULT '[]',
    consent_id   UUID,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at   TIMESTAMPTZ,
    deleted_at   TIMESTAMPTZ,
    UNIQUE (user_id, domain, key)
);
CREATE INDEX IF NOT EXISTS idx_memory_items_user_kind ON memory_items (user_id, kind) WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS consents (
    id          UUID PRIMARY KEY,
    user_id     TEXT NOT NULL,
    category    TEXT NOT NULL,                   -- calendar | email | health | finance | social | location | reading | routines
    world       TEXT NOT NULL DEFAULT 'shared',
    purpose     TEXT NOT NULL,
    granted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_consents_user ON consents (user_id, category, world);

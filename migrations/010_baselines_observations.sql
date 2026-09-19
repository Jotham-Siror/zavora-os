-- Phase 2 · S7-T1 schema (landed in team sprint A for the Intelligence lane): daily pattern
-- aggregates, rolling personal baselines and observations (concept §7.2, §8.3).
-- Everything here is derived from the content-free ledger — numbers only, never content.
-- Retention: activity_daily 3 y · baselines rolling · observations 365 d (S11 purge job).
-- 009 is reserved for the contacts store (team sprint B).

CREATE TABLE IF NOT EXISTS activity_daily (
    user_id     TEXT NOT NULL,
    day         DATE NOT NULL,
    dimension   TEXT NOT NULL,                 -- work.end, reading.minutes, comms.family.volume, ...
    value       DOUBLE PRECISION NOT NULL,
    sample_n    INTEGER NOT NULL DEFAULT 1,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, day, dimension)
);
CREATE INDEX IF NOT EXISTS idx_activity_daily_user_dimension_day ON activity_daily (user_id, dimension, day);

CREATE TABLE IF NOT EXISTS baselines (
    user_id     TEXT NOT NULL,
    dimension   TEXT NOT NULL,
    day_class   TEXT NOT NULL CHECK (day_class IN ('weekday', 'weekend')),
    median      DOUBLE PRECISION,
    mad         DOUBLE PRECISION,
    sample_n    INTEGER,
    window_end  DATE NOT NULL,
    confirmed   BOOLEAN NOT NULL DEFAULT FALSE, -- the user said "yes, that's my normal"
    computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, dimension, day_class, window_end)
);

CREATE TABLE IF NOT EXISTS observations (
    id            UUID PRIMARY KEY,
    user_id       TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('drift', 'balance', 'behavior', 'pattern', 'conflict')),
    domain        TEXT NOT NULL DEFAULT 'shared' CHECK (domain IN ('work', 'home', 'shared')),
    dimensions    TEXT[] NOT NULL,
    window_days   INTEGER NOT NULL,
    facts         JSONB NOT NULL,              -- numbers only; the phrasing step reads this
    text          TEXT NOT NULL,               -- neutral-language sentence(s), lint-checked (§7.6)
    offer         TEXT,                        -- "Would you like me to help you review what's changed?"
    status        TEXT NOT NULL DEFAULT 'new'
                  CHECK (status IN ('new', 'shown', 'accepted', 'dismissed', 'snoozed')),
    snoozed_until TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    shown_at      TIMESTAMPTZ,
    resolved_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_observations_user_status_created ON observations (user_id, status, created_at);

-- Phase 2 · S4-T3: tasks store shared by Work Productivity and Home Personal Productivity.
-- Deadlines, focus sessions, reminders, errands and household chores are tasks with a kind.
-- `postponed_count` feeds the Balance Agent's postponement debt (§9.1); the ledger gets a
-- content-free `task_postponed` event per postponement (§7.2). Retention: user-controlled
-- (Appendix C) — rows go with the account.
CREATE TABLE IF NOT EXISTS tasks (
    id               UUID PRIMARY KEY,
    user_id          TEXT NOT NULL,
    domain           TEXT NOT NULL CHECK (domain IN ('work', 'home', 'shared')),
    title            TEXT NOT NULL,
    kind             TEXT NOT NULL DEFAULT 'task'
                     CHECK (kind IN ('task', 'deadline', 'focus_session', 'reminder', 'errand', 'household')),
    due              TIMESTAMPTZ,
    duration_minutes INTEGER,
    priority         TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('low', 'normal', 'high')),
    status           TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'done', 'cancelled')),
    postponed_count  INTEGER NOT NULL DEFAULT 0,
    source_agent     TEXT NOT NULL,
    notes            TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at     TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_tasks_user_domain_status_due ON tasks (user_id, domain, status, due);

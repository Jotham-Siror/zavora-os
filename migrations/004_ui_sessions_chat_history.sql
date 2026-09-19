-- Phase 2 · S1-T5: Mother chat transcript per UI session.
ALTER TABLE ui_sessions ADD COLUMN IF NOT EXISTS chat_history JSONB NOT NULL DEFAULT '[]';

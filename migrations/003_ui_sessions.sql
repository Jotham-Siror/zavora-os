CREATE TABLE ui_sessions (
    session_id VARCHAR(255) PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    scenario VARCHAR(64),
    origin_text TEXT,
    artifacts JSONB NOT NULL DEFAULT '{}',
    cards JSONB NOT NULL DEFAULT '[]',
    agents_active JSONB NOT NULL DEFAULT '[]',
    agents_resting JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ui_sessions_user_id ON ui_sessions(user_id);
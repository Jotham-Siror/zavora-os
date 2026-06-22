CREATE TABLE agent_sessions (
    id VARCHAR(255) NOT NULL,
    app_name VARCHAR(255) NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    state JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (app_name, user_id, id)
);

CREATE TABLE agent_events (
    id BIGSERIAL PRIMARY KEY,
    session_app_name VARCHAR(255) NOT NULL,
    session_user_id VARCHAR(255) NOT NULL,
    session_id VARCHAR(255) NOT NULL,
    event_data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (session_app_name, session_user_id, session_id)
        REFERENCES agent_sessions(app_name, user_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_agent_events_session ON agent_events(session_app_name, session_user_id, session_id);
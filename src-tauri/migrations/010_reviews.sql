-- Review sessions: formal approval workflows for sets of requirements
CREATE TABLE IF NOT EXISTS review_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'open',  -- open | in_progress | approved | rejected | closed
    created_by TEXT NOT NULL DEFAULT 'User',
    created_at TEXT NOT NULL,
    closed_at TEXT,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_review_sessions_project ON review_sessions(project_id);

-- Requirements included in a review session
CREATE TABLE IF NOT EXISTS review_items (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    node_id TEXT NOT NULL,        -- requirement node
    verdict TEXT,                  -- NULL | approved | rejected | needs_changes
    verdict_by TEXT,
    verdict_at TEXT,
    verdict_note TEXT,
    FOREIGN KEY (session_id) REFERENCES review_sessions(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_review_items_session ON review_items(session_id);
CREATE INDEX IF NOT EXISTS idx_review_items_node ON review_items(node_id);

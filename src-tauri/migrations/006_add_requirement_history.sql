-- Migration 006: Persistent requirement history / audit trail

CREATE TABLE requirement_history (
    id            TEXT PRIMARY KEY, -- UUIDv4
    project_id    TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    node_id       TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    actor         TEXT NOT NULL DEFAULT 'system',
    change_source TEXT NOT NULL DEFAULT 'manual', -- manual | ai | import | api
    changed_at    TEXT NOT NULL, -- ISO-8601 UTC
    prev_snapshot TEXT NOT NULL, -- JSON RequirementSnapshot
    next_snapshot TEXT NOT NULL  -- JSON RequirementSnapshot
);

CREATE INDEX idx_req_history_project ON requirement_history(project_id, changed_at DESC);
CREATE INDEX idx_req_history_node ON requirement_history(node_id, changed_at DESC);

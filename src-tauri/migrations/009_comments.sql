CREATE TABLE IF NOT EXISTS req_comments (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    node_id TEXT NOT NULL,          -- The requirement node this comment is on
    parent_id TEXT,                 -- NULL = top-level, non-NULL = reply
    author TEXT NOT NULL DEFAULT 'User',
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    resolved_at TEXT,               -- NULL = open, non-NULL = resolved
    resolved_by TEXT,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX idx_req_comments_node ON req_comments(node_id);
CREATE INDEX idx_req_comments_project ON req_comments(project_id);
CREATE INDEX idx_req_comments_parent ON req_comments(parent_id);

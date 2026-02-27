-- Tracks which requirement links are "suspect" (source changed after link was created)
CREATE TABLE IF NOT EXISTS suspect_links (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    edge_id TEXT NOT NULL,           -- The edge that is suspect
    source_node_id TEXT NOT NULL,    -- The requirement that changed
    target_node_id TEXT NOT NULL,    -- The downstream req that needs review
    flagged_at TEXT NOT NULL,        -- ISO-8601 when flagged
    flagged_reason TEXT NOT NULL,    -- Human-readable: what changed
    resolved_at TEXT,                -- NULL = still suspect
    resolved_by TEXT,                -- actor string who resolved
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX idx_suspect_links_project ON suspect_links(project_id);
CREATE INDEX idx_suspect_links_edge ON suspect_links(edge_id);
CREATE INDEX idx_suspect_links_resolved ON suspect_links(project_id, resolved_at);

-- Model baselines (named snapshots of the full project model state)
CREATE TABLE IF NOT EXISTS model_baselines (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_by  TEXT NOT NULL DEFAULT 'User',
    created_at  TEXT NOT NULL,
    -- full JSON snapshot: { nodes: [...], edges: [...] }
    snapshot    TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_model_baselines_project ON model_baselines(project_id, created_at DESC);

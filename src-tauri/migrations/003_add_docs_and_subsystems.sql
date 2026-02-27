-- Migration 003: Documents, subsystem knowledge, artifacts, activity

-- ============================================================
-- DOCUMENTS (per-project, stored extracted text)
-- ============================================================

CREATE TABLE documents (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    doc_type    TEXT NOT NULL DEFAULT '',
    size        INTEGER NOT NULL DEFAULT 0,
    added_at    TEXT NOT NULL,
    text        TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_documents_project ON documents(project_id);
CREATE INDEX idx_documents_added   ON documents(added_at);

-- ============================================================
-- SUBSYSTEM KNOWLEDGE PAGES
-- ============================================================

CREATE TABLE subsystem_knowledge (
    id           TEXT PRIMARY KEY,
    subsystem_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    body         TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

CREATE INDEX idx_subsystem_knowledge_subsystem ON subsystem_knowledge(subsystem_id);

-- ============================================================
-- SUBSYSTEM ARTIFACTS
-- ============================================================

CREATE TABLE subsystem_artifacts (
    id           TEXT PRIMARY KEY,
    subsystem_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    kind         TEXT NOT NULL,
    title        TEXT NOT NULL,
    link         TEXT NOT NULL DEFAULT '',
    notes        TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_subsystem_artifacts_subsystem ON subsystem_artifacts(subsystem_id);

-- ============================================================
-- SUBSYSTEM ACTIVITY
-- ============================================================

CREATE TABLE subsystem_activity (
    id           TEXT PRIMARY KEY,
    subsystem_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    text         TEXT NOT NULL,
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_subsystem_activity_subsystem ON subsystem_activity(subsystem_id);

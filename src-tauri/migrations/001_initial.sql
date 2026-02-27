-- Migration 001: Initial schema
-- SystemProduct SE tool — core graph model
-- Note: WAL mode and foreign_keys are set via SqliteConnectOptions, not here.

-- ============================================================
-- PROJECTS
-- ============================================================

CREATE TABLE projects (
    id          TEXT PRIMARY KEY,           -- UUIDv4
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,              -- ISO-8601 UTC
    modified_at TEXT NOT NULL
);

-- ============================================================
-- NODES
-- Known fields are typed columns. Truly dynamic metadata
-- goes into `meta` (JSON). Never put queryable data in `meta`.
-- ============================================================

CREATE TABLE nodes (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,              -- NodeKind enum value
    name        TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',

    -- Requirement-specific (NULL on non-requirement nodes)
    req_id      TEXT,                       -- human-readable ID, e.g. "REQ-001"
    req_text    TEXT,
    req_rationale TEXT,
    req_priority  TEXT,                     -- "shall" | "should" | "may"
    req_status    TEXT,                     -- "draft" | "approved" | "obsolete"
    req_source    TEXT,                     -- origin (stakeholder, standard, etc.)
    req_verification_method TEXT,           -- "analysis" | "test" | "inspection" | "demonstration"

    -- Block-specific
    block_is_abstract INTEGER,              -- 0/1 boolean
    block_multiplicity TEXT,

    -- Port-specific
    port_direction TEXT,                    -- "in" | "out" | "inout"
    port_type_ref  TEXT,                    -- UUID of the type block (soft ref)

    -- Use case / actor
    uc_level TEXT,                          -- "summary" | "user" | "subfunction"

    -- Test case
    tc_procedure TEXT,
    tc_expected  TEXT,
    tc_status    TEXT,                      -- "not_run" | "pass" | "fail"

    -- Overflow: arbitrary extra properties set by user or AI
    meta        TEXT NOT NULL DEFAULT '{}', -- JSON object

    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

CREATE INDEX idx_nodes_project  ON nodes(project_id);
CREATE INDEX idx_nodes_kind     ON nodes(kind);
CREATE INDEX idx_nodes_req_id   ON nodes(req_id) WHERE req_id IS NOT NULL;

-- ============================================================
-- EDGES
-- Relationships are first-class. Both endpoints must exist
-- in the same project (enforced by application layer — SQLite
-- doesn't support multi-column FK cross-checks cleanly).
-- ============================================================

CREATE TABLE edges (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,              -- EdgeKind enum value
    source_id   TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_id   TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    label       TEXT NOT NULL DEFAULT '',
    meta        TEXT NOT NULL DEFAULT '{}', -- JSON

    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

CREATE INDEX idx_edges_project   ON edges(project_id);
CREATE INDEX idx_edges_source    ON edges(source_id);
CREATE INDEX idx_edges_target    ON edges(target_id);
CREATE INDEX idx_edges_kind      ON edges(kind);

-- Composite index for "all edges of a given kind touching a node"
CREATE INDEX idx_edges_kind_source ON edges(kind, source_id);
CREATE INDEX idx_edges_kind_target ON edges(kind, target_id);

-- ============================================================
-- DIAGRAMS
-- A diagram is a named view over the graph. It belongs to a
-- project and has a type (BDD, IBD, UseCase, etc.).
-- ============================================================

CREATE TABLE diagrams (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,              -- "bdd" | "ibd" | "usecase" | "sequence" | "statemachine"
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    -- ELK layout options stored per-diagram
    layout_options TEXT NOT NULL DEFAULT '{}', -- JSON
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);

CREATE INDEX idx_diagrams_project ON diagrams(project_id);

-- ============================================================
-- DIAGRAM ELEMENTS
-- Positional / visual data for nodes as they appear in a
-- specific diagram. Same node can appear in many diagrams
-- at different positions. This is the ONLY place x/y lives.
-- ============================================================

CREATE TABLE diagram_elements (
    id          TEXT PRIMARY KEY,
    diagram_id  TEXT NOT NULL REFERENCES diagrams(id) ON DELETE CASCADE,
    node_id     TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,

    -- Canvas position and size (pixels in logical canvas space)
    x           REAL NOT NULL DEFAULT 0,
    y           REAL NOT NULL DEFAULT 0,
    width       REAL NOT NULL DEFAULT 120,
    height      REAL NOT NULL DEFAULT 60,

    -- Per-appearance overrides (e.g. collapsed/expanded in this diagram)
    collapsed   INTEGER NOT NULL DEFAULT 0,
    style_overrides TEXT NOT NULL DEFAULT '{}', -- JSON

    UNIQUE(diagram_id, node_id)
);

CREATE INDEX idx_de_diagram ON diagram_elements(diagram_id);
CREATE INDEX idx_de_node    ON diagram_elements(node_id);

-- ============================================================
-- DIAGRAM EDGE ROUTES
-- Optional manual waypoints for an edge in a specific diagram.
-- If absent, the layout engine or auto-router determines the path.
-- ============================================================

CREATE TABLE diagram_edge_routes (
    id          TEXT PRIMARY KEY,
    diagram_id  TEXT NOT NULL REFERENCES diagrams(id) ON DELETE CASCADE,
    edge_id     TEXT NOT NULL REFERENCES edges(id) ON DELETE CASCADE,
    -- Ordered JSON array of {x, y} waypoints
    waypoints   TEXT NOT NULL DEFAULT '[]',

    UNIQUE(diagram_id, edge_id)
);

CREATE INDEX idx_der_diagram ON diagram_edge_routes(diagram_id);

-- ============================================================
-- AI SUGGESTIONS
-- Pending suggestions that have not been accepted or dismissed.
-- Accepted suggestions write into nodes/edges then delete the row.
-- Dismissed suggestions delete the row.
-- Ghost elements on the canvas query this table.
-- ============================================================

CREATE TABLE ai_suggestions (
    id           TEXT PRIMARY KEY,
    project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    diagram_id   TEXT REFERENCES diagrams(id) ON DELETE CASCADE, -- NULL = not diagram-scoped
    kind         TEXT NOT NULL,             -- "node" | "edge" | "text" | "analysis"
    -- For node/edge suggestions: proposed data as JSON matching node/edge shape
    payload      TEXT NOT NULL,             -- JSON
    -- Human-readable explanation surfaced in the UI
    rationale    TEXT NOT NULL DEFAULT '',
    -- Analysis suggestions have a severity
    severity     TEXT,                      -- "info" | "warning" | "error"
    -- Which existing node/field this attaches to (for inline text suggestions)
    target_node_id  TEXT REFERENCES nodes(id) ON DELETE CASCADE,
    target_field    TEXT,                   -- e.g. "req_text"
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_ai_project  ON ai_suggestions(project_id);
CREATE INDEX idx_ai_diagram  ON ai_suggestions(diagram_id) WHERE diagram_id IS NOT NULL;
CREATE INDEX idx_ai_target   ON ai_suggestions(target_node_id) WHERE target_node_id IS NOT NULL;

-- ============================================================
-- SETTINGS
-- Per-project and global key/value config (AI provider,
-- theme overrides, export paths, etc.)
-- ============================================================

CREATE TABLE settings (
    key         TEXT NOT NULL,
    project_id  TEXT REFERENCES projects(id) ON DELETE CASCADE, -- NULL = global
    value       TEXT NOT NULL               -- JSON scalar or object
);

CREATE UNIQUE INDEX idx_settings_key_project
    ON settings(key, COALESCE(project_id, ''));

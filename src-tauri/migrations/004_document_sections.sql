-- Migration 004: Document sections + derives edge kind

-- ============================================================
-- DOCUMENT SECTIONS
-- Structured sub-units parsed from an ingested document.
-- Each section can be traced to requirements via edges of
-- kind "derives" (section → requirement).
-- ============================================================

CREATE TABLE document_sections (
    id          TEXT PRIMARY KEY,           -- UUIDv4
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,

    -- Human-readable section identifier (e.g. "3.1.2", "WBS 1.2", "PN-4001")
    section_ref TEXT NOT NULL DEFAULT '',

    -- Structural type drives display + parser hints
    -- Values: 'heading' | 'paragraph' | 'requirement' | 'bom_item' |
    --         'boe_line' | 'sow_section' | 'icd_interface' | 'list_item'
    section_type TEXT NOT NULL DEFAULT 'paragraph',

    -- Display title (heading text, BOM description, etc.)
    title       TEXT NOT NULL DEFAULT '',

    -- Full text content of the section
    body        TEXT NOT NULL DEFAULT '',

    -- For BOM: part number, quantity, unit
    part_number TEXT,
    quantity    TEXT,
    unit        TEXT,

    -- Ordering within document
    position    INTEGER NOT NULL DEFAULT 0,

    created_at  TEXT NOT NULL
);

CREATE INDEX idx_doc_sections_document ON document_sections(document_id);
CREATE INDEX idx_doc_sections_project  ON document_sections(project_id);
CREATE INDEX idx_doc_sections_type     ON document_sections(section_type);

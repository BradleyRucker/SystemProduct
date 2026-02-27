-- Migration 007: Add body format for subsystem knowledge pages

ALTER TABLE subsystem_knowledge
ADD COLUMN body_format TEXT NOT NULL DEFAULT 'plain';

UPDATE subsystem_knowledge
SET body_format = 'plain'
WHERE body_format IS NULL OR TRIM(body_format) = '';

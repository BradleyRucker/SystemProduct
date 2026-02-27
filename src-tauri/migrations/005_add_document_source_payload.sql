-- Migration 005: Preserve uploaded source payload for rich document rendering

ALTER TABLE documents ADD COLUMN source_base64 TEXT;
ALTER TABLE documents ADD COLUMN source_mime TEXT;

-- Migration 002: Add requirement allocations to nodes

ALTER TABLE nodes ADD COLUMN req_allocations TEXT;

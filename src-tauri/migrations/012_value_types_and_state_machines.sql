-- Migration 012: Value Types, Typed Ports, State Machines, Parametric Diagrams
-- Adds columns needed for the SysML feature expansion.

-- ── Port extensions ──────────────────────────────────────────────────────────
-- type_name: human-readable type (e.g. "Voltage", "Real")
-- port_multiplicity: SysML multiplicity string
ALTER TABLE nodes ADD COLUMN port_type_name TEXT;
ALTER TABLE nodes ADD COLUMN port_multiplicity TEXT;

-- ── ValueType-specific columns ────────────────────────────────────────────────
ALTER TABLE nodes ADD COLUMN vt_base_type TEXT;     -- "Real", "Integer", "Boolean", "String"
ALTER TABLE nodes ADD COLUMN vt_unit TEXT;           -- "m/s", "kg", "V", "A"
ALTER TABLE nodes ADD COLUMN vt_constraint TEXT;     -- constraint expression

-- ── ConstraintBlock-specific columns ──────────────────────────────────────────
ALTER TABLE nodes ADD COLUMN cb_expression TEXT;     -- math/logical expression
ALTER TABLE nodes ADD COLUMN cb_parameters TEXT;     -- JSON array of parameter names

-- ── State-specific columns ────────────────────────────────────────────────────
ALTER TABLE nodes ADD COLUMN state_pseudo_kind TEXT; -- "initial" | "final" | "choice" | etc.
ALTER TABLE nodes ADD COLUMN state_entry TEXT;       -- entry action
ALTER TABLE nodes ADD COLUMN state_exit TEXT;        -- exit action
ALTER TABLE nodes ADD COLUMN state_do TEXT;          -- do-activity

-- ── Transition edge meta columns (stored in edge meta JSON, no schema change needed) ──
-- transition guard, trigger, and effect are stored in edge.meta JSON blob.
-- No ALTER TABLE needed for edges since they use a meta JSON column.

-- ── Diagram kind extension ────────────────────────────────────────────────────
-- The diagrams.kind column already accepts any text value — "parametric" and
-- "statemachine" are already handled in application code.
-- No schema change needed here.

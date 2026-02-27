-- Migration 011: Discrete Event Simulation
-- Adds simulation parameters and script to block nodes,
-- plus simulation_scenarios and simulation_results tables.

-- Extend nodes table (safe ALTER - nullable defaults, no rebuild needed)
ALTER TABLE nodes ADD COLUMN sim_params TEXT DEFAULT NULL;
ALTER TABLE nodes ADD COLUMN sim_script TEXT DEFAULT NULL;

-- Simulation scenarios: named run configurations with stimulus events
CREATE TABLE simulation_scenarios (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 10000,
    events      TEXT NOT NULL DEFAULT '[]',  -- JSON array of { time_ms, block_id, signal_type, value }
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL
);
CREATE INDEX idx_sim_scenarios_project ON simulation_scenarios(project_id);

-- Simulation results: one row per scenario run
CREATE TABLE simulation_results (
    id          TEXT PRIMARY KEY,
    scenario_id TEXT NOT NULL REFERENCES simulation_scenarios(id) ON DELETE CASCADE,
    ran_at      TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending | running | complete | error
    metrics     TEXT NOT NULL DEFAULT '{}',        -- JSON: { block_id: { utilization, queue_depth, processed_count, failures } }
    timeline    TEXT NOT NULL DEFAULT '[]',        -- JSON: [ { timestamp_ms, block_id, event_type, detail } ]
    errors      TEXT NOT NULL DEFAULT '[]'         -- JSON: [ error_string ]
);
CREATE INDEX idx_sim_results_scenario ON simulation_results(scenario_id);

/// Tauri event names emitted from the backend to the frontend.
/// Import these constants in both Rust (emit) and TypeScript (listen).

pub const MODEL_CHANGED: &str = "model:changed";
pub const VALIDATION_UPDATED: &str = "validation:updated";
pub const AI_SUGGESTION_READY: &str = "ai:suggestion_ready";
pub const AI_ANALYSIS_READY: &str = "ai:analysis_ready";
pub const DIAGRAM_LAYOUT_READY: &str = "diagram:layout_ready";

use crate::ai::provider::{Message, Prompt, Role};
use crate::core::model::*;
use crate::core::validation;
use crate::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, State};
use uuid::Uuid;

// ── Projects ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    state.store.list_projects().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_project(
    name: String,
    description: String,
    state: State<'_, AppState>,
) -> Result<Project, String> {
    let now = Utc::now();
    let project = Project {
        id: Uuid::new_v4(),
        name,
        description,
        created_at: now,
        modified_at: now,
    };
    state
        .store
        .create_project(&project)
        .await
        .map_err(|e| e.to_string())?;
    Ok(project)
}

#[tauri::command]
pub async fn get_project(id: String, state: State<'_, AppState>) -> Result<Project, String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .get_project(uuid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())
}

#[tauri::command]
pub async fn delete_project(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_project(uuid)
        .await
        .map_err(|e| e.to_string())
}

// ── Nodes ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_nodes(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Node>, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.list_nodes(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_node(node: Node, state: State<'_, AppState>) -> Result<(), String> {
    let node_id = node.id;
    let project_id = node.project_id;
    let is_requirement = node.kind == crate::core::model::NodeKind::Requirement;
    state
        .store
        .upsert_node(&node)
        .await
        .map_err(|e| e.to_string())?;
    // Flag downstream links as suspect when a requirement changes
    if is_requirement {
        let _ = state.store.flag_suspect_links(project_id, node_id, "requirement updated").await;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_requirement_history(
    node_id: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<RequirementHistoryEntry>, String> {
    let id: Uuid = node_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let capped_limit = limit.unwrap_or(20).clamp(1, 200) as usize;
    state
        .store
        .list_requirement_history(id, capped_limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_node(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_node(uuid)
        .await
        .map_err(|e| e.to_string())
}

// ── Edges ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn upsert_edge(edge: Edge, state: State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .upsert_edge(&edge)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_edge(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_edge(uuid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn edges_for_node(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Edge>, String> {
    let uuid: Uuid = node_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .edges_for_node(uuid)
        .await
        .map_err(|e| e.to_string())
}

// ── Diagrams ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_diagrams(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Diagram>, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_diagrams(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_diagram(diagram: Diagram, state: State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .upsert_diagram(&diagram)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn diagram_elements(
    diagram_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<DiagramElement>, String> {
    let id: Uuid = diagram_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .diagram_elements(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_diagram_element(
    element: DiagramElement,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .upsert_diagram_element(&element)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_diagram(diagram_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id: Uuid = diagram_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_diagram(id)
        .await
        .map_err(|e| e.to_string())
}

// -- Documents --------------------------------------------------------------

#[tauri::command]
pub async fn list_documents(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Document>, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_documents(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_document(doc: Document, state: State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .upsert_document(&doc)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_document(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_document(uuid)
        .await
        .map_err(|e| e.to_string())
}

// -- Document sections -------------------------------------------------------

#[tauri::command]
pub async fn list_document_sections(
    document_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<DocumentSection>, String> {
    let id: Uuid = document_id
        .parse()
        .map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_document_sections(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_project_document_sections(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<DocumentSection>, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_project_document_sections(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_document_section(
    section: DocumentSection,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .upsert_document_section(&section)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_document_section(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_document_section(uuid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_document_sections(
    document_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid: Uuid = document_id
        .parse()
        .map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_document_sections(uuid)
        .await
        .map_err(|e| e.to_string())
}

// -- Subsystem knowledge ----------------------------------------------------

#[tauri::command]
pub async fn list_subsystem_knowledge(
    subsystem_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SubsystemKnowledgePage>, String> {
    let id: Uuid = subsystem_id
        .parse()
        .map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_subsystem_knowledge(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_subsystem_knowledge(
    page: SubsystemKnowledgePage,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .upsert_subsystem_knowledge(&page)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_subsystem_knowledge(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_subsystem_knowledge(uuid)
        .await
        .map_err(|e| e.to_string())
}

// -- Subsystem artifacts ----------------------------------------------------

#[tauri::command]
pub async fn list_subsystem_artifacts(
    subsystem_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SubsystemArtifact>, String> {
    let id: Uuid = subsystem_id
        .parse()
        .map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_subsystem_artifacts(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_project_artifacts(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SubsystemArtifact>, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_project_artifacts(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn upsert_subsystem_artifact(
    artifact: SubsystemArtifact,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .upsert_subsystem_artifact(&artifact)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_subsystem_artifact(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .delete_subsystem_artifact(uuid)
        .await
        .map_err(|e| e.to_string())
}

// -- Subsystem activity -----------------------------------------------------

#[tauri::command]
pub async fn list_subsystem_activity(
    subsystem_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SubsystemActivity>, String> {
    let id: Uuid = subsystem_id
        .parse()
        .map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_subsystem_activity(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_subsystem_activity(
    entry: SubsystemActivity,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .add_subsystem_activity(&entry)
        .await
        .map_err(|e| e.to_string())
}

// -- Settings ---------------------------------------------------------------

#[tauri::command]
pub async fn get_setting(
    key: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let pid = match project_id {
        Some(id) => Some(id.parse().map_err(|e: uuid::Error| e.to_string())?),
        None => None,
    };
    state
        .store
        .get_setting(&key, pid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pid = match project_id {
        Some(id) => Some(id.parse().map_err(|e: uuid::Error| e.to_string())?),
        None => None,
    };
    state
        .store
        .set_setting(&key, pid, &value)
        .await
        .map_err(|e| e.to_string())
}

// ── Validation ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn validate_model(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<validation::ValidationIssue>, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let nodes = state
        .store
        .list_nodes(id)
        .await
        .map_err(|e| e.to_string())?;
    let edges = {
        let mut all = Vec::new();
        for node in &nodes {
            let mut e = state
                .store
                .edges_for_node(node.id)
                .await
                .map_err(|e| e.to_string())?;
            all.append(&mut e);
        }
        all.sort_by_key(|e| e.id);
        all.dedup_by_key(|e| e.id);
        all
    };
    Ok(validation::validate(&nodes, &edges))
}

// ── Export ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn export_markdown(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let project = state
        .store
        .get_project(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let nodes = state
        .store
        .list_nodes(id)
        .await
        .map_err(|e| e.to_string())?;
    let edges = {
        let mut all = Vec::new();
        for node in &nodes {
            let mut e = state
                .store
                .edges_for_node(node.id)
                .await
                .map_err(|e| e.to_string())?;
            all.append(&mut e);
        }
        all.sort_by_key(|e| e.id);
        all.dedup_by_key(|e| e.id);
        all
    };
    Ok(crate::core::export::to_markdown(&project, &nodes, &edges))
}

#[tauri::command]
pub async fn export_json(project_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let project = state
        .store
        .get_project(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let nodes = state
        .store
        .list_nodes(id)
        .await
        .map_err(|e| e.to_string())?;
    let edges = {
        let mut all = Vec::new();
        for node in &nodes {
            let mut e = state
                .store
                .edges_for_node(node.id)
                .await
                .map_err(|e| e.to_string())?;
            all.append(&mut e);
        }
        all.sort_by_key(|e| e.id);
        all.dedup_by_key(|e| e.id);
        all
    };
    crate::core::export::to_native_json(&project, &nodes, &edges).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_xmi(project_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let id: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let project = state
        .store
        .get_project(id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;
    let nodes = state
        .store
        .list_nodes(id)
        .await
        .map_err(|e| e.to_string())?;
    let edges = {
        let mut all = Vec::new();
        for node in &nodes {
            let mut e = state
                .store
                .edges_for_node(node.id)
                .await
                .map_err(|e| e.to_string())?;
            all.append(&mut e);
        }
        all.sort_by_key(|e| e.id);
        all.dedup_by_key(|e| e.id);
        all
    };
    Ok(crate::core::export::to_xmi(&project, &nodes, &edges))
}

// ── AI availability ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ai_available(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.ai_provider.lock().unwrap().is_available())
}

#[tauri::command]
pub async fn ai_provider_name(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.ai_provider.lock().unwrap().name().to_string())
}

#[tauri::command]
pub async fn ollama_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    use crate::ai::ollama::OllamaProvider;

    let base_url = state
        .store
        .get_setting("ai.ollama.base_url", None)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    let model = state
        .store
        .get_setting("ai.ollama.model", None)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "qwen2.5:7b".to_string());

    let active_provider = state
        .store
        .get_setting("ai.provider", None)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

    let probe = OllamaProvider::new(&model, Some(base_url.clone()));
    let reachable = probe.check_available().await;

    Ok(serde_json::json!({
        "reachable": reachable,
        "base_url": base_url,
        "model": model,
        "is_active": active_provider == "ollama",
    }))
}

#[tauri::command]
pub async fn set_ollama_config(
    model: String,
    base_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use crate::ai::ollama::OllamaProvider;

    let url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());

    state
        .store
        .set_setting("ai.ollama.model", None, &model)
        .await
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("ai.ollama.base_url", None, &url)
        .await
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("ai.provider", None, "ollama")
        .await
        .map_err(|e| e.to_string())?;

    let new_provider: Arc<dyn crate::ai::provider::AIProvider> =
        Arc::new(OllamaProvider::new(model, Some(url)));
    *state.ai_provider.lock().unwrap() = new_provider;
    Ok(())
}

#[tauri::command]
pub async fn set_anthropic_key(key: String, state: State<'_, AppState>) -> Result<(), String> {
    use crate::ai::anthropic::AnthropicProvider;
    use crate::ai::provider::NullProvider;

    state
        .store
        .set_setting("ai.anthropic.api_key", None, &key)
        .await
        .map_err(|e| e.to_string())?;
    if !key.is_empty() {
        state
            .store
            .set_setting("ai.provider", None, "anthropic")
            .await
            .map_err(|e| e.to_string())?;
    }

    let new_provider: Arc<dyn crate::ai::provider::AIProvider> = if !key.is_empty() {
        Arc::new(AnthropicProvider::new(key))
    } else {
        Arc::new(NullProvider)
    };
    *state.ai_provider.lock().unwrap() = new_provider;
    Ok(())
}

// ── Requirement parser ────────────────────────────────────────────────────────

/// Send sentences to req_parser.py via the system Python interpreter.
/// Tries Miniconda first, then falls back to "python" / "python3" on PATH.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementParseBlock {
    pub text: String,
    #[serde(default)]
    pub section_title: String,
    #[serde(default)]
    pub section_ref: String,
    #[serde(default)]
    pub section_type: String,
    #[serde(default)]
    pub line_index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementQualityInput {
    #[serde(default)]
    pub id: String,
    pub sentence: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub confidence: String,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub classification: String,
    #[serde(default)]
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementQualityOutput {
    #[serde(default)]
    pub id: String,
    pub sentence: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub confidence: String,
    #[serde(default)]
    pub classification: String,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub review_priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementAllocationInput {
    #[serde(default)]
    pub id: String,
    pub sentence: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub confidence: String,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub classification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationSubsystemInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementAllocationOutput {
    #[serde(default)]
    pub id: String,
    pub sentence: String,
    pub allocation: String,
    #[serde(default)]
    pub confidence: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub new_subsystem_name: String,
}

#[tauri::command]
pub async fn parse_requirements(
    sentences: Option<Vec<String>>,
    blocks: Option<Vec<RequirementParseBlock>>,
    doc_type: Option<String>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let payload = if let Some(blocks) = blocks {
        serde_json::json!({
            "blocks": blocks,
            "sentences": sentences.unwrap_or_default(),
            "doc_type": doc_type.unwrap_or_default(),
        })
    } else {
        serde_json::json!({
            "sentences": sentences.unwrap_or_default(),
            "doc_type": doc_type.unwrap_or_default(),
        })
    };
    let input = serde_json::to_string(&payload).map_err(|e| e.to_string())?;

    // Locate req_parser.py — try several locations in order:
    //   1. Bundled in app resource dir (production)
    //   2. CARGO_MANIFEST_DIR-relative (dev, most reliable)
    //   3. cwd-relative fallback
    let script_path = {
        let resource_dir = app
            .path()
            .resource_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let bundled = resource_dir.join("sidecar").join("req_parser.py");

        // In dev, CARGO_MANIFEST_DIR points to src-tauri/, so go one level up
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cargo_relative = manifest_dir
            .parent()
            .map(|p| p.join("sidecar").join("req_parser.py"))
            .unwrap_or_else(|| manifest_dir.join("sidecar").join("req_parser.py"));

        let cwd_relative = std::path::PathBuf::from("sidecar/req_parser.py");

        if bundled.exists() {
            bundled
        } else if cargo_relative.exists() {
            cargo_relative
        } else if cwd_relative.exists() {
            cwd_relative
        } else {
            return Err(format!(
                "req_parser.py not found. Looked in: {}, {}, {}",
                bundled.display(),
                cargo_relative.display(),
                cwd_relative.display()
            ));
        }
    };

    // Try Python interpreters in order of preference
    let candidates = [r"C:\Users\aliso\miniconda3\python.exe", "python", "python3"];

    let mut last_err = String::from("no Python interpreter found");
    for python in &candidates {
        match run_python_script(python, &script_path, &input).await {
            Ok(out) if !out.trim().is_empty() => return Ok(out.trim().to_string()),
            Ok(_) => {
                last_err = format!("{python}: produced empty output");
            }
            Err(e) => {
                last_err = format!("{python}: {e}");
            }
        }
    }

    Err(format!("req_parser failed: {last_err}"))
}

async fn run_python_script(
    python: &str,
    script: &std::path::Path,
    input: &str,
) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new(python)
        .arg(script)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn failed: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(format!("{input}\n").as_bytes())
            .await
            .map_err(|e| e.to_string())?;
    }

    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if stdout.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(format!("empty output. stderr: {}", stderr.trim()));
    }

    Ok(stdout)
}

// ── Simulation commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn save_sim_params(
    node_id: String,
    params: Option<SimParams>,
    script: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid: Uuid = node_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let node = state
        .store
        .get_node(uuid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "node not found".to_string())?;
    let updated_data = match node.data {
        NodeData::Block(mut b) => {
            b.sim_params = params;
            b.sim_script = script;
            NodeData::Block(b)
        }
        _ => return Err("node is not a block".to_string()),
    };
    let updated = Node {
        data: updated_data,
        modified_at: Utc::now(),
        ..node
    };
    state.store.upsert_node(&updated).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_sim_params(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let uuid: Uuid = node_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let node = state
        .store
        .get_node(uuid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "node not found".to_string())?;
    match node.data {
        NodeData::Block(b) => Ok(serde_json::json!({
            "sim_params": b.sim_params,
            "sim_script": b.sim_script,
        })),
        _ => Ok(serde_json::json!({ "sim_params": null, "sim_script": null })),
    }
}

#[tauri::command]
pub async fn save_scenario(
    scenario: SimulationScenario,
    state: State<'_, AppState>,
) -> Result<SimulationScenario, String> {
    state
        .store
        .upsert_simulation_scenario(&scenario)
        .await
        .map_err(|e| e.to_string())?;
    Ok(scenario)
}

#[tauri::command]
pub async fn list_scenarios(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SimulationScenario>, String> {
    let uuid: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .list_simulation_scenarios(uuid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_simulation(
    scenario_id: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let scenario_uuid: Uuid = scenario_id.parse().map_err(|e: uuid::Error| e.to_string())?;

    let scenario = state
        .store
        .get_simulation_scenario(scenario_uuid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "scenario not found".to_string())?;

    let nodes = state
        .store
        .list_nodes(scenario.project_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut all_edges = Vec::new();
    for node in &nodes {
        let mut e = state
            .store
            .edges_for_node(node.id)
            .await
            .map_err(|e| e.to_string())?;
        all_edges.append(&mut e);
    }
    all_edges.sort_by_key(|e| e.id);
    all_edges.dedup_by_key(|e| e.id);

    // Build block_behaviors: block_id -> { sim_params, sim_script }
    let block_behaviors: serde_json::Map<String, serde_json::Value> = nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Block)
        .filter_map(|n| {
            if let NodeData::Block(ref b) = n.data {
                if b.sim_params.is_some() || b.sim_script.is_some() {
                    Some((
                        n.id.to_string(),
                        serde_json::json!({
                            "sim_params": b.sim_params,
                            "sim_script": b.sim_script,
                        }),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    let project = state
        .store
        .get_project(scenario.project_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "project not found".to_string())?;

    let project_json_str = crate::core::export::to_native_json(&project, &nodes, &all_edges)
        .map_err(|e| e.to_string())?;
    let project_json: serde_json::Value =
        serde_json::from_str(&project_json_str).unwrap_or_default();

    let input_payload = serde_json::json!({
        "project_json": project_json,
        "scenario": {
            "id": scenario.id,
            "name": scenario.name,
            "duration_ms": scenario.duration_ms,
            "events": scenario.events,
        },
        "block_behaviors": block_behaviors,
    });
    let input = serde_json::to_string(&input_payload).map_err(|e| e.to_string())?;

    // Create a pending result row
    let result_id = Uuid::new_v4();
    let pending_result = SimulationResult {
        id: result_id,
        scenario_id: scenario_uuid,
        ran_at: Utc::now(),
        status: "running".to_string(),
        metrics: serde_json::Value::Object(Default::default()),
        timeline: serde_json::Value::Array(vec![]),
        errors: serde_json::Value::Array(vec![]),
    };
    state
        .store
        .insert_simulation_result(&pending_result)
        .await
        .map_err(|e| e.to_string())?;

    // Resolve simulation_engine.py (same 3-path strategy as req_parser.py)
    let script_path = {
        let resource_dir = app
            .path()
            .resource_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let bundled = resource_dir.join("sidecar").join("simulation_engine.py");
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let cargo_relative = manifest_dir
            .parent()
            .map(|p| p.join("sidecar").join("simulation_engine.py"))
            .unwrap_or_else(|| manifest_dir.join("sidecar").join("simulation_engine.py"));
        let cwd_relative = std::path::PathBuf::from("sidecar/simulation_engine.py");

        if bundled.exists() {
            bundled
        } else if cargo_relative.exists() {
            cargo_relative
        } else if cwd_relative.exists() {
            cwd_relative
        } else {
            state
                .store
                .update_simulation_result_status(
                    result_id,
                    "error",
                    serde_json::Value::Object(Default::default()),
                    serde_json::Value::Array(vec![]),
                    serde_json::json!(["simulation_engine.py not found"]),
                )
                .await
                .ok();
            return Ok(result_id.to_string());
        }
    };

    let candidates = [r"C:\Users\aliso\miniconda3\python.exe", "python", "python3"];
    let mut last_err = String::from("no Python interpreter found");
    let mut engine_output: Option<String> = None;

    for python in &candidates {
        match run_python_script(python, &script_path, &input).await {
            Ok(out) if !out.trim().is_empty() => {
                engine_output = Some(out.trim().to_string());
                break;
            }
            Ok(_) => {
                last_err = format!("{python}: produced empty output");
            }
            Err(e) => {
                last_err = format!("{python}: {e}");
            }
        }
    }

    match engine_output {
        Some(out) => match serde_json::from_str::<serde_json::Value>(&out) {
            Ok(parsed) => {
                let status =
                    if parsed.get("status").and_then(|v| v.as_str()) == Some("error") {
                        "error"
                    } else {
                        "complete"
                    };
                state
                    .store
                    .update_simulation_result_status(
                        result_id,
                        status,
                        parsed.get("metrics").cloned().unwrap_or_default(),
                        parsed.get("timeline").cloned().unwrap_or_default(),
                        parsed.get("errors").cloned().unwrap_or_default(),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Err(e) => {
                state
                    .store
                    .update_simulation_result_status(
                        result_id,
                        "error",
                        serde_json::Value::Object(Default::default()),
                        serde_json::Value::Array(vec![]),
                        serde_json::json!([format!("JSON parse error: {e}"), out]),
                    )
                    .await
                    .ok();
            }
        },
        None => {
            state
                .store
                .update_simulation_result_status(
                    result_id,
                    "error",
                    serde_json::Value::Object(Default::default()),
                    serde_json::Value::Array(vec![]),
                    serde_json::json!([last_err]),
                )
                .await
                .ok();
        }
    }

    Ok(result_id.to_string())
}

#[tauri::command]
pub async fn get_simulation_result(
    result_id: String,
    state: State<'_, AppState>,
) -> Result<SimulationResult, String> {
    let uuid: Uuid = result_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .get_simulation_result(uuid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "result not found".to_string())
}

// -- Local LLM (llama.cpp) ---------------------------------------------------

fn resolve_llama_paths(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."));

    let bin = resource_dir.join("llama").join("llama-cli.exe");
    let model = resource_dir
        .join("models")
        .join("Qwen2.5-0.5B-Instruct-Q4_K_M.gguf");

    if bin.exists() && model.exists() {
        return Ok((bin, model));
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_root = manifest_dir.parent().unwrap_or(&manifest_dir);
    let dev_bin = dev_root
        .join("src-tauri")
        .join("resources")
        .join("llama")
        .join("llama-cli.exe");
    let dev_model = dev_root
        .join("src-tauri")
        .join("resources")
        .join("models")
        .join("Qwen2.5-0.5B-Instruct-Q4_K_M.gguf");

    if dev_bin.exists() && dev_model.exists() {
        return Ok((dev_bin, dev_model));
    }

    Err("llama-cli or model not found in resources".to_string())
}

#[tauri::command]
pub async fn local_llm_available(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(resolve_llama_paths(&app).is_ok())
}

fn extract_json_array(raw: &str) -> Option<String> {
    let mut start: Option<usize> = None;
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;

    for (i, ch) in raw.char_indices() {
        if start.is_none() {
            if ch == '[' {
                start = Some(i);
                depth = 1;
            }
            continue;
        }

        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    let start_idx = start?;
                    return Some(raw[start_idx..=i].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

fn extract_json_object(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let cleaned = if trimmed.starts_with("```") {
        trimmed
            .lines()
            .skip(1)
            .take_while(|line| !line.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        trimmed.to_string()
    };

    if serde_json::from_str::<serde_json::Value>(&cleaned).is_ok() {
        return Some(cleaned);
    }

    let start = cleaned.find('{')?;
    let end = cleaned.rfind('}')?;
    if end > start {
        Some(cleaned[start..=end].to_string())
    } else {
        None
    }
}

fn requirement_needs_quality_review(item: &RequirementQualityInput) -> bool {
    let confidence = item.confidence.trim().to_lowercase();
    let classification = item.classification.trim().to_lowercase();
    let name_words = item.name.split_whitespace().count();
    let sentence_len = item.sentence.chars().count();

    if confidence != "high" {
        return true;
    }
    if classification.is_empty() || classification == "unknown" {
        return true;
    }
    if name_words < 3 {
        return true;
    }
    if sentence_len > 260 {
        return true;
    }
    item.flags.iter().any(|f| {
        let f = f.to_lowercase();
        f.contains("compound")
            || f.contains("hedge")
            || f.contains("ambig")
            || f.contains("implicit_constraint")
    })
}

#[tauri::command]
pub async fn llm_extract_requirements(
    text: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::io::AsyncReadExt;
    use tokio::process::Command;
    use tokio::time::timeout;

    let (bin, model) = resolve_llama_paths(&app)?;

    let trimmed = if text.len() > 8000 {
        format!("{}...", &text[..8000])
    } else {
        text
    };

    let prompt = format!(
        "You are a requirements extraction engine. Extract requirement statements from the document.\n\
Return ONLY a JSON array. Each item: {{\"sentence\":\"<verbatim text>\",\"name\":\"<specific 3-7 word name derived from the requirement subject and constraint — never generic like System Requirement or Data Requirement>\",\"confidence\":\"high|medium|low\",\"flags\":[\"...\"]}}.\n\
Name rules: derive from the actual subject + measurement/constraint in each sentence. Example good names: \"RF Link Margin 6 dB Min\", \"Boot Time Under 10s\", \"AES-256 Data Encryption\".\n\
Keep sentences verbatim; use the source wording exactly.\n\
Document:\n---\n{}\n---\nJSON:",
        trimmed
    );

    let mut child = Command::new(bin)
        .arg("-m")
        .arg(model)
        .arg("--temp")
        .arg("0.2")
        .arg("--top-p")
        .arg("0.9")
        .arg("--repeat-penalty")
        .arg("1.1")
        .arg("--n-predict")
        .arg("384")
        .arg("--single-turn")
        .arg("--no-conversation")
        .arg("--simple-io")
        .arg("--no-display-prompt")
        .arg("--color")
        .arg("off")
        .arg("--log-colors")
        .arg("off")
        .arg("--prompt")
        .arg(prompt)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let status = match timeout(Duration::from_secs(90), child.wait()).await {
        Ok(res) => res.map_err(|e| e.to_string())?,
        Err(_) => {
            let _ = child.start_kill();
            return Err("LLM timed out after 90s".to_string());
        }
    };

    let mut stdout = String::new();
    if let Some(mut out) = child.stdout.take() {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf).await;
        stdout = String::from_utf8_lossy(&buf).to_string();
    }

    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        let mut buf = Vec::new();
        let _ = err.read_to_end(&mut buf).await;
        stderr = String::from_utf8_lossy(&buf).to_string();
    }

    if let Some(json) = extract_json_array(&stdout) {
        return Ok(json);
    }

    let _ = status;
    Err(format!(
        "LLM output did not contain JSON. stdout: {}, stderr: {}",
        stdout.chars().take(200).collect::<String>(),
        stderr.chars().take(200).collect::<String>()
    ))
}

#[tauri::command]
pub async fn ai_quality_pass_requirements(
    requirements: Vec<RequirementQualityInput>,
    doc_type: Option<String>,
    doc_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let provider = state.ai_provider.lock().unwrap().clone();
    if !provider.is_available() {
        return Err("no_api_key".to_string());
    }

    if requirements.is_empty() {
        return Ok(serde_json::json!({ "results": [] }).to_string());
    }

    let mut candidates: Vec<RequirementQualityInput> = requirements
        .iter()
        .filter(|item| requirement_needs_quality_review(item))
        .cloned()
        .collect();
    if candidates.is_empty() {
        candidates = requirements.iter().take(20).cloned().collect();
    } else {
        candidates.truncate(40);
    }

    let dtype = doc_type.unwrap_or_else(|| "General".to_string());
    let dname = doc_name.unwrap_or_else(|| "document".to_string());
    let payload = serde_json::to_string_pretty(&candidates).map_err(|e| e.to_string())?;

    let prompt = Prompt {
        system: Some(
            "You are a systems engineering requirement quality reviewer applying IEEE 29148.\n\
Do NOT rewrite or paraphrase the requirement sentence — only improve the short name field.\n\
\n\
NAME RULES (most important):\n\
- The name must uniquely identify WHAT the requirement is about — never use generic filler.\n\
- Derive the name from the actual subject + constraint/action in the sentence.\n\
- Format: \"<Subject> <Constraint/Property/Action>\" in Title Case, 3-7 words.\n\
- Bad names (reject these patterns): \"System Requirement\", \"Performance Requirement\", \"Data Requirement\", \"Interface Requirement\", \"Security Requirement\", \"High Requirement\", \"Network Requirement\", or any name that could apply to dozens of requirements.\n\
- Good examples: \"Uplink Data Rate 100 Mbps\", \"Battery Reserve 72 Hour Minimum\", \"GPS Fix Acquisition Under 30s\", \"AES-256 Payload Encryption\", \"Operator Alert Latency Under 2s\".\n\
- If the current name is already specific and accurate, keep it unchanged.\n\
\n\
QUALITY FLAGS (choose all that apply): ambiguous, compound_shall, missing_measurement, missing_verification_method, hedge_word, passive_voice, implicit_subject, testable, performance, interface, safety, security.\n\
\n\
CLASSIFICATION: system | contractual | verification | interface | constraint | unknown.\n\
\n\
Return ONLY this JSON object — no markdown, no explanation:\n\
{\"results\":[{\"id\":\"...\",\"sentence\":\"...\",\"name\":\"<specific descriptive name>\",\
\"confidence\":\"high|medium|low\",\"classification\":\"system|contractual|verification|interface|constraint|unknown\",\
\"flags\":[\"...\"],\"review_priority\":\"high|medium|low\"}]}"
                .to_string(),
        ),
        messages: vec![Message {
            role: Role::User,
            content: format!(
                "Document: \"{dname}\" (type: {dtype})\n\
Review these extracted requirements. For each, produce a specific descriptive name derived \
from the actual subject and constraint in that requirement sentence. Return the JSON object exactly.\n\n\
{payload}"
            ),
        }],
        max_tokens: Some(2048),
    };

    let response = provider.complete(prompt).await.map_err(|e| e.to_string())?;
    let raw = response.content.trim().to_string();
    let raw_json = extract_json_object(&raw).ok_or_else(|| {
        format!(
            "AI quality pass did not return JSON object. output: {}",
            raw.chars().take(220).collect::<String>()
        )
    })?;

    let parsed: serde_json::Value =
        serde_json::from_str(&raw_json).map_err(|e| format!("Invalid JSON: {e}"))?;
    let mut out: Vec<RequirementQualityOutput> = Vec::new();

    if let Some(items) = parsed["results"].as_array() {
        for item in items {
            let sentence = item["sentence"].as_str().unwrap_or("").trim().to_string();
            if sentence.is_empty() {
                continue;
            }
            let confidence = match item["confidence"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "high" | "medium" | "low" => item["confidence"].as_str().unwrap_or("").to_string(),
                _ => "medium".to_string(),
            };
            let classification = match item["classification"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "system" | "contractual" | "verification" | "interface" | "constraint"
                | "unknown" => item["classification"].as_str().unwrap_or("").to_string(),
                _ => "unknown".to_string(),
            };
            let review_priority = match item["review_priority"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "high" | "medium" | "low" => {
                    item["review_priority"].as_str().unwrap_or("").to_string()
                }
                _ => "medium".to_string(),
            };
            let flags = item["flags"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                        .filter(|s| !s.is_empty())
                        .take(12)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            out.push(RequirementQualityOutput {
                id: item["id"].as_str().unwrap_or("").trim().to_string(),
                sentence,
                name: item["name"].as_str().unwrap_or("").trim().to_string(),
                confidence,
                classification,
                flags,
                review_priority,
            });
        }
    }

    let output = serde_json::json!({ "results": out });
    Ok(output.to_string())
}

#[tauri::command]
pub async fn ai_suggest_requirement_allocations(
    requirements: Vec<RequirementAllocationInput>,
    subsystems: Vec<AllocationSubsystemInput>,
    doc_type: Option<String>,
    doc_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let provider = state.ai_provider.lock().unwrap().clone();
    if !provider.is_available() {
        return Err("no_api_key".to_string());
    }

    if requirements.is_empty() {
        return Ok(serde_json::json!({ "results": [] }).to_string());
    }

    let dtype = doc_type.unwrap_or_else(|| "General".to_string());
    let dname = doc_name.unwrap_or_else(|| "document".to_string());

    let mut candidates = requirements;
    candidates.truncate(120);

    let mut subsystem_list = subsystems;
    subsystem_list.truncate(40);

    let payload = serde_json::to_string_pretty(&candidates).map_err(|e| e.to_string())?;
    let subsystem_payload =
        serde_json::to_string_pretty(&subsystem_list).map_err(|e| e.to_string())?;

    let prompt = Prompt {
        system: Some(
            "You are a systems engineer allocating requirements to physical or domain subsystems \
in a Model-Based Systems Engineering (MBSE) architecture.\n\
\n\
SUBSYSTEM DEFINITION — CRITICAL:\n\
Subsystems are physical hardware units, major domain components, or top-level engineering \
disciplines. They are NOT software functions, features, or use-cases.\n\
\n\
Good subsystem examples (physical/domain level):\n\
  FPGA, Microprocessor, Microcontroller, Power Distribution, Onboard Computer,\n\
  Communication Module, RF Subsystem, GPS Receiver, Inertial Measurement Unit,\n\
  Sensor Array, Propulsion System, Thermal Management, Battery Pack,\n\
  Flight Controller, Motor Driver, Payload Interface, Data Storage,\n\
  Ground Control Station, User Interface Terminal, Network Switch,\n\
  Hydraulic Actuator, Structural Frame, Navigation System.\n\
\n\
Bad subsystem examples (these are software functions — NEVER suggest these):\n\
  display_search_results, lock_account, notify_emergency, user_authentication,\n\
  error_handling, login_module, alert_driver, payment_processing.\n\
\n\
ALLOCATION RULES:\n\
1. Choose ONE allocation from the provided subsystem list, OR 'System Level'.\n\
2. Use 'System Level' for cross-cutting, contractual, or project-wide requirements.\n\
3. If no listed subsystem fits but the requirement is clearly subsystem-specific,\n\
   keep allocation as 'System Level' AND set new_subsystem_name to a concise \n\
   physical/domain subsystem name (e.g. 'Flight Controller', 'Power Distribution Unit').\n\
4. NEVER set new_subsystem_name to a software function or feature name.\n\
\n\
Return ONLY a JSON object:\n\
{\"results\":[{\"id\":\"...\",\"sentence\":\"...\",\"allocation\":\"System Level|<exact subsystem name>\",\
\"confidence\":\"high|medium|low\",\"rationale\":\"...\",\"new_subsystem_name\":\"optional\"}]}"
                .to_string(),
        ),
        messages: vec![Message {
            role: Role::User,
            content: format!(
                "Document: \"{dname}\" (type: {dtype})\n\
Subsystems (use exact names when allocating):\n{subsystem_payload}\n\n\
Requirements to allocate:\n{payload}"
            ),
        }],
        max_tokens: Some(3072),
    };

    let response = provider.complete(prompt).await.map_err(|e| e.to_string())?;
    let raw = response.content.trim().to_string();
    let raw_json = extract_json_object(&raw).ok_or_else(|| {
        format!(
            "AI allocation pass did not return JSON object. output: {}",
            raw.chars().take(220).collect::<String>()
        )
    })?;

    let parsed: serde_json::Value =
        serde_json::from_str(&raw_json).map_err(|e| format!("Invalid JSON: {e}"))?;

    let subsystem_lookup = subsystem_list
        .iter()
        .map(|s| (s.name.trim().to_lowercase(), s.name.trim().to_string()))
        .collect::<std::collections::HashMap<_, _>>();

    let mut out: Vec<RequirementAllocationOutput> = Vec::new();
    if let Some(items) = parsed["results"].as_array() {
        for item in items {
            let sentence = item["sentence"].as_str().unwrap_or("").trim().to_string();
            if sentence.is_empty() {
                continue;
            }

            let confidence = match item["confidence"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "high" | "medium" | "low" => item["confidence"].as_str().unwrap_or("").to_string(),
                _ => "medium".to_string(),
            };

            let allocation_raw = item["allocation"].as_str().unwrap_or("").trim();
            let allocation_norm = allocation_raw.to_lowercase();
            let allocation = if allocation_norm.is_empty()
                || allocation_norm == "system"
                || allocation_norm == "system-level"
                || allocation_norm == "system level"
            {
                "System Level".to_string()
            } else if let Some(exact) = subsystem_lookup.get(&allocation_norm) {
                exact.clone()
            } else {
                "System Level".to_string()
            };

            let mut new_subsystem_name = item["new_subsystem_name"]
                .as_str()
                .unwrap_or("")
                .trim()
                .replace('\n', " ");
            if new_subsystem_name.len() > 64 {
                new_subsystem_name = new_subsystem_name.chars().take(64).collect();
            }
            if new_subsystem_name.len() < 3 {
                new_subsystem_name.clear();
            }

            out.push(RequirementAllocationOutput {
                id: item["id"].as_str().unwrap_or("").trim().to_string(),
                sentence,
                allocation,
                confidence,
                rationale: item["rationale"].as_str().unwrap_or("").trim().to_string(),
                new_subsystem_name,
            });
        }
    }

    let output = serde_json::json!({ "results": out });
    Ok(output.to_string())
}

// -- AI requirement extraction (Claude / Anthropic) --------------------------

#[tauri::command]
pub async fn ai_extract_requirements(
    text: String,
    doc_type: Option<String>,
    doc_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let provider = state.ai_provider.lock().unwrap().clone();
    if !provider.is_available() {
        return Err("no_api_key".to_string());
    }

    let doc_label = doc_name.unwrap_or_else(|| "document".to_string());
    let dtype = doc_type.unwrap_or_else(|| "General".to_string());
    let is_local = provider.name() == "ollama";

    let all_results: Vec<serde_json::Value> = if is_local {
        run_chunked_local_extraction(provider.clone(), &text, &doc_label, &dtype, None).await
    } else {
        let trimmed: String = text.chars().take(60_000).collect();
        run_single_extraction(provider.clone(), &trimmed, &doc_label, &dtype, false, None)
            .await
            .map_err(|e| e.to_string())?
    };

    let output = serde_json::json!({ "results": all_results });
    Ok(output.to_string())
}

/// Split text into overlapping chunks, snapping boundaries to sentence endings.
fn chunk_text_by_sentences(text: &str, chunk_chars: usize, overlap_chars: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    if total <= chunk_chars {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < total {
        let raw_end = (start + chunk_chars).min(total);
        // Snap end forward to next sentence boundary
        let end = if raw_end < total {
            let lookahead = (raw_end + 300).min(total);
            chars[raw_end..lookahead]
                .iter()
                .position(|&c| c == '.' || c == '\n')
                .map(|p| raw_end + p + 1)
                .unwrap_or(raw_end)
        } else {
            raw_end
        };

        chunks.push(chars[start..end].iter().collect());
        if end >= total {
            break;
        }

        // Next chunk starts overlap chars back, snapped to a sentence start
        let raw_next = end.saturating_sub(overlap_chars);
        let next_start = chars[raw_next..end]
            .iter()
            .rposition(|&c| c == '.' || c == '\n')
            .map(|p| raw_next + p + 1)
            .unwrap_or(raw_next);

        start = if next_start > start { next_start } else { end };
    }

    chunks
}

/// Run local extraction chunk-by-chunk and merge unique requirement sentences.
async fn run_chunked_local_extraction(
    provider: Arc<dyn crate::ai::provider::AIProvider>,
    text: &str,
    doc_label: &str,
    dtype: &str,
    enrichment_context: Option<&str>,
) -> Vec<serde_json::Value> {
    let chunks = chunk_text_by_sentences(text, 6_000, 400);
    let total = chunks.len();
    let mut merged: Vec<serde_json::Value> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let label = format!("{doc_label} (part {}/{})", i + 1, total);
        let extracted = run_single_extraction(
            provider.clone(),
            chunk,
            &label,
            dtype,
            true,
            enrichment_context,
        )
        .await;

        if let Ok(items) = extracted {
            for item in items {
                let key = item["sentence"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_lowercase();
                if !key.is_empty() && seen.insert(key) {
                    merged.push(item);
                }
            }
        }
    }

    merged
}

/// Run extraction prompt on one chunk of text.
async fn run_single_extraction(
    provider: Arc<dyn crate::ai::provider::AIProvider>,
    text: &str,
    doc_label: &str,
    dtype: &str,
    is_local: bool,
    enrichment_context: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let naming_rules = "NAME FIELD RULES:\n\
- Derive name from the actual subject + constraint/measurement in that sentence.\n\
- 3-7 words, Title Case.\n\
- Include the key metric, component, or property if present (e.g. \"RF Link Margin 6 dB Minimum\", \"Watchdog Timeout Under 500 ms\", \"User Session Idle Logout 15 Min\").\n\
- NEVER use generic titles like \"System Requirement\", \"Performance Requirement\", \"Data Requirement\", \"Interface Requirement\", or any name that could apply to multiple requirements.\n";

    let (system, user) = if is_local {
        let mut sys = format!(
            "You are a requirements extraction tool. \
Extract every requirement from the text — technical, security, communications, \
programmatic, and contractor obligations. \
A requirement uses 'shall', 'must', or 'will'. \
Copy each requirement sentence VERBATIM. Never paraphrase or invent text. \
Return only valid JSON, no other text.\n\n\
{naming_rules}"
        );

        if let Some(ctx) = enrichment_context.map(str::trim).filter(|v| !v.is_empty()) {
            sys.push_str(
                "\nUse the context below as hints to resolve entities and relationships. \
Do not invent requirements that are not explicitly present in the excerpt.\n\n",
            );
            sys.push_str(ctx);
        }

        let usr = format!(
            "Extract every requirement from this excerpt of \"{doc_label}\" ({dtype}).\n\
Include ALL types: technical, performance, security, comms, interface, programmatic, reporting.\n\n\
---\n{text}\n---\n\n\
Return ONLY this JSON (no markdown, no explanation):\n\
{{\"results\":[{{\"sentence\":\"<verbatim text>\",\"name\":\"<specific descriptive name from subject+constraint>\",\
\"confidence\":\"high|medium|low\",\"flags\":[]}}]}}\n\n\
Confidence guide:\n\
- high: clear shall/must/will with explicit subject and measurable constraint\n\
- medium: likely requirement, implicit subject or missing measurement\n\
- low: possible obligation, ambiguous modal or missing subject\n\
- If no requirements found in this excerpt: {{\"results\":[]}}"
        );
        (sys, usr)
    } else {
        let sys = format!(
            "You are a precise requirements engineering assistant applying IEEE 29148.\n\
Extract every verifiable requirement from the document.\n\
Rules: copy sentence verbatim — no paraphrasing, split compound shalls into separate items, \
skip headings/rationale/notes, assign confidence high|medium|low, return only valid JSON.\n\n\
{naming_rules}"
        );
        let usr = format!(
            "Document: \"{doc_label}\" (type: {dtype})\n\n\
---\n{text}\n---\n\n\
Return JSON with a specific descriptive name for each requirement derived from its subject and constraint:\n\
{{\"results\":[{{\"sentence\":\"<exact verbatim text>\",\"name\":\"<specific name from subject+constraint>\",\
\"confidence\":\"high|medium|low\",\"flags\":[\"modal:shall\",\"has_measurement\",\"missing_subject\",...]}}]}}"
        );
        (sys, usr)
    };

    let prompt = Prompt {
        system: Some(system),
        messages: vec![Message {
            role: Role::User,
            content: user,
        }],
        max_tokens: Some(4096),
    };

    let response = provider.complete(prompt).await.map_err(|e| e.to_string())?;
    let raw = response.content.trim().to_string();

    let raw = if raw.starts_with("```") {
        raw.lines()
            .skip(1)
            .take_while(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        raw
    };

    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid JSON: {e}"))?;

    Ok(parsed["results"].as_array().cloned().unwrap_or_default())
}

// ── AI diagram generation ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct DiagramNodeInput {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiagramEdgeInput {
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
}

/// Ask the AI to decide which nodes to place in a diagram and return their
/// positions. Returns JSON: `{"placements":[{"node_id","x","y","width","height"}]}`
#[tauri::command]
pub async fn ai_generate_diagram(
    diagram_kind: String,
    diagram_name: String,
    nodes: Vec<DiagramNodeInput>,
    edges: Vec<DiagramEdgeInput>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let provider = state.ai_provider.lock().unwrap().clone();
    if !provider.is_available() {
        return Err("no_api_key".to_string());
    }
    if nodes.is_empty() {
        return Ok(serde_json::json!({ "placements": [] }).to_string());
    }

    let nodes_json = serde_json::to_string_pretty(&nodes).map_err(|e| e.to_string())?;
    let edges_json = serde_json::to_string_pretty(&edges).map_err(|e| e.to_string())?;

    let kind_guidance = match diagram_kind.as_str() {
        "bdd" => "Block Definition Diagram (BDD): show blocks and their composition/specialization relationships. Place the system root block at center-top. Subsystem blocks below it in a horizontal row.",
        "ibd" => "Internal Block Diagram (IBD): show internal structure with ports and interfaces. Use a grid layout.",
        "usecase" => "Use Case Diagram: actors on the left, use cases in an ellipse cluster in the center.",
        "sequence" => "Sequence Diagram: actors/blocks across the top as columns, interactions implied by order.",
        _ => "Arrange nodes in a clear hierarchical layout with related nodes close together.",
    };

    let prompt = Prompt {
        system: Some(format!(
            "You are an MBSE diagram layout engine. Given a set of model nodes and edges, \
select the most relevant nodes for a {diagram_kind} diagram and assign each a canvas position.\n\
\n\
Layout guidance: {kind_guidance}\n\
\n\
Canvas coordinate system: origin (0,0) is top-left. X increases right, Y increases down.\n\
Typical node width: 180, height: 90. Leave at least 40px gap between nodes.\n\
Use a canvas of roughly 1200 x 800.\n\
\n\
Return ONLY valid JSON:\n\
{{\"placements\":[{{\"node_id\":\"...\",\"x\":0,\"y\":0,\"width\":180,\"height\":90}}]}}\n\
Include only nodes relevant to a {diagram_kind}. Do not invent new node IDs."
        )),
        messages: vec![Message {
            role: Role::User,
            content: format!(
                "Diagram name: \"{diagram_name}\" (kind: {diagram_kind})\n\nNodes:\n{nodes_json}\n\nEdges:\n{edges_json}\n\nReturn the diagram layout."
            ),
        }],
        max_tokens: Some(2048),
    };

    let response = provider.complete(prompt).await.map_err(|e| e.to_string())?;
    let raw = response.content.trim().to_string();
    let json_str = extract_json_object(&raw).ok_or_else(|| {
        format!("AI did not return valid JSON. Output: {}", raw.chars().take(200).collect::<String>())
    })?;
    Ok(json_str)
}

// ── Suspect links ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_suspect_links(project_id: String, state: State<'_, AppState>) -> Result<Vec<crate::core::model::SuspectLink>, String> {
    let uuid: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.get_suspect_links(uuid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resolve_suspect_link(id: String, resolved_by: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.resolve_suspect_link(uuid, &resolved_by).await.map_err(|e| e.to_string())
}

// ── Inline comments ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_req_comment(
    project_id: String,
    node_id: String,
    parent_id: Option<String>,
    author: String,
    body: String,
    state: State<'_, AppState>,
) -> Result<crate::core::model::ReqComment, String> {
    let project_uuid: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let node_uuid: Uuid = node_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let parent_uuid = parent_id.map(|s| s.parse::<Uuid>().map_err(|e: uuid::Error| e.to_string())).transpose()?;
    state.store
        .add_req_comment(project_uuid, node_uuid, parent_uuid, &author, &body)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_req_comments(node_id: String, state: State<'_, AppState>) -> Result<Vec<crate::core::model::ReqComment>, String> {
    let uuid: Uuid = node_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.get_req_comments(uuid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_comment_counts(project_id: String, state: State<'_, AppState>) -> Result<std::collections::HashMap<String, i64>, String> {
    let uuid: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.get_comment_counts_for_project(uuid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resolve_req_comment(id: String, resolved_by: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.resolve_req_comment(uuid, &resolved_by).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_req_comment(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.delete_req_comment(uuid).await.map_err(|e| e.to_string())
}

// ── Review workflow ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_review_session(
    project_id: String,
    title: String,
    description: Option<String>,
    node_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<crate::core::model::ReviewSession, String> {
    let project_uuid: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    let node_uuids: Vec<Uuid> = node_ids.iter()
        .map(|s| s.parse::<Uuid>().map_err(|e: uuid::Error| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    state.store.create_review_session(project_uuid, &title, description.as_deref(), node_uuids).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_review_sessions(project_id: String, state: State<'_, AppState>) -> Result<Vec<crate::core::model::ReviewSession>, String> {
    let uuid: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.list_review_sessions(uuid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_review_verdict(
    item_id: String,
    verdict: String,
    verdict_by: String,
    note: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid: Uuid = item_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.set_review_verdict(uuid, &verdict, &verdict_by, note.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn close_review_session(session_id: String, status: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = session_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.close_review_session(uuid, &status).await.map_err(|e| e.to_string())
}

// ── Model baselines ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_baseline(
    project_id: String,
    name: String,
    description: Option<String>,
    created_by: Option<String>,
    state: State<'_, AppState>,
) -> Result<ModelBaseline, String> {
    let pid: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;

    // Collect the full model state into a JSON snapshot
    let nodes = state.store.list_nodes(pid).await.map_err(|e| e.to_string())?;
    let edges = {
        let mut all = Vec::new();
        for node in &nodes {
            let mut e = state
                .store
                .edges_for_node(node.id)
                .await
                .map_err(|e| e.to_string())?;
            all.append(&mut e);
        }
        all.sort_by_key(|e| e.id);
        all.dedup_by_key(|e| e.id);
        all
    };

    let snapshot = serde_json::json!({
        "nodes": nodes,
        "edges": edges,
    });

    let baseline = ModelBaseline {
        id: Uuid::new_v4(),
        project_id: pid,
        name,
        description: description.unwrap_or_default(),
        created_by: created_by.unwrap_or_else(|| "User".to_string()),
        created_at: Utc::now(),
        snapshot,
    };

    state.store.create_baseline(&baseline).await.map_err(|e| e.to_string())?;
    Ok(baseline)
}

#[tauri::command]
pub async fn list_baselines(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ModelBaseline>, String> {
    let pid: Uuid = project_id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.list_baselines(pid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_baseline(
    id: String,
    state: State<'_, AppState>,
) -> Result<ModelBaseline, String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state
        .store
        .get_baseline(uuid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "baseline not found".to_string())
}

#[tauri::command]
pub async fn delete_baseline(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid: Uuid = id.parse().map_err(|e: uuid::Error| e.to_string())?;
    state.store.delete_baseline(uuid).await.map_err(|e| e.to_string())
}

// ── GraphRAG requirement extraction (Ollama + knowledge graph) ───────────────

/// Extract requirements using a hybrid path:
/// 1) build GraphRAG entity/relationship context from the document
/// 2) run the normal `run_single_extraction` prompt over chunks, enriched by
///    that context
///
/// If GraphRAG initialisation fails (e.g. embedding model not pulled yet),
/// extraction still runs without enrichment.
///
/// Frontend calls this when the user has Ollama selected as provider AND
/// the "Use GraphRAG" toggle is enabled in the requirements-extractor page.
#[tauri::command]
pub async fn graphrag_extract_requirements(
    text: String,
    doc_type: Option<String>,
    doc_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use crate::ai::graphrag::{build_requirement_enrichment_context, GraphRagExtractorConfig};

    let doc_label = doc_name.unwrap_or_else(|| "document".to_string());
    let dtype = doc_type.unwrap_or_else(|| "General".to_string());
    let provider = state.ai_provider.lock().unwrap().clone();
    if !provider.is_available() {
        return Err("no_api_key".to_string());
    }

    // Read Ollama settings from the store (same keys as set_ollama_config).
    let base_url = state
        .store
        .get_setting("ai.ollama.base_url", None)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    let chat_model = state
        .store
        .get_setting("ai.ollama.model", None)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "qwen2.5:7b".to_string());

    let embed_model = state
        .store
        .get_setting("ai.ollama.embed_model", None)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "nomic-embed-text".to_string());

    let cfg = GraphRagExtractorConfig {
        ollama_base_url: base_url,
        ollama_chat_model: chat_model,
        ollama_embed_model: embed_model,
        ..Default::default()
    };

    // Cap input the same way ai_extract_requirements does for local models.
    let capped: String = text.chars().take(60_000).collect();

    // Build graph context first, then run the same extraction prompt used by
    // run_single_extraction with this context injected.
    let graph_context = match build_requirement_enrichment_context(&capped, &cfg).await {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("GraphRAG context build warning (continuing without): {e}");
            String::new()
        }
    };

    let graph_context = graph_context.trim().to_string();
    let results = if graph_context.is_empty() {
        run_chunked_local_extraction(provider, &capped, &doc_label, &dtype, None).await
    } else {
        run_chunked_local_extraction(
            provider,
            &capped,
            &doc_label,
            &dtype,
            Some(graph_context.as_str()),
        )
        .await
    };

    let output = serde_json::json!({ "results": results });
    Ok(output.to_string())
}

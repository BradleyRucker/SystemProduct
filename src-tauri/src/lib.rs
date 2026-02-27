use std::sync::{Arc, Mutex};
use tauri::Manager;

pub mod ai;
pub mod commands;
pub mod core;
pub mod diagrams;
pub mod events;

use ai::provider::{AIProvider, NullProvider};
use core::store::Store;

pub struct AppState {
    pub store: Store,
    pub ai_provider: Mutex<Arc<dyn AIProvider>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&data_dir)?;

            let db_path = data_dir.join("systemproduct.db");
            let db_path_str = db_path.to_string_lossy().to_string();

            // Bootstrap async runtime for store initialization
            let store = tauri::async_runtime::block_on(async {
                Store::open(&db_path_str)
                    .await
                    .expect("failed to open database")
            });

            // Resolve AI provider: env var → DB active provider → NullProvider.
            let ai_provider: Arc<dyn AIProvider> = tauri::async_runtime::block_on(async {
                // Env var always wins
                let env_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
                if !env_key.is_empty() {
                    return Arc::new(ai::anthropic::AnthropicProvider::new(env_key))
                        as Arc<dyn AIProvider>;
                }

                // Check which provider was last saved
                let saved_provider = store
                    .get_setting("ai.provider", None)
                    .await
                    .unwrap_or(None)
                    .unwrap_or_default();

                match saved_provider.as_str() {
                    "anthropic" => {
                        let key = store
                            .get_setting("ai.anthropic.api_key", None)
                            .await
                            .unwrap_or(None)
                            .unwrap_or_default();
                        if !key.is_empty() {
                            return Arc::new(ai::anthropic::AnthropicProvider::new(key))
                                as Arc<dyn AIProvider>;
                        }
                    }
                    "ollama" => {
                        let model = store
                            .get_setting("ai.ollama.model", None)
                            .await
                            .unwrap_or(None)
                            .unwrap_or_else(|| "qwen2.5:7b".to_string());
                        let base_url = store
                            .get_setting("ai.ollama.base_url", None)
                            .await
                            .unwrap_or(None);
                        return Arc::new(ai::ollama::OllamaProvider::new(model, base_url))
                            as Arc<dyn AIProvider>;
                    }
                    _ => {}
                }

                Arc::new(NullProvider) as Arc<dyn AIProvider>
            });

            app.manage(AppState {
                store,
                ai_provider: Mutex::new(ai_provider),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_projects,
            commands::create_project,
            commands::get_project,
            commands::delete_project,
            commands::list_nodes,
            commands::upsert_node,
            commands::list_requirement_history,
            commands::delete_node,
            commands::upsert_edge,
            commands::delete_edge,
            commands::edges_for_node,
            commands::list_diagrams,
            commands::upsert_diagram,
            commands::diagram_elements,
            commands::upsert_diagram_element,
            commands::delete_diagram,
            commands::list_documents,
            commands::upsert_document,
            commands::delete_document,
            commands::list_document_sections,
            commands::list_project_document_sections,
            commands::upsert_document_section,
            commands::delete_document_section,
            commands::delete_document_sections,
            commands::list_subsystem_knowledge,
            commands::upsert_subsystem_knowledge,
            commands::delete_subsystem_knowledge,
            commands::list_subsystem_artifacts,
            commands::list_project_artifacts,
            commands::upsert_subsystem_artifact,
            commands::delete_subsystem_artifact,
            commands::list_subsystem_activity,
            commands::add_subsystem_activity,
            commands::get_setting,
            commands::set_setting,
            commands::validate_model,
            commands::export_markdown,
            commands::export_json,
            commands::export_xmi,
            commands::ai_available,
            commands::ai_provider_name,
            commands::set_anthropic_key,
            commands::ollama_status,
            commands::set_ollama_config,
            commands::parse_requirements,
            commands::local_llm_available,
            commands::llm_extract_requirements,
            commands::ai_quality_pass_requirements,
            commands::ai_suggest_requirement_allocations,
            commands::ai_extract_requirements,
            commands::graphrag_extract_requirements,
            commands::ai_generate_diagram,
            commands::get_suspect_links,
            commands::resolve_suspect_link,
            commands::add_req_comment,
            commands::get_req_comments,
            commands::get_comment_counts,
            commands::resolve_req_comment,
            commands::delete_req_comment,
            commands::create_review_session,
            commands::list_review_sessions,
            commands::set_review_verdict,
            commands::close_review_session,
            commands::save_sim_params,
            commands::get_sim_params,
            commands::save_scenario,
            commands::list_scenarios,
            commands::run_simulation,
            commands::get_simulation_result,
            commands::create_baseline,
            commands::list_baselines,
            commands::get_baseline,
            commands::delete_baseline,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use crate::ai::context::ContextBuilder;
/// Background passive analysis — runs after edits settle (debounced).
/// Produces AiSuggestion rows written to SQLite.
/// Never blocks the UI thread. Never pops notifications.
use crate::ai::provider::{AIProvider, Message, Prompt, Role};
use crate::core::model::Node;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSuggestion {
    pub id: Uuid,
    pub project_id: Uuid,
    pub diagram_id: Option<Uuid>,
    pub kind: SuggestionKind,
    pub payload: Value,
    pub rationale: String,
    pub severity: Option<Severity>,
    pub target_node_id: Option<Uuid>,
    pub target_field: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionKind {
    Node,
    Edge,
    Text,
    Analysis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Analyze a set of requirements for common quality issues.
/// Returns suggestions — the caller persists them.
pub async fn analyze_requirements(
    provider: &dyn AIProvider,
    project_id: Uuid,
    nodes: &[Node],
) -> Result<Vec<AiSuggestion>> {
    if !provider.is_available() {
        return Ok(vec![]);
    }

    let ctx = ContextBuilder::new();
    let req_context = ctx.requirements_context(nodes);

    let prompt = Prompt {
        system: Some(ctx.system_prompt().to_string()),
        messages: vec![Message {
            role: Role::User,
            content: format!(
                "Analyze the following requirements for quality issues. Each requirement includes: \
                 req_id, name, text, priority (shall/should/may), status (draft/approved/obsolete), \
                 verification_method (null if unset), and allocations (empty array if none).\n\
                 \n\
                 Check for ALL of the following issues — report each one that applies:\n\
                 1. AMBIGUITY: vague terms like \"fast\", \"user-friendly\", \"adequate\", \"as needed\", \
                    \"high performance\" that cannot be verified objectively.\n\
                 2. MISSING VERIFICATION: verification_method is null — flag it and suggest the most \
                    appropriate method (Test, Analysis, Inspection, or Demonstration) based on the text.\n\
                 3. UNALLOCATED: allocations is empty — flag requirements with no subsystem assigned.\n\
                 4. PRIORITY MISMATCH: text uses \"shall\" language but priority is \"should\" or \"may\", \
                    or text uses \"may\" language but priority is \"shall\".\n\
                 5. INCOMPLETENESS: requirement is missing measurable acceptance criteria (no numeric \
                    threshold, range, or pass/fail condition).\n\
                 6. NAME QUALITY: name is generic (e.g. ends with \"Requirement\", or could describe \
                    a dozen different requirements) — suggest a specific 3-7 word name derived from \
                    subject + constraint/measurement in the text.\n\
                 \n\
                 Return ONLY a JSON array. Each object must have:\n\
                 - \"req_id\": the requirement's req_id string\n\
                 - \"issue\": a concise, specific description of the problem (1-2 sentences)\n\
                 - \"severity\": \"error\" (blocks approval), \"warning\" (should fix), or \"info\" (minor)\n\
                 - \"field\": which field is affected (\"text\", \"name\", \"verification_method\", \
                    \"allocations\", \"priority\")\n\
                 - \"suggestion\": a concrete fix or replacement value (e.g. the suggested \
                    verification method, the improved name, or the rewritten text fragment)\n\
                 \n\
                 Requirements:\n{req_context}"
            ),
        }],
        max_tokens: Some(2048),
    };

    let response = provider.complete(prompt).await?;

    // Parse the JSON array from the response
    let issues: Vec<Value> = extract_json_array(&response.content)?;

    let suggestions = issues
        .into_iter()
        .filter_map(|issue| {
            let req_id_str = issue["req_id"].as_str()?;
            // Find the matching node
            let target_node = nodes.iter().find(|n| {
                if let crate::core::model::NodeData::Requirement(r) = &n.data {
                    r.req_id.as_deref() == Some(req_id_str)
                } else {
                    false
                }
            })?;

            let severity = match issue["severity"].as_str()? {
                "error" => Severity::Error,
                "warning" => Severity::Warning,
                _ => Severity::Info,
            };

            let target_field = issue["field"].as_str().map(|s| s.to_string());
            let rationale = {
                let issue_text = issue["issue"].as_str().unwrap_or("");
                let suggestion_text = issue["suggestion"].as_str().unwrap_or("");
                if suggestion_text.is_empty() {
                    issue_text.to_string()
                } else {
                    format!("{issue_text} Suggestion: {suggestion_text}")
                }
            };

            Some(AiSuggestion {
                id: Uuid::new_v4(),
                project_id,
                diagram_id: None,
                kind: SuggestionKind::Analysis,
                payload: issue.clone(),
                rationale,
                severity: Some(severity),
                target_node_id: Some(target_node.id),
                target_field,
                created_at: Utc::now(),
            })
        })
        .collect();

    Ok(suggestions)
}

fn extract_json_array(text: &str) -> Result<Vec<Value>> {
    // Find the first '[' and last ']' to extract the JSON array,
    // handling models that wrap output in markdown code fences.
    let start = text.find('[').unwrap_or(0);
    let end = text.rfind(']').map(|i| i + 1).unwrap_or(text.len());
    let slice = &text[start..end];
    Ok(serde_json::from_str(slice)?)
}

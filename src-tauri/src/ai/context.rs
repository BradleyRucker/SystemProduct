/// Builds AI prompts that are grounded in the current model state.
/// The goal is to give the AI just enough context to be useful without
/// sending the entire graph on every call.
use crate::core::model::{Edge, Node, NodeKind};
use serde_json::json;

pub struct ContextBuilder {
    system_preamble: String,
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self {
            system_preamble: include_str!("prompts/system.txt").to_string(),
        }
    }

    /// Context for requirement analysis / generation tasks.
    /// Includes priority, status, verification_method, and allocations so the
    /// AI can give model-aware, actionable suggestions (e.g. flag missing
    /// verification methods, note unallocated requirements, etc.).
    pub fn requirements_context(&self, nodes: &[Node]) -> String {
        let reqs: Vec<_> = nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Requirement)
            .map(|n| {
                if let crate::core::model::NodeData::Requirement(r) = &n.data {
                    json!({
                        "id": n.id,
                        "req_id": r.req_id.as_deref().unwrap_or(""),
                        "name": n.name,
                        "text": r.text.as_deref().unwrap_or(""),
                        "priority": r.priority,
                        "status": r.status,
                        "verification_method": r.verification_method,
                        "allocations": r.allocations.as_deref().unwrap_or(&[]),
                    })
                } else {
                    json!({
                        "id": n.id,
                        "req_id": "",
                        "name": n.name,
                        "text": "",
                        "priority": null,
                        "status": null,
                        "verification_method": null,
                        "allocations": [],
                    })
                }
            })
            .collect();

        serde_json::to_string_pretty(&reqs).unwrap_or_default()
    }

    /// Context for BDD generation from a set of blocks + edges
    pub fn bdd_context(&self, nodes: &[Node], edges: &[Edge]) -> String {
        let blocks: Vec<_> = nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Block | NodeKind::Interface))
            .map(|n| json!({ "id": n.id, "name": n.name, "kind": n.kind.to_string() }))
            .collect();

        let rels: Vec<_> = edges
            .iter()
            .map(|e| json!({ "kind": e.kind.to_string(), "source": e.source_id, "target": e.target_id }))
            .collect();

        serde_json::to_string_pretty(&json!({ "blocks": blocks, "relationships": rels }))
            .unwrap_or_default()
    }

    pub fn system_prompt(&self) -> &str {
        &self.system_preamble
    }
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

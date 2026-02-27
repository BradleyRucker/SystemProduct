use crate::core::model::{Edge, EdgeKind, Node, NodeData, NodeKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub id: Uuid,
    pub severity: IssueSeverity,
    pub code: &'static str,
    pub message: String,
    pub node_id: Option<Uuid>,
    pub edge_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Error,
    Warning,
    Info,
}

/// Run all structural validation rules against the current model.
/// Returns an empty vec when the model is valid.
pub fn validate(nodes: &[Node], edges: &[Edge]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    for node in nodes {
        issues.extend(validate_node(node));
    }

    for edge in edges {
        issues.extend(validate_edge(edge, nodes));
    }

    issues
}

fn validate_node(node: &Node) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if node.name.trim().is_empty() {
        issues.push(ValidationIssue {
            id: Uuid::new_v4(),
            severity: IssueSeverity::Warning,
            code: "NODE_UNNAMED",
            message: format!("{} has no name", node.kind),
            node_id: Some(node.id),
            edge_id: None,
        });
    }

    if let crate::core::model::NodeData::Requirement(r) = &node.data {
        if r.text.as_ref().map(|t| t.trim().is_empty()).unwrap_or(true) {
            issues.push(ValidationIssue {
                id: Uuid::new_v4(),
                severity: IssueSeverity::Warning,
                code: "REQ_NO_TEXT",
                message: format!(
                    "Requirement '{}' has no requirement text",
                    r.req_id.as_deref().unwrap_or(&node.name)
                ),
                node_id: Some(node.id),
                edge_id: None,
            });
        }

        if r.verification_method.is_none() {
            issues.push(ValidationIssue {
                id: Uuid::new_v4(),
                severity: IssueSeverity::Info,
                code: "REQ_NO_VERIF",
                message: format!(
                    "Requirement '{}' has no verification method",
                    r.req_id.as_deref().unwrap_or(&node.name)
                ),
                node_id: Some(node.id),
                edge_id: None,
            });
        }
    }

    issues
}

fn validate_edge(edge: &Edge, nodes: &[Node]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    let source = nodes.iter().find(|n| n.id == edge.source_id);
    let target = nodes.iter().find(|n| n.id == edge.target_id);

    match (source, target) {
        (None, _) => issues.push(ValidationIssue {
            id: Uuid::new_v4(),
            severity: IssueSeverity::Error,
            code: "EDGE_DANGLING_SOURCE",
            message: format!("Edge {:?} has a missing source node", edge.kind),
            node_id: None,
            edge_id: Some(edge.id),
        }),
        (_, None) => issues.push(ValidationIssue {
            id: Uuid::new_v4(),
            severity: IssueSeverity::Error,
            code: "EDGE_DANGLING_TARGET",
            message: format!("Edge {:?} has a missing target node", edge.kind),
            node_id: None,
            edge_id: Some(edge.id),
        }),
        (Some(src), Some(tgt)) => {
            // Semantic rules per edge kind
            match edge.kind {
                EdgeKind::Satisfies => {
                    if tgt.kind != NodeKind::Requirement {
                        issues.push(ValidationIssue {
                            id: Uuid::new_v4(),
                            severity: IssueSeverity::Error,
                            code: "SATISFIES_WRONG_TARGET",
                            message: format!(
                                "«satisfies» target must be a Requirement, got {}",
                                tgt.kind
                            ),
                            node_id: Some(tgt.id),
                            edge_id: Some(edge.id),
                        });
                    }
                    if src.kind != NodeKind::Block {
                        issues.push(ValidationIssue {
                            id: Uuid::new_v4(),
                            severity: IssueSeverity::Warning,
                            code: "SATISFIES_UNUSUAL_SOURCE",
                            message: format!(
                                "«satisfies» source is usually a Block, got {}",
                                src.kind
                            ),
                            node_id: Some(src.id),
                            edge_id: Some(edge.id),
                        });
                    }
                }
                EdgeKind::Verifies => {
                    if src.kind != NodeKind::TestCase {
                        issues.push(ValidationIssue {
                            id: Uuid::new_v4(),
                            severity: IssueSeverity::Error,
                            code: "VERIFIES_WRONG_SOURCE",
                            message: "«verifies» source must be a TestCase".to_string(),
                            node_id: Some(src.id),
                            edge_id: Some(edge.id),
                        });
                    }
                    if tgt.kind != NodeKind::Requirement {
                        issues.push(ValidationIssue {
                            id: Uuid::new_v4(),
                            severity: IssueSeverity::Error,
                            code: "VERIFIES_WRONG_TARGET",
                            message: "«verifies» target must be a Requirement".to_string(),
                            node_id: Some(tgt.id),
                            edge_id: Some(edge.id),
                        });
                    }
                }
                EdgeKind::Connects => {
                    // Allow port-to-port, block-to-block (simulation signal flow),
                    // and port-to-block/block-to-port (mixed). Warn if mixing kinds.
                    let src_ok = matches!(src.kind, NodeKind::Port | NodeKind::Block);
                    let tgt_ok = matches!(tgt.kind, NodeKind::Port | NodeKind::Block);
                    if !src_ok || !tgt_ok {
                        issues.push(ValidationIssue {
                            id: Uuid::new_v4(),
                            severity: IssueSeverity::Error,
                            code: "CONNECTS_INVALID_ENDPOINT",
                            message: "«connects» endpoints must be Ports or Blocks".to_string(),
                            node_id: None,
                            edge_id: Some(edge.id),
                        });
                    }
                    // PORT_TYPE_MISMATCH: warn if both are ports with conflicting typed names
                    if src.kind == NodeKind::Port && tgt.kind == NodeKind::Port {
                        if let (NodeData::Port(sp), NodeData::Port(tp)) = (&src.data, &tgt.data) {
                            if let (Some(st), Some(tt)) = (&sp.type_name, &tp.type_name) {
                                if st != tt {
                                    issues.push(ValidationIssue {
                                        id: Uuid::new_v4(),
                                        severity: IssueSeverity::Warning,
                                        code: "PORT_TYPE_MISMATCH",
                                        message: format!(
                                            "Port type mismatch: «{}» connected to «{}»",
                                            st, tt
                                        ),
                                        node_id: None,
                                        edge_id: Some(edge.id),
                                    });
                                }
                            }
                        }
                    }
                }
                EdgeKind::Transition => {
                    if src.kind != NodeKind::State || tgt.kind != NodeKind::State {
                        issues.push(ValidationIssue {
                            id: Uuid::new_v4(),
                            severity: IssueSeverity::Error,
                            code: "TRANSITION_NOT_STATES",
                            message: "«transition» must link two States".to_string(),
                            node_id: None,
                            edge_id: Some(edge.id),
                        });
                    }
                }
                EdgeKind::BindingConnector => {
                    // binding connectors link ports on constraint blocks; permissive validation
                    let src_ok = matches!(src.kind, NodeKind::Port | NodeKind::ConstraintBlock | NodeKind::Block);
                    let tgt_ok = matches!(tgt.kind, NodeKind::Port | NodeKind::ConstraintBlock | NodeKind::Block);
                    if !src_ok || !tgt_ok {
                        issues.push(ValidationIssue {
                            id: Uuid::new_v4(),
                            severity: IssueSeverity::Warning,
                            code: "BINDING_CONNECTOR_UNUSUAL",
                            message: "«bindingConnector» usually links Ports or ConstraintBlocks".to_string(),
                            node_id: None,
                            edge_id: Some(edge.id),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    issues
}

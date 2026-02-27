/// ELK layout engine integration.
/// ELK runs as a JS worker (elk.js bundled in the frontend).
/// This module defines the Rust-side data structures that are
/// serialized to JSON, passed to the frontend worker, and whose
/// results are deserialized back into DiagramElement positions.
///
/// Layout pipeline
/// ---------------
/// Generating a diagram goes through four named phases so the frontend can
/// display incremental progress and the backend can reason about each step:
///
///  Phase 0 – BUILD    Build the raw node/edge lists from the model store.
///  Phase 1 – GROUP    Cluster nodes into logical groups (by kind, subsystem,
///                     or allocation) so ELK can apply compound-node layout.
///  Phase 2 – ELK      Serialise to ElkGraph, run elk.js in the JS worker,
///                     receive positioned coordinates.
///  Phase 3 – FINALISE Apply waypoints from ELK sections; persist positions
///                     back to diagram_elements in SQLite.
///
/// Each phase emits a `LayoutPhaseEvent` that is forwarded to the frontend
/// via Tauri events so the UI can show a progress indicator.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Layout pipeline phases ─────────────────────────────────────────────────

/// Identifies which phase of the layout pipeline is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutPhase {
    /// Phase 0 — collect nodes/edges from the model store.
    Build,
    /// Phase 1 — group nodes into compound clusters.
    Group,
    /// Phase 2 — run ELK layout algorithm in the JS worker.
    Elk,
    /// Phase 3 — apply positions and persist to SQLite.
    Finalise,
}

/// Emitted as a Tauri event (`diagram://layout-phase`) so the frontend can
/// render a step-by-step progress indicator similar to a workflow studio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPhaseEvent {
    pub diagram_id: Uuid,
    pub phase: LayoutPhase,
    /// Human-readable status message for the UI.
    pub message: String,
}

impl LayoutPhaseEvent {
    pub fn new(diagram_id: Uuid, phase: LayoutPhase, message: impl Into<String>) -> Self {
        Self {
            diagram_id,
            phase,
            message: message.into(),
        }
    }
}

// ── Node grouping ──────────────────────────────────────────────────────────

/// A logical group of nodes used by Phase 1 (GROUP).
/// Maps 1:1 to an ELK compound node (a parent with children).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeGroup {
    /// Stable ID for the group (derived from group label so it is
    /// deterministic across re-runs).
    pub id: String,
    /// Display label shown on the group boundary box in the canvas.
    pub label: String,
    /// Node IDs that belong to this group.
    pub member_ids: Vec<Uuid>,
    /// Preferred ELK layout direction for children inside this group.
    pub direction: GroupDirection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GroupDirection {
    Down,
    Right,
}

impl GroupDirection {
    pub fn as_elk_str(self) -> &'static str {
        match self {
            GroupDirection::Down => "DOWN",
            GroupDirection::Right => "RIGHT",
        }
    }
}

/// Partition a flat node list into groups by `node_kind` string.
/// All nodes of the same kind go into one group.  Unrecognised kinds form a
/// catch-all group called "Other".
///
/// This is intentionally simple — callers can replace it with domain-specific
/// logic (e.g. subsystem allocation, diagram section, etc.).
pub fn group_by_kind(nodes: &[(Uuid, String)]) -> Vec<NodeGroup> {
    use std::collections::HashMap;

    let mut buckets: HashMap<String, Vec<Uuid>> = HashMap::new();
    for (id, kind) in nodes {
        buckets.entry(kind.clone()).or_default().push(*id);
    }

    buckets
        .into_iter()
        .map(|(kind, member_ids)| {
            let id = format!("group-{kind}");
            NodeGroup {
                id,
                label: kind_display_label(&kind),
                member_ids,
                direction: kind_preferred_direction(&kind),
            }
        })
        .collect()
}

fn kind_display_label(kind: &str) -> String {
    match kind {
        "block" => "Blocks".to_string(),
        "requirement" => "Requirements".to_string(),
        "interface" => "Interfaces".to_string(),
        "port" => "Ports".to_string(),
        "actor" => "Actors".to_string(),
        "use_case" => "Use Cases".to_string(),
        "state" => "States".to_string(),
        "action" => "Actions".to_string(),
        other => {
            // Title-case the kind string as a fallback.
            let mut chars = other.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}

fn kind_preferred_direction(kind: &str) -> GroupDirection {
    match kind {
        "state" | "action" => GroupDirection::Right,
        _ => GroupDirection::Down,
    }
}

/// Input graph sent to ELK
#[derive(Debug, Serialize)]
pub struct ElkGraph {
    pub id: String,
    #[serde(rename = "layoutOptions")]
    pub layout_options: ElkLayoutOptions,
    pub children: Vec<ElkNode>,
    pub edges: Vec<ElkEdge>,
}

#[derive(Debug, Serialize)]
pub struct ElkLayoutOptions {
    #[serde(rename = "elk.algorithm")]
    pub algorithm: String,
    #[serde(rename = "elk.direction")]
    pub direction: String,
    #[serde(rename = "elk.spacing.nodeNode")]
    pub node_spacing: f64,
    #[serde(rename = "elk.layered.spacing.nodeNodeBetweenLayers")]
    pub layer_spacing: f64,
    #[serde(rename = "elk.hierarchyHandling")]
    pub hierarchy_handling: String,
}

impl Default for ElkLayoutOptions {
    fn default() -> Self {
        Self {
            algorithm: "layered".to_string(),
            direction: "DOWN".to_string(),
            node_spacing: 32.0,
            layer_spacing: 48.0,
            hierarchy_handling: "INCLUDE_CHILDREN".to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ElkNode {
    pub id: String,
    pub width: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ElkNode>,
}

#[derive(Debug, Serialize)]
pub struct ElkEdge {
    pub id: String,
    pub sources: Vec<String>,
    pub targets: Vec<String>,
}

/// Output from ELK after layout
#[derive(Debug, Deserialize)]
pub struct ElkResult {
    pub children: Vec<ElkNodeResult>,
    pub edges: Vec<ElkEdgeResult>,
}

#[derive(Debug, Deserialize)]
pub struct ElkNodeResult {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Deserialize)]
pub struct ElkEdgeResult {
    pub id: String,
    pub sections: Vec<ElkSection>,
}

#[derive(Debug, Deserialize)]
pub struct ElkSection {
    #[serde(rename = "startPoint")]
    pub start_point: ElkPoint,
    #[serde(rename = "endPoint")]
    pub end_point: ElkPoint,
    #[serde(rename = "bendPoints", default)]
    pub bend_points: Vec<ElkPoint>,
}

#[derive(Debug, Deserialize)]
pub struct ElkPoint {
    pub x: f64,
    pub y: f64,
}

/// Build an ELK input graph from the node/edge lists for a diagram.
///
/// When `groups` is non-empty (Phase 1 – GROUP output), each group is emitted
/// as an ELK compound node whose children are the member leaf nodes.  This
/// produces the bracketed, section-labelled layout visible in workflow studio
/// tools.  When `groups` is empty the graph is flat (original behaviour).
pub fn build_elk_graph(
    diagram_id: Uuid,
    nodes: &[(Uuid, f64, f64)],   // (id, default_width, default_height)
    edges: &[(Uuid, Uuid, Uuid)], // (id, source, target)
    options: ElkLayoutOptions,
) -> ElkGraph {
    build_elk_graph_with_groups(diagram_id, nodes, edges, &[], options)
}

/// Variant of [`build_elk_graph`] that accepts grouping information from the
/// GROUP phase.  Each [`NodeGroup`] becomes an ELK compound parent node
/// containing its member leaf nodes as `children`.
pub fn build_elk_graph_with_groups(
    diagram_id: Uuid,
    nodes: &[(Uuid, f64, f64)],
    edges: &[(Uuid, Uuid, Uuid)],
    groups: &[NodeGroup],
    options: ElkLayoutOptions,
) -> ElkGraph {
    use std::collections::HashMap;

    // Build a lookup: node_id → (width, height)
    let sizes: HashMap<Uuid, (f64, f64)> = nodes.iter().map(|(id, w, h)| (*id, (*w, *h))).collect();

    // Build a lookup: node_id → group_id  (nodes not in any group → ungrouped)
    let mut node_to_group: HashMap<Uuid, String> = HashMap::new();
    for grp in groups {
        for mid in &grp.member_ids {
            node_to_group.insert(*mid, grp.id.clone());
        }
    }

    let mut top_level_children: Vec<ElkNode> = Vec::new();

    if groups.is_empty() {
        // Flat layout — original behaviour.
        for (id, w, h) in nodes {
            top_level_children.push(ElkNode {
                id: id.to_string(),
                width: *w,
                height: *h,
                children: vec![],
            });
        }
    } else {
        // Grouped layout — emit compound parent nodes, then ungrouped leaf nodes.
        for grp in groups {
            let child_nodes: Vec<ElkNode> = grp
                .member_ids
                .iter()
                .filter_map(|mid| {
                    let (w, h) = sizes.get(mid)?;
                    Some(ElkNode {
                        id: mid.to_string(),
                        width: *w,
                        height: *h,
                        children: vec![],
                    })
                })
                .collect();

            // Compound group node — ELK sizes it to fit its children.
            // We give it a small default so ELK does not collapse it if
            // all members happen to be filtered out.
            let grp_node = ElkNode {
                id: grp.id.clone(),
                width: 40.0,
                height: 40.0,
                children: child_nodes,
            };
            top_level_children.push(grp_node);
        }

        // Nodes that are not in any group are placed at the top level.
        for (id, w, h) in nodes {
            if !node_to_group.contains_key(id) {
                top_level_children.push(ElkNode {
                    id: id.to_string(),
                    width: *w,
                    height: *h,
                    children: vec![],
                });
            }
        }
    }

    ElkGraph {
        id: diagram_id.to_string(),
        layout_options: options,
        children: top_level_children,
        edges: edges
            .iter()
            .map(|(id, src, tgt)| ElkEdge {
                id: id.to_string(),
                sources: vec![src.to_string()],
                targets: vec![tgt.to_string()],
            })
            .collect(),
    }
}

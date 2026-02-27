/// Diagram Intermediate Representation
/// Typed JSON structure passed from Rust → frontend for rendering.
/// The frontend never queries SQLite directly — it consumes this IR.
///
/// The IR now carries optional group information produced by the layout
/// pipeline's GROUP phase.  When `groups` is non-empty the frontend renders
/// compound bounding boxes (labelled sections) around the member nodes,
/// similar to the bracketed swim-lane sections in workflow studio tools.
use crate::core::model::{DiagramEdgeRoute, DiagramElement, DiagramKind, Edge, Node};
use crate::diagrams::layout::{LayoutPhase, NodeGroup};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramIR {
    pub diagram_id: Uuid,
    pub kind: DiagramKind,
    pub name: String,
    pub nodes: Vec<IRNode>,
    pub edges: Vec<IREdge>,
    /// Compound groups produced by the GROUP phase.  Empty for flat diagrams.
    #[serde(default)]
    pub groups: Vec<IRNodeGroup>,
    /// Which pipeline phase this IR was last produced by.
    /// Lets the frontend show a progress state (e.g. skeleton nodes while ELK runs).
    #[serde(default)]
    pub layout_phase: Option<LayoutPhase>,
}

/// A logical grouping of nodes displayed as a labelled bounding box.
/// Corresponds 1:1 to a [`NodeGroup`] from the layout pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRNodeGroup {
    pub id: String,
    pub label: String,
    pub member_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRNode {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub description: String,
    /// Serialized kind-specific data (passed through as-is)
    pub data: serde_json::Value,
    /// Canvas position from diagram_elements
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub collapsed: bool,
    pub style_overrides: serde_json::Value,
    /// True if this node has a pending AI suggestion ghost
    pub has_suggestion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IREdge {
    pub id: Uuid,
    pub kind: String,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub label: String,
    pub waypoints: Vec<IRPoint>,
    pub has_suggestion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRPoint {
    pub x: f64,
    pub y: f64,
}

/// Build the IR for a diagram from its constituent parts.
///
/// `groups` may be empty (flat layout) or populated from the GROUP phase.
/// `layout_phase` records which pipeline phase produced this IR so the
/// frontend can render an appropriate progress indicator.
pub fn build_ir(
    diagram_id: Uuid,
    kind: DiagramKind,
    name: String,
    nodes: &[Node],
    edges: &[Edge],
    elements: &[DiagramElement],
    routes: &[DiagramEdgeRoute],
    suggested_node_ids: &[Uuid],
    suggested_edge_ids: &[Uuid],
    groups: &[NodeGroup],
    layout_phase: Option<LayoutPhase>,
) -> DiagramIR {
    let ir_nodes = elements
        .iter()
        .filter_map(|el| {
            let node = nodes.iter().find(|n| n.id == el.node_id)?;
            Some(IRNode {
                id: node.id,
                kind: node.kind.to_string(),
                name: node.name.clone(),
                description: node.description.clone(),
                data: serde_json::to_value(&node.data).unwrap_or(serde_json::Value::Null),
                x: el.x,
                y: el.y,
                width: el.width,
                height: el.height,
                collapsed: el.collapsed,
                style_overrides: serde_json::to_value(&el.style_overrides)
                    .unwrap_or(serde_json::Value::Null),
                has_suggestion: suggested_node_ids.contains(&node.id),
            })
        })
        .collect();

    // Include only edges where both endpoints appear in this diagram
    let element_node_ids: Vec<Uuid> = elements.iter().map(|e| e.node_id).collect();

    let ir_edges = edges
        .iter()
        .filter(|e| {
            element_node_ids.contains(&e.source_id) && element_node_ids.contains(&e.target_id)
        })
        .map(|edge| {
            let waypoints = routes
                .iter()
                .find(|r| r.edge_id == edge.id)
                .map(|r| {
                    r.waypoints
                        .iter()
                        .map(|p| IRPoint { x: p.x, y: p.y })
                        .collect()
                })
                .unwrap_or_default();

            IREdge {
                id: edge.id,
                kind: edge.kind.to_string(),
                source_id: edge.source_id,
                target_id: edge.target_id,
                label: edge.label.clone(),
                waypoints,
                has_suggestion: suggested_edge_ids.contains(&edge.id),
            }
        })
        .collect();

    let ir_groups = groups
        .iter()
        .map(|g| IRNodeGroup {
            id: g.id.clone(),
            label: g.label.clone(),
            member_ids: g.member_ids.clone(),
        })
        .collect();

    DiagramIR {
        diagram_id,
        kind,
        name,
        nodes: ir_nodes,
        edges: ir_edges,
        groups: ir_groups,
        layout_phase,
    }
}

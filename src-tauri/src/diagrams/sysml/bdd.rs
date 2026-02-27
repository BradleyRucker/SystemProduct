/// Block Definition Diagram rules.
/// BDDs show block hierarchies (Composes, Specializes) and
/// Requirement satisfaction links.
use crate::core::model::{EdgeKind, NodeKind};

/// Returns true if a node of this kind may appear in a BDD.
pub fn node_allowed(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Block | NodeKind::Interface | NodeKind::Requirement | NodeKind::Port
    )
}

/// Returns true if an edge of this kind may appear in a BDD.
pub fn edge_allowed(kind: &EdgeKind) -> bool {
    matches!(
        kind,
        EdgeKind::Composes
            | EdgeKind::Specializes
            | EdgeKind::Satisfies
            | EdgeKind::Traces
            | EdgeKind::Realizes
    )
}

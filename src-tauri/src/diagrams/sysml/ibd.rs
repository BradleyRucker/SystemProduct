/// Internal Block Diagram rules.
/// IBDs show the internal structure of a single block —
/// its parts (composed blocks) and port connections.
use crate::core::model::{EdgeKind, NodeKind};

pub fn node_allowed(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Block | NodeKind::Port | NodeKind::Interface)
}

pub fn edge_allowed(kind: &EdgeKind) -> bool {
    matches!(kind, EdgeKind::Connects | EdgeKind::Composes)
}

/// SysML diagram-type-specific logic.
/// Each submodule knows which node/edge kinds are valid for that diagram
/// and provides default sizing for those element types.
use crate::core::model::NodeKind;

pub mod bdd;
pub mod ibd;

/// Default canvas size for a node kind in a given diagram context.
pub fn default_size(kind: &NodeKind) -> (f64, f64) {
    match kind {
        NodeKind::Block | NodeKind::Interface => (160.0, 80.0),
        NodeKind::Requirement => (200.0, 90.0),
        NodeKind::Port => (20.0, 20.0),
        NodeKind::UseCase => (140.0, 60.0),
        NodeKind::Actor => (60.0, 80.0),
        NodeKind::TestCase => (160.0, 70.0),
        NodeKind::Stakeholder => (120.0, 60.0),
        NodeKind::Function => (140.0, 60.0),
        NodeKind::External => (180.0, 70.0),
        NodeKind::ValueType => (160.0, 72.0),
        NodeKind::ConstraintBlock => (200.0, 90.0),
        NodeKind::State => (160.0, 72.0),
    }
}

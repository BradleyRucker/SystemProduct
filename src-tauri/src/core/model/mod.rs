use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

// ── Node ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: NodeKind,
    pub name: String,
    pub description: String,

    /// Kind-specific data. Kept typed so validation and queries stay sane.
    pub data: NodeData,

    /// Escape hatch for user-defined or AI-generated metadata.
    /// Nothing queryable should live here.
    pub meta: HashMap<String, Value>,

    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Requirement,
    Block,
    Interface,
    Port,
    UseCase,
    Actor,
    TestCase,
    Stakeholder,
    Function,
    External,
    ValueType,
    ConstraintBlock,
    State,
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NodeKind::Requirement => "requirement",
            NodeKind::Block => "block",
            NodeKind::Interface => "interface",
            NodeKind::Port => "port",
            NodeKind::UseCase => "use_case",
            NodeKind::Actor => "actor",
            NodeKind::TestCase => "test_case",
            NodeKind::Stakeholder => "stakeholder",
            NodeKind::Function => "function",
            NodeKind::External => "external",
            NodeKind::ValueType => "value_type",
            NodeKind::ConstraintBlock => "constraint_block",
            NodeKind::State => "state",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeData {
    Requirement(RequirementData),
    Block(BlockData),
    Interface,
    Port(PortData),
    UseCase(UseCaseData),
    Actor,
    TestCase(TestCaseData),
    Stakeholder,
    Function,
    External,
    ValueType(ValueTypeData),
    ConstraintBlock(ConstraintBlockData),
    State(StateData),
}

// ── Kind-specific data structs ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequirementData {
    /// Human-readable identifier, e.g. "REQ-001"
    pub req_id: Option<String>,
    pub text: Option<String>,
    pub rationale: Option<String>,
    pub priority: RequirementPriority,
    pub status: RequirementStatus,
    pub source: Option<String>,
    /// Subsystem allocation tags (e.g. ["FPGA", "Microcontroller"])
    pub allocations: Option<Vec<String>>,
    pub verification_method: Option<VerificationMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RequirementSnapshot {
    pub req_id: String,
    pub name: String,
    pub text: String,
    pub rationale: String,
    pub priority: String,
    pub status: String,
    pub verification_method: String,
    pub source: String,
    pub allocations: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementHistoryEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub node_id: Uuid,
    pub ts: DateTime<Utc>,
    pub actor: String,
    pub source: String,
    pub prev: RequirementSnapshot,
    pub next: RequirementSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequirementPriority {
    Shall,
    #[default]
    Should,
    May,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStatus {
    #[default]
    Draft,
    Approved,
    Obsolete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    Analysis,
    Test,
    Inspection,
    Demonstration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlockData {
    pub is_abstract: bool,
    pub multiplicity: Option<String>,
    pub sim_params: Option<SimParams>,
    pub sim_script: Option<String>,
}

// ── Simulation types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SimParams {
    pub processing_time_ms: Option<f64>,
    pub failure_rate: Option<f64>,
    pub queue_capacity: Option<u32>,
    pub throughput_per_sec: Option<f64>,
    pub input_signal_type: Option<String>,
    pub output_signal_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationScenarioEvent {
    pub time_ms: f64,
    pub block_id: Uuid,
    pub signal_type: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationScenario {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub duration_ms: i64,
    pub events: Vec<SimulationScenarioEvent>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub id: Uuid,
    pub scenario_id: Uuid,
    pub ran_at: DateTime<Utc>,
    pub status: String,
    pub metrics: Value,
    pub timeline: Value,
    pub errors: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PortData {
    pub direction: PortDirection,
    /// UUID of the type block (soft reference — not a FK).
    pub type_ref: Option<Uuid>,
    /// Human-readable type name (e.g. "Voltage", "Real", "Integer").
    pub type_name: Option<String>,
    /// SysML multiplicity string (e.g. "1", "0..*", "1..n").
    pub multiplicity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PortDirection {
    In,
    Out,
    #[default]
    InOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UseCaseData {
    pub level: UseCaseLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UseCaseLevel {
    Summary,
    #[default]
    User,
    Subfunction,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TestCaseData {
    pub procedure: Option<String>,
    pub expected: Option<String>,
    pub status: TestStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    #[default]
    NotRun,
    Pass,
    Fail,
}

/// SysML ValueType — wraps a primitive with units/constraints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValueTypeData {
    /// Base type string: "Real", "Integer", "Boolean", "String", etc.
    pub base_type: Option<String>,
    /// Unit name, e.g. "m/s", "kg", "V", "A".
    pub unit: Option<String>,
    /// Optional constraint expression, e.g. "0.0 <= x <= 100.0".
    pub constraint: Option<String>,
}

/// SysML ConstraintBlock — used in Parametric diagrams.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConstraintBlockData {
    /// The mathematical / logical expression constraining the system.
    pub expression: Option<String>,
    /// Parameter names bound to port-like value properties.
    pub parameters: Option<Vec<String>>,
}

/// State in a state machine diagram.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateData {
    /// "initial", "final", "choice", "fork", "join", or "" for normal state.
    pub pseudo_kind: Option<String>,
    /// Entry action (short text or OCL expression).
    pub entry_action: Option<String>,
    /// Exit action.
    pub exit_action: Option<String>,
    /// Do-activity.
    pub do_activity: Option<String>,
}

// ── Edge ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: EdgeKind,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub label: String,
    pub meta: HashMap<String, Value>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Block satisfies a Requirement
    Satisfies,
    /// Requirement refines another Requirement
    Refines,
    /// Function allocated to a Block
    Allocates,
    /// Block realizes a UseCase
    Realizes,
    /// Generic traceability link (any → any)
    Traces,
    /// TestCase verifies a Requirement
    Verifies,
    /// Port-to-port flow connection
    Connects,
    /// Block composed within a Block (composition)
    Composes,
    /// Block specializes another Block (inheritance/generalization)
    Specializes,
    /// DocumentSection derives a Requirement (document traceability)
    Derives,
    /// Any node blocks another (schedule dependency, used by timeline)
    Blocks,
    /// State machine transition between States
    Transition,
    /// Parametric binding connector between value properties
    BindingConnector,
}

impl std::fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EdgeKind::Satisfies => "satisfies",
            EdgeKind::Refines => "refines",
            EdgeKind::Allocates => "allocates",
            EdgeKind::Realizes => "realizes",
            EdgeKind::Traces => "traces",
            EdgeKind::Verifies => "verifies",
            EdgeKind::Connects => "connects",
            EdgeKind::Composes => "composes",
            EdgeKind::Specializes => "specializes",
            EdgeKind::Derives => "derives",
            EdgeKind::Blocks => "blocks",
            EdgeKind::Transition => "transition",
            EdgeKind::BindingConnector => "binding_connector",
        };
        write!(f, "{}", s)
    }
}

// ── Diagram types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagram {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: DiagramKind,
    pub name: String,
    pub description: String,
    pub layout_options: HashMap<String, Value>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagramKind {
    Bdd,
    Ibd,
    UseCase,
    Sequence,
    StateMachine,
    Parametric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramElement {
    pub id: Uuid,
    pub diagram_id: Uuid,
    pub node_id: Uuid,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub collapsed: bool,
    pub style_overrides: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramEdgeRoute {
    pub id: Uuid,
    pub diagram_id: Uuid,
    pub edge_id: Uuid,
    pub waypoints: Vec<Point>,
}

// ── Project ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

// -- Documents + subsystem content -----------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub doc_type: String,
    pub size: i64,
    pub added_at: DateTime<Utc>,
    pub text: String,
    #[serde(default)]
    pub source_base64: Option<String>,
    #[serde(default)]
    pub source_mime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemKnowledgePage {
    pub id: Uuid,
    pub subsystem_id: Uuid,
    pub title: String,
    pub body: String,
    #[serde(default = "default_subsystem_knowledge_body_format")]
    pub body_format: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_subsystem_knowledge_body_format() -> String {
    "plain".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemArtifact {
    pub id: Uuid,
    pub subsystem_id: Uuid,
    pub kind: String,
    pub title: String,
    pub link: String,
    pub notes: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemActivity {
    pub id: Uuid,
    pub subsystem_id: Uuid,
    pub text: String,
    pub created_at: DateTime<Utc>,
}

// ── Document sections ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SectionType {
    Heading,
    Paragraph,
    Requirement,
    BomItem,
    BoeLine,
    SowSection,
    IcdInterface,
    ListItem,
}

impl Default for SectionType {
    fn default() -> Self {
        SectionType::Paragraph
    }
}

impl std::fmt::Display for SectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SectionType::Heading => "heading",
            SectionType::Paragraph => "paragraph",
            SectionType::Requirement => "requirement",
            SectionType::BomItem => "bom_item",
            SectionType::BoeLine => "boe_line",
            SectionType::SowSection => "sow_section",
            SectionType::IcdInterface => "icd_interface",
            SectionType::ListItem => "list_item",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for SectionType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "heading" => Ok(SectionType::Heading),
            "paragraph" => Ok(SectionType::Paragraph),
            "requirement" => Ok(SectionType::Requirement),
            "bom_item" => Ok(SectionType::BomItem),
            "boe_line" => Ok(SectionType::BoeLine),
            "sow_section" => Ok(SectionType::SowSection),
            "icd_interface" => Ok(SectionType::IcdInterface),
            "list_item" => Ok(SectionType::ListItem),
            _ => Ok(SectionType::Paragraph),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSection {
    pub id: Uuid,
    pub document_id: Uuid,
    pub project_id: Uuid,
    pub section_ref: String,
    pub section_type: SectionType,
    pub title: String,
    pub body: String,
    pub part_number: Option<String>,
    pub quantity: Option<String>,
    pub unit: Option<String>,
    pub position: i64,
    pub created_at: DateTime<Utc>,
}

// ── Suspect links ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspectLink {
    pub id: Uuid,
    pub project_id: Uuid,
    pub edge_id: Uuid,
    pub source_node_id: Uuid,
    pub target_node_id: Uuid,
    pub flagged_at: chrono::DateTime<Utc>,
    pub flagged_reason: String,
    pub resolved_at: Option<chrono::DateTime<Utc>>,
    pub resolved_by: Option<String>,
}

// ── Review workflow ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Open,
    InProgress,
    Approved,
    Rejected,
    Closed,
}

impl std::fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewStatus::Open => write!(f, "open"),
            ReviewStatus::InProgress => write!(f, "in_progress"),
            ReviewStatus::Approved => write!(f, "approved"),
            ReviewStatus::Rejected => write!(f, "rejected"),
            ReviewStatus::Closed => write!(f, "closed"),
        }
    }
}

impl std::str::FromStr for ReviewStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(ReviewStatus::Open),
            "in_progress" => Ok(ReviewStatus::InProgress),
            "approved" => Ok(ReviewStatus::Approved),
            "rejected" => Ok(ReviewStatus::Rejected),
            "closed" => Ok(ReviewStatus::Closed),
            _ => Ok(ReviewStatus::Open),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSession {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: ReviewStatus,
    pub created_by: String,
    pub created_at: chrono::DateTime<Utc>,
    pub closed_at: Option<chrono::DateTime<Utc>>,
    pub items: Vec<ReviewItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: Uuid,
    pub session_id: Uuid,
    pub node_id: Uuid,
    pub verdict: Option<String>,   // "approved" | "rejected" | "needs_changes"
    pub verdict_by: Option<String>,
    pub verdict_at: Option<chrono::DateTime<Utc>>,
    pub verdict_note: Option<String>,
}

// ── Inline comments ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqComment {
    pub id: Uuid,
    pub project_id: Uuid,
    pub node_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub author: String,
    pub body: String,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    pub resolved_at: Option<chrono::DateTime<Utc>>,
    pub resolved_by: Option<String>,
}

// ── Model baselines ───────────────────────────────────────────────────────────

/// A named snapshot of the full model state at a point in time.
/// `snapshot` is a JSON object: `{ "nodes": [...], "edges": [...] }`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBaseline {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub created_by: String,
    pub created_at: chrono::DateTime<Utc>,
    pub snapshot: serde_json::Value,
}

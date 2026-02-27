/**
 * TypeScript mirror of the Rust model types.
 * Must stay in sync with src-tauri/src/core/model/mod.rs
 */

export type NodeKind =
  | "requirement"
  | "block"
  | "interface"
  | "port"
  | "use_case"
  | "actor"
  | "test_case"
  | "stakeholder"
  | "function"
  | "external"
  | "value_type"
  | "constraint_block"
  | "state";

export type EdgeKind =
  | "satisfies"
  | "refines"
  | "allocates"
  | "realizes"
  | "traces"
  | "verifies"
  | "connects"
  | "composes"
  | "specializes"
  | "derives"
  | "blocks"
  | "transition"
  | "binding_connector";

export type DiagramKind =
  | "bdd"
  | "ibd"
  | "usecase"
  | "sequence"
  | "statemachine"
  | "parametric";

// ── Kind-specific data ────────────────────────────────────────────────────────

export interface RequirementData {
  kind: "requirement";
  req_id?: string;
  text?: string;
  rationale?: string;
  priority: "shall" | "should" | "may";
  status: "draft" | "approved" | "obsolete";
  source?: string;
  allocations?: string[];
  verification_method?: "analysis" | "test" | "inspection" | "demonstration";
}

export interface BlockData {
  kind: "block";
  is_abstract: boolean;
  multiplicity?: string;
  sim_params?: SimParams;
  sim_script?: string;
}

export interface PortData {
  kind: "port";
  direction: "in" | "out" | "inout";
  type_ref?: string;
  type_name?: string;
  multiplicity?: string;
}

export interface UseCaseData {
  kind: "use_case";
  level: "summary" | "user" | "subfunction";
}

export interface TestCaseData {
  kind: "test_case";
  procedure?: string;
  expected?: string;
  status: "not_run" | "pass" | "fail";
}

export interface ValueTypeData {
  kind: "value_type";
  base_type?: "Real" | "Integer" | "Boolean" | "String" | string;
  unit?: string;
  constraint?: string;
}

export interface ConstraintBlockData {
  kind: "constraint_block";
  expression?: string;
  parameters?: string[];
}

export interface StateData {
  kind: "state";
  pseudo_kind?: "initial" | "final" | "choice" | "fork" | "join" | string;
  entry_action?: string;
  exit_action?: string;
  do_activity?: string;
}

export type NodeData =
  | RequirementData
  | BlockData
  | PortData
  | UseCaseData
  | TestCaseData
  | ValueTypeData
  | ConstraintBlockData
  | StateData
  | { kind: "interface" | "actor" | "stakeholder" | "function" | "external" };

// ── Core model types ──────────────────────────────────────────────────────────

export interface Node {
  id: string;
  project_id: string;
  kind: NodeKind;
  name: string;
  description: string;
  data: NodeData;
  meta: Record<string, unknown>;
  created_at: string;
  modified_at: string;
}

export interface Edge {
  id: string;
  project_id: string;
  kind: EdgeKind;
  source_id: string;
  target_id: string;
  label: string;
  meta: Record<string, unknown>;
  created_at: string;
  modified_at: string;
}

export interface Project {
  id: string;
  name: string;
  description: string;
  created_at: string;
  modified_at: string;
}

export interface Document {
  id: string;
  project_id: string;
  name: string;
  doc_type: string;
  size: number;
  added_at: string;
  text: string;
  source_base64?: string;
  source_mime?: string;
}

export type SectionType =
  | "heading"
  | "paragraph"
  | "requirement"
  | "bom_item"
  | "boe_line"
  | "sow_section"
  | "icd_interface"
  | "list_item";

export interface DocumentSection {
  id: string;
  document_id: string;
  project_id: string;
  section_ref: string;
  section_type: SectionType;
  title: string;
  body: string;
  part_number?: string;
  quantity?: string;
  unit?: string;
  position: number;
  created_at: string;
}

export interface SubsystemKnowledgePage {
  id: string;
  subsystem_id: string;
  title: string;
  body: string;
  body_format?: KnowledgeBodyFormat;
  created_at: string;
  updated_at: string;
}

export type KnowledgeBodyFormat = "plain" | "markdown" | "rich";

export interface SubsystemArtifact {
  id: string;
  subsystem_id: string;
  kind: string;
  title: string;
  link: string;
  notes: string;
  created_at: string;
}

export interface SubsystemActivity {
  id: string;
  subsystem_id: string;
  text: string;
  created_at: string;
}

export interface Diagram {
  id: string;
  project_id: string;
  kind: DiagramKind;
  name: string;
  description: string;
  layout_options: Record<string, unknown>;
  created_at: string;
  modified_at: string;
}

export interface DiagramElement {
  id: string;
  diagram_id: string;
  node_id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  collapsed: boolean;
  style_overrides: Record<string, unknown>;
}

export interface ValidationIssue {
  id: string;
  severity: "error" | "warning" | "info";
  code: string;
  message: string;
  node_id?: string;
  edge_id?: string;
}

export interface AiSuggestion {
  id: string;
  project_id: string;
  diagram_id?: string;
  kind: "node" | "edge" | "text" | "analysis";
  payload: unknown;
  rationale: string;
  severity?: "info" | "warning" | "error";
  target_node_id?: string;
  target_field?: string;
  created_at: string;
}

// ── Diagram IR (what the canvas renders) ─────────────────────────────────────

export interface IRNode {
  id: string;
  kind: NodeKind;
  name: string;
  description: string;
  data: NodeData;
  x: number;
  y: number;
  width: number;
  height: number;
  collapsed: boolean;
  style_overrides: Record<string, unknown>;
  has_suggestion: boolean;
  // Live traceability data — computed in rebuildIR()
  satisfies_count: number;      // edges where this block is the target of satisfies
  verifies_count: number;       // edges where this test case is source of verifies
  open_comments: number;        // open req_comments for this node
  has_suspect: boolean;         // any unresolved suspect link targets this node
  coverage_status: 'none' | 'partial' | 'full' | 'n/a';  // for blocks: are all satisfying reqs approved?
  linked_req_ids: string[];     // IDs of requirements that satisfy/trace to this node
}

export interface IREdge {
  id: string;
  kind: EdgeKind;
  source_id: string;
  target_id: string;
  label: string;
  waypoints: { x: number; y: number }[];
  has_suggestion: boolean;
}

export interface DiagramIR {
  diagram_id: string;
  kind: DiagramKind;
  name: string;
  nodes: IRNode[];
  edges: IREdge[];
}

export interface SuspectLink {
  id: string;
  project_id: string;
  edge_id: string;
  source_node_id: string;
  target_node_id: string;
  flagged_at: string;
  flagged_reason: string;
  resolved_at?: string;
  resolved_by?: string;
}

export interface ReqComment {
  id: string;
  project_id: string;
  node_id: string;
  parent_id?: string;
  author: string;
  body: string;
  created_at: string;
  updated_at: string;
  resolved_at?: string;
  resolved_by?: string;
}

// ── Review workflow ───────────────────────────────────────────────────────────

export type ReviewStatus = 'open' | 'in_progress' | 'approved' | 'rejected' | 'closed';
export type ReviewVerdict = 'approved' | 'rejected' | 'needs_changes';

export interface ReviewItem {
  id: string;
  session_id: string;
  node_id: string;
  verdict?: ReviewVerdict;
  verdict_by?: string;
  verdict_at?: string;
  verdict_note?: string;
}

export interface ReviewSession {
  id: string;
  project_id: string;
  title: string;
  description?: string;
  status: ReviewStatus;
  created_by: string;
  created_at: string;
  closed_at?: string;
  items: ReviewItem[];
}

// ── Simulation types ──────────────────────────────────────────────────────────

export interface SimParams {
  processing_time_ms?: number;
  failure_rate?: number;
  queue_capacity?: number;
  throughput_per_sec?: number;
  input_signal_type?: string;
  output_signal_type?: string;
}

export interface SimulationScenarioEvent {
  time_ms: number;
  block_id: string;
  signal_type: string;
  value?: unknown;
}

export interface SimulationScenario {
  id: string;
  project_id: string;
  name: string;
  description: string;
  duration_ms: number;
  events: SimulationScenarioEvent[];
  created_at: string;
  modified_at: string;
}

export interface BlockSimMetrics {
  utilization: number;
  queue_depth: number;
  processed_count: number;
  failures: number;
}

export interface SimulationTimelineEvent {
  timestamp_ms: number;
  block_id: string;
  event_type: "stimulus" | "processed" | "failure";
  detail: string;
}

export interface SimulationResult {
  id: string;
  scenario_id: string;
  ran_at: string;
  status: "pending" | "running" | "complete" | "error";
  metrics: Record<string, BlockSimMetrics>;
  timeline: SimulationTimelineEvent[];
  errors: string[];
}

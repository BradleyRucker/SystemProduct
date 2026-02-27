use crate::core::model::{Edge, Node, Project};
use anyhow::Result;
use serde_json::{json, Value};

// ── JSON-LD ───────────────────────────────────────────────────────────────────

pub fn to_json_ld(project: &Project, nodes: &[Node], edges: &[Edge]) -> Result<String> {
    let node_values: Vec<Value> = nodes
        .iter()
        .map(|n| {
            json!({
                "@id": format!("urn:uuid:{}", n.id),
                "@type": format!("sysml:{}", n.kind),
                "name": n.name,
                "description": n.description,
                "data": serde_json::to_value(&n.data).unwrap_or(Value::Null),
            })
        })
        .collect();

    let edge_values: Vec<Value> = edges
        .iter()
        .map(|e| {
            json!({
                "@id": format!("urn:uuid:{}", e.id),
                "@type": format!("sysml:{}", e.kind),
                "source": format!("urn:uuid:{}", e.source_id),
                "target": format!("urn:uuid:{}", e.target_id),
                "label": e.label,
            })
        })
        .collect();

    let doc = json!({
        "@context": {
            "sysml": "https://www.omg.org/spec/SysML/20230201/",
            "name": "http://schema.org/name",
            "description": "http://schema.org/description",
            "source": { "@type": "@id" },
            "target": { "@type": "@id" },
        },
        "@graph": {
            "@id": format!("urn:uuid:{}", project.id),
            "@type": "sysml:Model",
            "name": project.name,
            "description": project.description,
            "elements": node_values,
            "relationships": edge_values,
        }
    });

    Ok(serde_json::to_string_pretty(&doc)?)
}

// ── Markdown ──────────────────────────────────────────────────────────────────

pub fn to_markdown(project: &Project, nodes: &[Node], edges: &[Edge]) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", project.name));

    if !project.description.is_empty() {
        out.push_str(&format!("{}\n\n", project.description));
    }

    // Requirements table
    let reqs: Vec<_> = nodes
        .iter()
        .filter(|n| matches!(n.kind, crate::core::model::NodeKind::Requirement))
        .collect();

    if !reqs.is_empty() {
        out.push_str("## Requirements\n\n");
        out.push_str("| ID | Name | Text | Priority | Status | Verification |\n");
        out.push_str("|---|---|---|---|---|---|\n");

        for node in &reqs {
            if let crate::core::model::NodeData::Requirement(r) = &node.data {
                out.push_str(&format!(
                    "| {} | {} | {} | {:?} | {:?} | {} |\n",
                    r.req_id.as_deref().unwrap_or("-"),
                    node.name,
                    r.text.as_deref().unwrap_or("").replace('|', "\\|"),
                    r.priority,
                    r.status,
                    r.verification_method
                        .as_ref()
                        .map(|v| format!("{v:?}"))
                        .unwrap_or_else(|| "-".to_string()),
                ));
            }
        }
        out.push('\n');
    }

    // Traceability section
    if !edges.is_empty() {
        out.push_str("## Traceability\n\n");
        out.push_str("| Relationship | Source | Target |\n");
        out.push_str("|---|---|---|\n");

        for edge in edges {
            let src_name = nodes
                .iter()
                .find(|n| n.id == edge.source_id)
                .map(|n| n.name.as_str())
                .unwrap_or("?");
            let tgt_name = nodes
                .iter()
                .find(|n| n.id == edge.target_id)
                .map(|n| n.name.as_str())
                .unwrap_or("?");

            out.push_str(&format!(
                "| «{}» | {} | {} |\n",
                edge.kind, src_name, tgt_name
            ));
        }
        out.push('\n');
    }

    out
}

// ── Native JSON (round-trip) ──────────────────────────────────────────────────

pub fn to_native_json(project: &Project, nodes: &[Node], edges: &[Edge]) -> Result<String> {
    let doc = json!({
        "version": 1,
        "project": project,
        "nodes": nodes,
        "edges": edges,
    });
    Ok(serde_json::to_string_pretty(&doc)?)
}

// ── SysML XMI (OMG SysML 1.6 / UML 2.5 subset) ──────────────────────────────
//
// Produces a valid XMI 2.1 document with SysML 1.6 stereotypes.
// Subset implemented: Model, Package, Block, Requirement, Port, ValueType,
// ConstraintBlock, State, and all edge kinds as Dependencies / Associations.

pub fn to_xmi(project: &Project, nodes: &[Node], edges: &[Edge]) -> String {
    let mut out = String::with_capacity(8192);

    out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    out.push('\n');
    out.push_str(r#"<xmi:XMI xmi:version="2.1""#);
    out.push('\n');
    out.push_str(r#"  xmlns:xmi="http://schema.omg.org/spec/XMI/2.1""#);
    out.push('\n');
    out.push_str(r#"  xmlns:uml="http://www.eclipse.org/uml2/5.0.0/UML""#);
    out.push('\n');
    out.push_str(r#"  xmlns:SysML="http://www.eclipse.org/papyrus/2/SysML/1.6""#);
    out.push('\n');
    out.push_str(r#"  xmlns:Blocks="http://www.eclipse.org/papyrus/2/SysML/1.6/Blocks""#);
    out.push('\n');
    out.push_str(r#"  xmlns:Requirements="http://www.eclipse.org/papyrus/2/SysML/1.6/Requirements">"#);
    out.push('\n');

    // ── UML Model element ────────────────────────────────────────────────────
    let model_id = format!("_{}", project.id.to_string().replace('-', ""));
    out.push_str(&format!(
        r#"  <uml:Model xmi:id="{}" name="{}">"#,
        model_id,
        xml_escape(&project.name)
    ));
    out.push('\n');

    // ── Package containing all elements ──────────────────────────────────────
    out.push_str(&format!(
        r#"    <packagedElement xmi:type="uml:Package" xmi:id="{}_pkg" name="{}">"#,
        model_id,
        xml_escape(&project.name)
    ));
    out.push('\n');

    // Emit each node as a packagedElement
    for node in nodes {
        let nid = format!("_{}", node.id.to_string().replace('-', ""));
        let (uml_type, extra_attrs) = node_uml_type(node);

        out.push_str(&format!(
            r#"      <packagedElement xmi:type="{}" xmi:id="{}" name="{}"{}/>"#,
            uml_type,
            nid,
            xml_escape(&node.name),
            extra_attrs,
        ));
        out.push('\n');
    }

    // Emit edges as UML relationships
    for edge in edges {
        let eid = format!("_{}", edge.id.to_string().replace('-', ""));
        let src = format!("_{}", edge.source_id.to_string().replace('-', ""));
        let tgt = format!("_{}", edge.target_id.to_string().replace('-', ""));
        let (rel_type, extra) = edge_uml_type(edge);

        out.push_str(&format!(
            r#"      <packagedElement xmi:type="{}" xmi:id="{}" client="{}" supplier="{}"{}/>"#,
            rel_type, eid, src, tgt, extra,
        ));
        out.push('\n');
    }

    out.push_str("    </packagedElement>\n");
    out.push_str("  </uml:Model>\n");

    // ── SysML stereotype applications ─────────────────────────────────────────
    for node in nodes {
        let nid = format!("_{}", node.id.to_string().replace('-', ""));
        match node.kind {
            crate::core::model::NodeKind::Block => {
                out.push_str(&format!(
                    r#"  <Blocks:Block xmi:id="{}_st" base_Class="{}"/>"#,
                    nid, nid
                ));
                out.push('\n');
            }
            crate::core::model::NodeKind::Requirement => {
                let req_text = if let crate::core::model::NodeData::Requirement(r) = &node.data {
                    r.text.clone().unwrap_or_default()
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    r#"  <Requirements:Requirement xmi:id="{}_st" base_Class="{}" text="{}"/>"#,
                    nid,
                    nid,
                    xml_escape(&req_text)
                ));
                out.push('\n');
            }
            crate::core::model::NodeKind::ValueType => {
                out.push_str(&format!(
                    r#"  <Blocks:ValueType xmi:id="{}_st" base_DataType="{}"/>"#,
                    nid, nid
                ));
                out.push('\n');
            }
            crate::core::model::NodeKind::ConstraintBlock => {
                out.push_str(&format!(
                    r#"  <Blocks:ConstraintBlock xmi:id="{}_st" base_Class="{}"/>"#,
                    nid, nid
                ));
                out.push('\n');
            }
            _ => {}
        }
    }

    out.push_str("</xmi:XMI>\n");
    out
}

fn node_uml_type(node: &Node) -> (&'static str, String) {
    use crate::core::model::NodeKind;
    match node.kind {
        NodeKind::Requirement => ("uml:Class", r#" isAbstract="false""#.to_string()),
        NodeKind::Block => ("uml:Class", String::new()),
        NodeKind::Interface => ("uml:Interface", String::new()),
        NodeKind::Port => ("uml:Port", String::new()),
        NodeKind::UseCase => ("uml:UseCase", String::new()),
        NodeKind::Actor => ("uml:Actor", String::new()),
        NodeKind::TestCase => ("uml:Operation", String::new()),
        NodeKind::Stakeholder => ("uml:Class", String::new()),
        NodeKind::Function => ("uml:Activity", String::new()),
        NodeKind::External => ("uml:Component", String::new()),
        NodeKind::ValueType => ("uml:DataType", String::new()),
        NodeKind::ConstraintBlock => ("uml:Class", r#" isAbstract="false""#.to_string()),
        NodeKind::State => ("uml:State", String::new()),
    }
}

fn edge_uml_type(edge: &Edge) -> (&'static str, String) {
    use crate::core::model::EdgeKind;
    let name_attr = if !edge.label.is_empty() {
        format!(r#" name="{}""#, xml_escape(&edge.label))
    } else {
        String::new()
    };
    match edge.kind {
        EdgeKind::Satisfies | EdgeKind::Realizes | EdgeKind::Traces
        | EdgeKind::Allocates | EdgeKind::Refines | EdgeKind::Derives => {
            ("uml:Abstraction", name_attr)
        }
        EdgeKind::Verifies => ("uml:Dependency", name_attr),
        EdgeKind::Connects => ("uml:AssociationClass", name_attr),
        EdgeKind::Composes => {
            ("uml:Association", format!(r#"{} aggregation="composite""#, name_attr))
        }
        EdgeKind::Specializes => ("uml:Generalization", name_attr),
        EdgeKind::Blocks => ("uml:Dependency", name_attr),
        EdgeKind::Transition => ("uml:Transition", name_attr),
        EdgeKind::BindingConnector => ("uml:Dependency", name_attr),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

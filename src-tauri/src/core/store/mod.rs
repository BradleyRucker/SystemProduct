use crate::core::model::*;
use anyhow::Result;
use chrono::Utc;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions},
    Row,
};
use std::str::FromStr;
use uuid::Uuid;

pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn open(db_path: &str) -> Result<Self> {
        // WAL mode must be set via connect options, not a PRAGMA inside a
        // migration transaction — SQLite rejects that.
        let opts = SqliteConnectOptions::from_str(&format!("sqlite:{db_path}?mode=rwc"))?
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    // ── Projects ──────────────────────────────────────────────────────────────

    pub async fn create_project(&self, project: &Project) -> Result<()> {
        sqlx::query(
            "INSERT INTO projects (id, name, description, created_at, modified_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(project.id.to_string())
        .bind(&project.name)
        .bind(&project.description)
        .bind(project.created_at.to_rfc3339())
        .bind(project.modified_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query(
            "SELECT id, name, description, created_at, modified_at FROM projects
             ORDER BY modified_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_project).collect()
    }

    pub async fn get_project(&self, id: Uuid) -> Result<Option<Project>> {
        let row = sqlx::query(
            "SELECT id, name, description, created_at, modified_at FROM projects WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(row_to_project).transpose()
    }

    pub async fn delete_project(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Nodes ─────────────────────────────────────────────────────────────────

    pub async fn upsert_node(&self, node: &Node) -> Result<()> {
        let prev_requirement_snapshot = if node.kind == NodeKind::Requirement {
            let row = sqlx::query(
                "SELECT name, description, req_id, req_text, req_rationale, req_priority,
                        req_status, req_source, req_allocations, req_verification_method
                 FROM nodes
                 WHERE id = ? AND kind = 'requirement'",
            )
            .bind(node.id.to_string())
            .fetch_optional(&self.pool)
            .await?;

            row.as_ref().map(row_to_requirement_snapshot).transpose()?
        } else {
            None
        };

        let next_requirement_snapshot = requirement_snapshot_from_node(node);

        // Flatten kind-specific data for column storage
        let (
            req_id,
            req_text,
            req_rationale,
            req_priority,
            req_status,
            req_source,
            req_allocations,
            req_verif,
            block_abstract,
            block_mult,
            port_dir,
            port_type,
            port_type_name,
            port_multiplicity,
            uc_level,
            tc_procedure,
            tc_expected,
            tc_status,
            sim_params,
            sim_script,
            vt_base_type,
            vt_unit,
            vt_constraint,
            cb_expression,
            cb_parameters,
            state_pseudo_kind,
            state_entry,
            state_exit,
            state_do,
        ) = flatten_node_data(&node.data);

        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO nodes (
                id, project_id, kind, name, description,
                req_id, req_text, req_rationale, req_priority, req_status,
                req_source, req_allocations, req_verification_method,
                block_is_abstract, block_multiplicity,
                port_direction, port_type_ref, port_type_name, port_multiplicity,
                uc_level,
                tc_procedure, tc_expected, tc_status,
                sim_params, sim_script,
                vt_base_type, vt_unit, vt_constraint,
                cb_expression, cb_parameters,
                state_pseudo_kind, state_entry, state_exit, state_do,
                meta, created_at, modified_at
             ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?,
                ?, ?, ?, ?,
                ?,
                ?, ?, ?,
                ?, ?,
                ?, ?, ?,
                ?, ?,
                ?, ?, ?, ?,
                ?, ?, ?
             )
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                req_id = excluded.req_id,
                req_text = excluded.req_text,
                req_rationale = excluded.req_rationale,
                req_priority = excluded.req_priority,
                req_status = excluded.req_status,
                req_source = excluded.req_source,
                req_allocations = excluded.req_allocations,
                req_verification_method = excluded.req_verification_method,
                block_is_abstract = excluded.block_is_abstract,
                block_multiplicity = excluded.block_multiplicity,
                port_direction = excluded.port_direction,
                port_type_ref = excluded.port_type_ref,
                port_type_name = excluded.port_type_name,
                port_multiplicity = excluded.port_multiplicity,
                uc_level = excluded.uc_level,
                tc_procedure = excluded.tc_procedure,
                tc_expected = excluded.tc_expected,
                tc_status = excluded.tc_status,
                sim_params = excluded.sim_params,
                sim_script = excluded.sim_script,
                vt_base_type = excluded.vt_base_type,
                vt_unit = excluded.vt_unit,
                vt_constraint = excluded.vt_constraint,
                cb_expression = excluded.cb_expression,
                cb_parameters = excluded.cb_parameters,
                state_pseudo_kind = excluded.state_pseudo_kind,
                state_entry = excluded.state_entry,
                state_exit = excluded.state_exit,
                state_do = excluded.state_do,
                meta = excluded.meta,
                modified_at = excluded.modified_at",
        )
        .bind(node.id.to_string())
        .bind(node.project_id.to_string())
        .bind(node.kind.to_string())
        .bind(&node.name)
        .bind(&node.description)
        .bind(req_id)
        .bind(req_text)
        .bind(req_rationale)
        .bind(req_priority)
        .bind(req_status)
        .bind(req_source)
        .bind(req_allocations)
        .bind(req_verif)
        .bind(block_abstract)
        .bind(block_mult)
        .bind(port_dir)
        .bind(port_type)
        .bind(port_type_name)
        .bind(port_multiplicity)
        .bind(uc_level)
        .bind(tc_procedure)
        .bind(tc_expected)
        .bind(tc_status)
        .bind(sim_params)
        .bind(sim_script)
        .bind(vt_base_type)
        .bind(vt_unit)
        .bind(vt_constraint)
        .bind(cb_expression)
        .bind(cb_parameters)
        .bind(state_pseudo_kind)
        .bind(state_entry)
        .bind(state_exit)
        .bind(state_do)
        .bind(serde_json::to_string(&node.meta)?)
        .bind(node.created_at.to_rfc3339())
        .bind(node.modified_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        if let Some(next) = next_requirement_snapshot {
            if prev_requirement_snapshot.as_ref() != Some(&next) {
                let prev = prev_requirement_snapshot.unwrap_or_default();
                sqlx::query(
                    "INSERT INTO requirement_history
                     (id, project_id, node_id, actor, change_source, changed_at, prev_snapshot, next_snapshot)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(Uuid::new_v4().to_string())
                .bind(node.project_id.to_string())
                .bind(node.id.to_string())
                .bind(extract_history_actor(node))
                .bind(extract_history_source(node))
                .bind(node.modified_at.to_rfc3339())
                .bind(serde_json::to_string(&prev)?)
                .bind(serde_json::to_string(&next)?)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        Ok(())
    }

    pub async fn delete_node(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM nodes WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_nodes(&self, project_id: Uuid) -> Result<Vec<Node>> {
        let rows = sqlx::query("SELECT * FROM nodes WHERE project_id = ? ORDER BY created_at")
            .bind(project_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_node).collect()
    }

    pub async fn list_nodes_by_kind(&self, project_id: Uuid, kind: &NodeKind) -> Result<Vec<Node>> {
        let rows = sqlx::query(
            "SELECT * FROM nodes WHERE project_id = ? AND kind = ? ORDER BY created_at",
        )
        .bind(project_id.to_string())
        .bind(kind.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_node).collect()
    }

    // ── Edges ─────────────────────────────────────────────────────────────────

    pub async fn list_requirement_history(
        &self,
        node_id: Uuid,
        limit: usize,
    ) -> Result<Vec<RequirementHistoryEntry>> {
        let rows = sqlx::query(
            "SELECT * FROM requirement_history
             WHERE node_id = ?
             ORDER BY changed_at DESC
             LIMIT ?",
        )
        .bind(node_id.to_string())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_requirement_history).collect()
    }

    pub async fn upsert_edge(&self, edge: &Edge) -> Result<()> {
        sqlx::query(
            "INSERT INTO edges (id, project_id, kind, source_id, target_id, label, meta, created_at, modified_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                label = excluded.label,
                meta = excluded.meta,
                modified_at = excluded.modified_at",
        )
        .bind(edge.id.to_string())
        .bind(edge.project_id.to_string())
        .bind(edge.kind.to_string())
        .bind(edge.source_id.to_string())
        .bind(edge.target_id.to_string())
        .bind(&edge.label)
        .bind(serde_json::to_string(&edge.meta)?)
        .bind(edge.created_at.to_rfc3339())
        .bind(edge.modified_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_edge(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM edges WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn edges_for_node(&self, node_id: Uuid) -> Result<Vec<Edge>> {
        let rows = sqlx::query("SELECT * FROM edges WHERE source_id = ? OR target_id = ?")
            .bind(node_id.to_string())
            .bind(node_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_edge).collect()
    }

    // ── Diagrams ──────────────────────────────────────────────────────────────

    pub async fn upsert_diagram(&self, diagram: &Diagram) -> Result<()> {
        sqlx::query(
            "INSERT INTO diagrams (id, project_id, kind, name, description, layout_options, created_at, modified_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                layout_options = excluded.layout_options,
                modified_at = excluded.modified_at",
        )
        .bind(diagram.id.to_string())
        .bind(diagram.project_id.to_string())
        .bind(diagram_kind_str(&diagram.kind))
        .bind(&diagram.name)
        .bind(&diagram.description)
        .bind(serde_json::to_string(&diagram.layout_options)?)
        .bind(diagram.created_at.to_rfc3339())
        .bind(diagram.modified_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_diagram(&self, diagram_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM diagram_elements WHERE diagram_id = ?")
            .bind(diagram_id.to_string())
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM diagrams WHERE id = ?")
            .bind(diagram_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_diagrams(&self, project_id: Uuid) -> Result<Vec<Diagram>> {
        let rows = sqlx::query("SELECT * FROM diagrams WHERE project_id = ? ORDER BY created_at")
            .bind(project_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_diagram).collect()
    }

    // ── Diagram elements ──────────────────────────────────────────────────────

    pub async fn upsert_diagram_element(&self, el: &DiagramElement) -> Result<()> {
        sqlx::query(
            "INSERT INTO diagram_elements
                (id, diagram_id, node_id, x, y, width, height, collapsed, style_overrides)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(diagram_id, node_id) DO UPDATE SET
                x = excluded.x,
                y = excluded.y,
                width = excluded.width,
                height = excluded.height,
                collapsed = excluded.collapsed,
                style_overrides = excluded.style_overrides",
        )
        .bind(el.id.to_string())
        .bind(el.diagram_id.to_string())
        .bind(el.node_id.to_string())
        .bind(el.x)
        .bind(el.y)
        .bind(el.width)
        .bind(el.height)
        .bind(el.collapsed as i64)
        .bind(serde_json::to_string(&el.style_overrides)?)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn diagram_elements(&self, diagram_id: Uuid) -> Result<Vec<DiagramElement>> {
        let rows = sqlx::query("SELECT * FROM diagram_elements WHERE diagram_id = ?")
            .bind(diagram_id.to_string())
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_diagram_element).collect()
    }

    // -- Documents ----------------------------------------------------------

    pub async fn list_documents(&self, project_id: Uuid) -> Result<Vec<Document>> {
        let rows =
            sqlx::query("SELECT * FROM documents WHERE project_id = ? ORDER BY added_at DESC")
                .bind(project_id.to_string())
                .fetch_all(&self.pool)
                .await?;
        rows.iter().map(row_to_document).collect()
    }

    pub async fn upsert_document(&self, doc: &Document) -> Result<()> {
        sqlx::query(
            "INSERT INTO documents (id, project_id, name, doc_type, size, added_at, text, source_base64, source_mime)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                doc_type = excluded.doc_type,
                size = excluded.size,
                added_at = excluded.added_at,
                text = excluded.text,
                source_base64 = excluded.source_base64,
                source_mime = excluded.source_mime",
        )
        .bind(doc.id.to_string())
        .bind(doc.project_id.to_string())
        .bind(&doc.name)
        .bind(&doc.doc_type)
        .bind(doc.size)
        .bind(doc.added_at.to_rfc3339())
        .bind(&doc.text)
        .bind(&doc.source_base64)
        .bind(&doc.source_mime)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_document(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM documents WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Document sections -------------------------------------------------

    pub async fn upsert_document_section(&self, s: &DocumentSection) -> Result<()> {
        sqlx::query(
            "INSERT INTO document_sections
             (id, document_id, project_id, section_ref, section_type, title, body,
              part_number, quantity, unit, position, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               section_ref  = excluded.section_ref,
               section_type = excluded.section_type,
               title        = excluded.title,
               body         = excluded.body,
               part_number  = excluded.part_number,
               quantity     = excluded.quantity,
               unit         = excluded.unit,
               position     = excluded.position",
        )
        .bind(s.id.to_string())
        .bind(s.document_id.to_string())
        .bind(s.project_id.to_string())
        .bind(&s.section_ref)
        .bind(s.section_type.to_string())
        .bind(&s.title)
        .bind(&s.body)
        .bind(&s.part_number)
        .bind(&s.quantity)
        .bind(&s.unit)
        .bind(s.position)
        .bind(s.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_document_sections(&self, document_id: Uuid) -> Result<Vec<DocumentSection>> {
        let rows =
            sqlx::query("SELECT * FROM document_sections WHERE document_id = ? ORDER BY position")
                .bind(document_id.to_string())
                .fetch_all(&self.pool)
                .await?;
        rows.iter().map(row_to_document_section).collect()
    }

    pub async fn list_project_document_sections(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<DocumentSection>> {
        let rows = sqlx::query(
            "SELECT * FROM document_sections WHERE project_id = ? ORDER BY document_id, position",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_document_section).collect()
    }

    pub async fn delete_document_sections(&self, document_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM document_sections WHERE document_id = ?")
            .bind(document_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_document_section(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM document_sections WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Subsystem knowledge pages -----------------------------------------

    pub async fn list_subsystem_knowledge(
        &self,
        subsystem_id: Uuid,
    ) -> Result<Vec<SubsystemKnowledgePage>> {
        let rows = sqlx::query(
            "SELECT * FROM subsystem_knowledge WHERE subsystem_id = ? ORDER BY updated_at DESC",
        )
        .bind(subsystem_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_subsystem_knowledge).collect()
    }

    pub async fn upsert_subsystem_knowledge(&self, page: &SubsystemKnowledgePage) -> Result<()> {
        let body_format = if page.body_format.trim().is_empty() {
            "plain"
        } else {
            page.body_format.as_str()
        };
        sqlx::query(
            "INSERT INTO subsystem_knowledge (id, subsystem_id, title, body, body_format, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                body_format = excluded.body_format,
                updated_at = excluded.updated_at",
        )
        .bind(page.id.to_string())
        .bind(page.subsystem_id.to_string())
        .bind(&page.title)
        .bind(&page.body)
        .bind(body_format)
        .bind(page.created_at.to_rfc3339())
        .bind(page.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_subsystem_knowledge(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM subsystem_knowledge WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Subsystem artifacts ------------------------------------------------

    pub async fn list_subsystem_artifacts(
        &self,
        subsystem_id: Uuid,
    ) -> Result<Vec<SubsystemArtifact>> {
        let rows = sqlx::query(
            "SELECT * FROM subsystem_artifacts WHERE subsystem_id = ? ORDER BY created_at DESC",
        )
        .bind(subsystem_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_subsystem_artifact).collect()
    }

    pub async fn list_project_artifacts(&self, project_id: Uuid) -> Result<Vec<SubsystemArtifact>> {
        let rows = sqlx::query(
            "SELECT a.* FROM subsystem_artifacts a
             JOIN nodes n ON n.id = a.subsystem_id
             WHERE n.project_id = ?
             ORDER BY a.created_at DESC",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_subsystem_artifact).collect()
    }

    pub async fn upsert_subsystem_artifact(&self, artifact: &SubsystemArtifact) -> Result<()> {
        sqlx::query(
            "INSERT INTO subsystem_artifacts (id, subsystem_id, kind, title, link, notes, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind,
                title = excluded.title,
                link = excluded.link,
                notes = excluded.notes",
        )
        .bind(artifact.id.to_string())
        .bind(artifact.subsystem_id.to_string())
        .bind(&artifact.kind)
        .bind(&artifact.title)
        .bind(&artifact.link)
        .bind(&artifact.notes)
        .bind(artifact.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_subsystem_artifact(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM subsystem_artifacts WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Subsystem activity -------------------------------------------------

    pub async fn list_subsystem_activity(
        &self,
        subsystem_id: Uuid,
    ) -> Result<Vec<SubsystemActivity>> {
        let rows = sqlx::query(
            "SELECT * FROM subsystem_activity WHERE subsystem_id = ? ORDER BY created_at DESC",
        )
        .bind(subsystem_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(row_to_subsystem_activity).collect()
    }

    pub async fn add_subsystem_activity(&self, entry: &SubsystemActivity) -> Result<()> {
        sqlx::query(
            "INSERT INTO subsystem_activity (id, subsystem_id, text, created_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(entry.id.to_string())
        .bind(entry.subsystem_id.to_string())
        .bind(&entry.text)
        .bind(entry.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -- Settings -----------------------------------------------------------

    pub async fn get_setting(&self, key: &str, project_id: Option<Uuid>) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT value FROM settings
             WHERE key = ? AND COALESCE(project_id, '') = COALESCE(?, '')",
        )
        .bind(key)
        .bind(project_id.map(|id| id.to_string()))
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.try_get::<String, _>("value")).transpose()?)
    }

    pub async fn set_setting(
        &self,
        key: &str,
        project_id: Option<Uuid>,
        value: &str,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM settings
             WHERE key = ? AND COALESCE(project_id, '') = COALESCE(?, '')",
        )
        .bind(key)
        .bind(project_id.map(|id| id.to_string()))
        .execute(&self.pool)
        .await?;

        sqlx::query("INSERT INTO settings (key, project_id, value) VALUES (?, ?, ?)")
            .bind(key)
            .bind(project_id.map(|id| id.to_string()))
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Suspect links -------------------------------------------------------

    pub async fn flag_suspect_links(&self, project_id: Uuid, node_id: Uuid, changed_fields: &str) -> Result<()> {
        // Find all edges where this node is the source, with kinds that create derivation chains
        let rows = sqlx::query(
            "SELECT id, target_id FROM edges WHERE project_id = ? AND source_id = ? AND kind IN ('derives','refines','traces','satisfies')"
        )
        .bind(project_id.to_string())
        .bind(node_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            let edge_id: String = row.get("id");
            let target_id: String = row.get("target_id");
            let suspect_id = Uuid::new_v4();
            // Only insert if no unresolved suspect already exists for this edge
            sqlx::query(
                "INSERT INTO suspect_links (id, project_id, edge_id, source_node_id, target_node_id, flagged_at, flagged_reason)
                 SELECT ?, ?, ?, ?, ?, ?, ?
                 WHERE NOT EXISTS (
                     SELECT 1 FROM suspect_links WHERE edge_id = ? AND resolved_at IS NULL
                 )"
            )
            .bind(suspect_id.to_string())
            .bind(project_id.to_string())
            .bind(&edge_id)
            .bind(node_id.to_string())
            .bind(target_id)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(changed_fields)
            .bind(&edge_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn get_suspect_links(&self, project_id: Uuid) -> Result<Vec<SuspectLink>> {
        let rows = sqlx::query(
            "SELECT id, project_id, edge_id, source_node_id, target_node_id, flagged_at, flagged_reason, resolved_at, resolved_by
             FROM suspect_links WHERE project_id = ? AND resolved_at IS NULL ORDER BY flagged_at DESC"
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(|row| {
            Ok(SuspectLink {
                id: Uuid::parse_str(row.get("id"))?,
                project_id: Uuid::parse_str(row.get("project_id"))?,
                edge_id: Uuid::parse_str(row.get("edge_id"))?,
                source_node_id: Uuid::parse_str(row.get("source_node_id"))?,
                target_node_id: Uuid::parse_str(row.get("target_node_id"))?,
                flagged_at: chrono::DateTime::parse_from_rfc3339(row.get("flagged_at"))?.with_timezone(&chrono::Utc),
                flagged_reason: row.get("flagged_reason"),
                resolved_at: row.get::<Option<String>, _>("resolved_at")
                    .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)))
                    .transpose()?,
                resolved_by: row.get("resolved_by"),
            })
        }).collect()
    }

    pub async fn resolve_suspect_link(&self, id: Uuid, resolved_by: &str) -> Result<()> {
        sqlx::query("UPDATE suspect_links SET resolved_at = ?, resolved_by = ? WHERE id = ?")
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(resolved_by)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Inline comments ---------------------------------------------------

    pub async fn add_req_comment(&self, project_id: Uuid, node_id: Uuid, parent_id: Option<Uuid>, author: &str, body: &str) -> Result<ReqComment> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO req_comments (id, project_id, node_id, parent_id, author, body, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(node_id.to_string())
        .bind(parent_id.map(|u| u.to_string()))
        .bind(author)
        .bind(body)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(ReqComment {
            id,
            project_id,
            node_id,
            parent_id,
            author: author.to_string(),
            body: body.to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            resolved_at: None,
            resolved_by: None,
        })
    }

    pub async fn get_req_comments(&self, node_id: Uuid) -> Result<Vec<ReqComment>> {
        let rows = sqlx::query(
            "SELECT id, project_id, node_id, parent_id, author, body, created_at, updated_at, resolved_at, resolved_by
             FROM req_comments WHERE node_id = ? ORDER BY created_at ASC"
        )
        .bind(node_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(|row| {
            Ok(ReqComment {
                id: Uuid::parse_str(row.get("id"))?,
                project_id: Uuid::parse_str(row.get("project_id"))?,
                node_id: Uuid::parse_str(row.get("node_id"))?,
                parent_id: row.get::<Option<String>, _>("parent_id")
                    .map(|s| Uuid::parse_str(&s))
                    .transpose()?,
                author: row.get("author"),
                body: row.get("body"),
                created_at: chrono::DateTime::parse_from_rfc3339(row.get("created_at"))?.with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(row.get("updated_at"))?.with_timezone(&chrono::Utc),
                resolved_at: row.get::<Option<String>, _>("resolved_at")
                    .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)))
                    .transpose()?,
                resolved_by: row.get("resolved_by"),
            })
        }).collect()
    }

    pub async fn get_comment_counts_for_project(&self, project_id: Uuid) -> Result<std::collections::HashMap<String, i64>> {
        let rows = sqlx::query(
            "SELECT node_id, COUNT(*) as cnt FROM req_comments WHERE project_id = ? AND resolved_at IS NULL GROUP BY node_id"
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut map = std::collections::HashMap::new();
        for row in rows {
            let node_id: String = row.get("node_id");
            let cnt: i64 = row.get("cnt");
            map.insert(node_id, cnt);
        }
        Ok(map)
    }

    pub async fn resolve_req_comment(&self, id: Uuid, resolved_by: &str) -> Result<()> {
        sqlx::query("UPDATE req_comments SET resolved_at = ?, resolved_by = ? WHERE id = ?")
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(resolved_by)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_req_comment(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM req_comments WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Review sessions ---------------------------------------------------

    pub async fn create_review_session(&self, project_id: Uuid, title: &str, description: Option<&str>, node_ids: Vec<Uuid>) -> Result<ReviewSession> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO review_sessions (id, project_id, title, description, status, created_by, created_at) VALUES (?, ?, ?, ?, 'open', 'User', ?)"
        )
        .bind(id.to_string())
        .bind(project_id.to_string())
        .bind(title)
        .bind(description)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let mut items = Vec::new();
        for node_id in &node_ids {
            let item_id = Uuid::new_v4();
            sqlx::query("INSERT INTO review_items (id, session_id, node_id) VALUES (?, ?, ?)")
                .bind(item_id.to_string())
                .bind(id.to_string())
                .bind(node_id.to_string())
                .execute(&self.pool)
                .await?;
            items.push(ReviewItem {
                id: item_id,
                session_id: id,
                node_id: *node_id,
                verdict: None,
                verdict_by: None,
                verdict_at: None,
                verdict_note: None,
            });
        }

        Ok(ReviewSession {
            id,
            project_id,
            title: title.to_string(),
            description: description.map(|s| s.to_string()),
            status: ReviewStatus::Open,
            created_by: "User".to_string(),
            created_at: Utc::now(),
            closed_at: None,
            items,
        })
    }

    pub async fn list_review_sessions(&self, project_id: Uuid) -> Result<Vec<ReviewSession>> {
        let rows = sqlx::query(
            "SELECT id, project_id, title, description, status, created_by, created_at, closed_at FROM review_sessions WHERE project_id = ? ORDER BY created_at DESC"
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut sessions = Vec::new();
        for row in &rows {
            let session_id: String = row.get("id");
            let item_rows = sqlx::query(
                "SELECT id, session_id, node_id, verdict, verdict_by, verdict_at, verdict_note FROM review_items WHERE session_id = ?"
            )
            .bind(&session_id)
            .fetch_all(&self.pool)
            .await?;

            let items: Vec<ReviewItem> = item_rows.iter().map(|r| {
                Ok(ReviewItem {
                    id: Uuid::parse_str(r.get("id"))?,
                    session_id: Uuid::parse_str(r.get("session_id"))?,
                    node_id: Uuid::parse_str(r.get("node_id"))?,
                    verdict: r.get("verdict"),
                    verdict_by: r.get("verdict_by"),
                    verdict_at: r.get::<Option<String>, _>("verdict_at")
                        .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&Utc)))
                        .transpose()?,
                    verdict_note: r.get("verdict_note"),
                })
            }).collect::<Result<Vec<_>>>()?;

            sessions.push(ReviewSession {
                id: Uuid::parse_str(&session_id)?,
                project_id: Uuid::parse_str(row.get("project_id"))?,
                title: row.get("title"),
                description: row.get("description"),
                status: row.get::<String, _>("status").parse().unwrap_or(ReviewStatus::Open),
                created_by: row.get("created_by"),
                created_at: chrono::DateTime::parse_from_rfc3339(row.get("created_at"))?.with_timezone(&Utc),
                closed_at: row.get::<Option<String>, _>("closed_at")
                    .map(|s| chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&Utc)))
                    .transpose()?,
                items,
            });
        }
        Ok(sessions)
    }

    pub async fn set_review_verdict(&self, item_id: Uuid, verdict: &str, verdict_by: &str, note: Option<&str>) -> Result<()> {
        sqlx::query(
            "UPDATE review_items SET verdict = ?, verdict_by = ?, verdict_at = ?, verdict_note = ? WHERE id = ?"
        )
        .bind(verdict)
        .bind(verdict_by)
        .bind(Utc::now().to_rfc3339())
        .bind(note)
        .bind(item_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn close_review_session(&self, session_id: Uuid, status: &str) -> Result<()> {
        sqlx::query("UPDATE review_sessions SET status = ?, closed_at = ? WHERE id = ?")
            .bind(status)
            .bind(Utc::now().to_rfc3339())
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Node lookup ───────────────────────────────────────────────────────────

    pub async fn get_node(&self, id: Uuid) -> Result<Option<Node>> {
        let row = sqlx::query("SELECT * FROM nodes WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_node).transpose()
    }

    // ── Simulation scenarios ──────────────────────────────────────────────────

    pub async fn upsert_simulation_scenario(&self, s: &SimulationScenario) -> Result<()> {
        let events_json = serde_json::to_string(&s.events)?;
        sqlx::query(
            "INSERT INTO simulation_scenarios
             (id, project_id, name, description, duration_ms, events, created_at, modified_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 description = excluded.description,
                 duration_ms = excluded.duration_ms,
                 events = excluded.events,
                 modified_at = excluded.modified_at",
        )
        .bind(s.id.to_string())
        .bind(s.project_id.to_string())
        .bind(&s.name)
        .bind(&s.description)
        .bind(s.duration_ms)
        .bind(events_json)
        .bind(s.created_at.to_rfc3339())
        .bind(s.modified_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_simulation_scenarios(&self, project_id: Uuid) -> Result<Vec<SimulationScenario>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, description, duration_ms, events, created_at, modified_at
             FROM simulation_scenarios WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_simulation_scenario).collect()
    }

    pub async fn get_simulation_scenario(&self, id: Uuid) -> Result<Option<SimulationScenario>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, description, duration_ms, events, created_at, modified_at
             FROM simulation_scenarios WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_simulation_scenario).transpose()
    }

    // ── Simulation results ────────────────────────────────────────────────────

    pub async fn insert_simulation_result(&self, r: &SimulationResult) -> Result<()> {
        sqlx::query(
            "INSERT INTO simulation_results (id, scenario_id, ran_at, status, metrics, timeline, errors)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(r.id.to_string())
        .bind(r.scenario_id.to_string())
        .bind(r.ran_at.to_rfc3339())
        .bind(&r.status)
        .bind(serde_json::to_string(&r.metrics)?)
        .bind(serde_json::to_string(&r.timeline)?)
        .bind(serde_json::to_string(&r.errors)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_simulation_result(&self, id: Uuid) -> Result<Option<SimulationResult>> {
        let row = sqlx::query(
            "SELECT id, scenario_id, ran_at, status, metrics, timeline, errors
             FROM simulation_results WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_simulation_result).transpose()
    }

    pub async fn update_simulation_result_status(
        &self,
        id: Uuid,
        status: &str,
        metrics: serde_json::Value,
        timeline: serde_json::Value,
        errors: serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE simulation_results SET status = ?, metrics = ?, timeline = ?, errors = ? WHERE id = ?",
        )
        .bind(status)
        .bind(serde_json::to_string(&metrics)?)
        .bind(serde_json::to_string(&timeline)?)
        .bind(serde_json::to_string(&errors)?)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Model baselines ───────────────────────────────────────────────────────

    pub async fn create_baseline(&self, baseline: &ModelBaseline) -> Result<()> {
        sqlx::query(
            "INSERT INTO model_baselines (id, project_id, name, description, created_by, created_at, snapshot)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(baseline.id.to_string())
        .bind(baseline.project_id.to_string())
        .bind(&baseline.name)
        .bind(&baseline.description)
        .bind(&baseline.created_by)
        .bind(baseline.created_at.to_rfc3339())
        .bind(serde_json::to_string(&baseline.snapshot)?)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_baselines(&self, project_id: Uuid) -> Result<Vec<ModelBaseline>> {
        let rows = sqlx::query(
            "SELECT id, project_id, name, description, created_by, created_at, snapshot
             FROM model_baselines WHERE project_id = ? ORDER BY created_at DESC",
        )
        .bind(project_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_baseline).collect()
    }

    pub async fn get_baseline(&self, id: Uuid) -> Result<Option<ModelBaseline>> {
        let row = sqlx::query(
            "SELECT id, project_id, name, description, created_by, created_at, snapshot
             FROM model_baselines WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_baseline).transpose()
    }

    pub async fn delete_baseline(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM model_baselines WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Row mapping helpers ───────────────────────────────────────────────────────

fn row_to_project(row: &sqlx::sqlite::SqliteRow) -> Result<Project> {
    Ok(Project {
        id: row.try_get::<String, _>("id")?.parse()?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
        modified_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("modified_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
    })
}

fn row_to_node(row: &sqlx::sqlite::SqliteRow) -> Result<Node> {
    let kind_str: String = row.try_get("kind")?;
    let kind = parse_node_kind(&kind_str)?;
    let data = build_node_data(&kind, row)?;
    let meta_str: String = row.try_get("meta")?;

    Ok(Node {
        id: row.try_get::<String, _>("id")?.parse()?,
        project_id: row.try_get::<String, _>("project_id")?.parse()?,
        kind,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        data,
        meta: serde_json::from_str(&meta_str)?,
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
        modified_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("modified_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
    })
}

fn row_to_edge(row: &sqlx::sqlite::SqliteRow) -> Result<Edge> {
    let kind_str: String = row.try_get("kind")?;
    let meta_str: String = row.try_get("meta")?;

    Ok(Edge {
        id: row.try_get::<String, _>("id")?.parse()?,
        project_id: row.try_get::<String, _>("project_id")?.parse()?,
        kind: parse_edge_kind(&kind_str)?,
        source_id: row.try_get::<String, _>("source_id")?.parse()?,
        target_id: row.try_get::<String, _>("target_id")?.parse()?,
        label: row.try_get("label")?,
        meta: serde_json::from_str(&meta_str)?,
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
        modified_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("modified_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
    })
}

fn row_to_diagram(row: &sqlx::sqlite::SqliteRow) -> Result<Diagram> {
    let kind_str: String = row.try_get("kind")?;
    let layout_str: String = row.try_get("layout_options")?;

    Ok(Diagram {
        id: row.try_get::<String, _>("id")?.parse()?,
        project_id: row.try_get::<String, _>("project_id")?.parse()?,
        kind: parse_diagram_kind(&kind_str)?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        layout_options: serde_json::from_str(&layout_str)?,
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
        modified_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("modified_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
    })
}

fn row_to_diagram_element(row: &sqlx::sqlite::SqliteRow) -> Result<DiagramElement> {
    let style_str: String = row.try_get("style_overrides")?;
    Ok(DiagramElement {
        id: row.try_get::<String, _>("id")?.parse()?,
        diagram_id: row.try_get::<String, _>("diagram_id")?.parse()?,
        node_id: row.try_get::<String, _>("node_id")?.parse()?,
        x: row.try_get("x")?,
        y: row.try_get("y")?,
        width: row.try_get("width")?,
        height: row.try_get("height")?,
        collapsed: row.try_get::<i64, _>("collapsed")? != 0,
        style_overrides: serde_json::from_str(&style_str)?,
    })
}

fn row_to_document(row: &sqlx::sqlite::SqliteRow) -> Result<Document> {
    Ok(Document {
        id: row.try_get::<String, _>("id")?.parse()?,
        project_id: row.try_get::<String, _>("project_id")?.parse()?,
        name: row.try_get("name")?,
        doc_type: row.try_get("doc_type")?,
        size: row.try_get::<i64, _>("size")?,
        added_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("added_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
        text: row.try_get("text")?,
        source_base64: row
            .try_get::<Option<String>, _>("source_base64")
            .unwrap_or(None),
        source_mime: row
            .try_get::<Option<String>, _>("source_mime")
            .unwrap_or(None),
    })
}

fn row_to_subsystem_knowledge(row: &sqlx::sqlite::SqliteRow) -> Result<SubsystemKnowledgePage> {
    Ok(SubsystemKnowledgePage {
        id: row.try_get::<String, _>("id")?.parse()?,
        subsystem_id: row.try_get::<String, _>("subsystem_id")?.parse()?,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        body_format: row
            .try_get::<Option<String>, _>("body_format")
            .unwrap_or(None)
            .unwrap_or_else(|| "plain".to_string()),
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
        updated_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("updated_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
    })
}

fn row_to_subsystem_artifact(row: &sqlx::sqlite::SqliteRow) -> Result<SubsystemArtifact> {
    Ok(SubsystemArtifact {
        id: row.try_get::<String, _>("id")?.parse()?,
        subsystem_id: row.try_get::<String, _>("subsystem_id")?.parse()?,
        kind: row.try_get("kind")?,
        title: row.try_get("title")?,
        link: row.try_get("link")?,
        notes: row.try_get("notes")?,
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
    })
}

fn row_to_subsystem_activity(row: &sqlx::sqlite::SqliteRow) -> Result<SubsystemActivity> {
    Ok(SubsystemActivity {
        id: row.try_get::<String, _>("id")?.parse()?,
        subsystem_id: row.try_get::<String, _>("subsystem_id")?.parse()?,
        text: row.try_get("text")?,
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
    })
}

fn row_to_document_section(row: &sqlx::sqlite::SqliteRow) -> Result<DocumentSection> {
    let section_type_str: String = row.try_get("section_type")?;
    let section_type = section_type_str.parse::<SectionType>().unwrap_or_default();
    Ok(DocumentSection {
        id: row.try_get::<String, _>("id")?.parse()?,
        document_id: row.try_get::<String, _>("document_id")?.parse()?,
        project_id: row.try_get::<String, _>("project_id")?.parse()?,
        section_ref: row.try_get("section_ref")?,
        section_type,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        part_number: row.try_get("part_number")?,
        quantity: row.try_get("quantity")?,
        unit: row.try_get("unit")?,
        position: row.try_get("position")?,
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&chrono::Utc),
    })
}

// ── Enum ↔ string helpers ─────────────────────────────────────────────────────

fn row_to_requirement_snapshot(row: &sqlx::sqlite::SqliteRow) -> Result<RequirementSnapshot> {
    Ok(RequirementSnapshot {
        req_id: row
            .try_get::<Option<String>, _>("req_id")?
            .unwrap_or_default(),
        name: row.try_get::<String, _>("name").unwrap_or_default(),
        text: row
            .try_get::<Option<String>, _>("req_text")?
            .unwrap_or_default(),
        rationale: row
            .try_get::<Option<String>, _>("req_rationale")?
            .unwrap_or_default(),
        priority: row
            .try_get::<Option<String>, _>("req_priority")?
            .unwrap_or_default(),
        status: row
            .try_get::<Option<String>, _>("req_status")?
            .unwrap_or_default(),
        verification_method: row
            .try_get::<Option<String>, _>("req_verification_method")?
            .unwrap_or_default(),
        source: row
            .try_get::<Option<String>, _>("req_source")?
            .unwrap_or_default(),
        allocations: row
            .try_get::<Option<String>, _>("req_allocations")?
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
            .unwrap_or_default(),
        description: row.try_get::<String, _>("description").unwrap_or_default(),
    })
}

fn row_to_requirement_history(row: &sqlx::sqlite::SqliteRow) -> Result<RequirementHistoryEntry> {
    let prev_raw: String = row.try_get("prev_snapshot")?;
    let next_raw: String = row.try_get("next_snapshot")?;

    Ok(RequirementHistoryEntry {
        id: row.try_get::<String, _>("id")?.parse()?,
        project_id: row.try_get::<String, _>("project_id")?.parse()?,
        node_id: row.try_get::<String, _>("node_id")?.parse()?,
        ts: chrono::DateTime::parse_from_rfc3339(row.try_get::<String, _>("changed_at")?.as_str())?
            .with_timezone(&chrono::Utc),
        actor: row.try_get("actor")?,
        source: row.try_get("change_source")?,
        prev: serde_json::from_str(&prev_raw)?,
        next: serde_json::from_str(&next_raw)?,
    })
}

fn parse_node_kind(s: &str) -> Result<NodeKind> {
    match s {
        "requirement" => Ok(NodeKind::Requirement),
        "block" => Ok(NodeKind::Block),
        "interface" => Ok(NodeKind::Interface),
        "port" => Ok(NodeKind::Port),
        "use_case" => Ok(NodeKind::UseCase),
        "actor" => Ok(NodeKind::Actor),
        "test_case" => Ok(NodeKind::TestCase),
        "stakeholder" => Ok(NodeKind::Stakeholder),
        "function" => Ok(NodeKind::Function),
        "external" => Ok(NodeKind::External),
        "value_type" => Ok(NodeKind::ValueType),
        "constraint_block" => Ok(NodeKind::ConstraintBlock),
        "state" => Ok(NodeKind::State),
        other => anyhow::bail!("unknown node kind: {other}"),
    }
}

fn parse_edge_kind(s: &str) -> Result<EdgeKind> {
    match s {
        "satisfies" => Ok(EdgeKind::Satisfies),
        "refines" => Ok(EdgeKind::Refines),
        "allocates" => Ok(EdgeKind::Allocates),
        "realizes" => Ok(EdgeKind::Realizes),
        "traces" => Ok(EdgeKind::Traces),
        "verifies" => Ok(EdgeKind::Verifies),
        "connects" => Ok(EdgeKind::Connects),
        "composes" => Ok(EdgeKind::Composes),
        "specializes" => Ok(EdgeKind::Specializes),
        "derives" => Ok(EdgeKind::Derives),
        "blocks" => Ok(EdgeKind::Blocks),
        "transition" => Ok(EdgeKind::Transition),
        "binding_connector" => Ok(EdgeKind::BindingConnector),
        other => anyhow::bail!("unknown edge kind: {other}"),
    }
}

fn parse_diagram_kind(s: &str) -> Result<DiagramKind> {
    match s {
        "bdd" => Ok(DiagramKind::Bdd),
        "ibd" => Ok(DiagramKind::Ibd),
        "usecase" => Ok(DiagramKind::UseCase),
        "sequence" => Ok(DiagramKind::Sequence),
        "statemachine" => Ok(DiagramKind::StateMachine),
        "parametric" => Ok(DiagramKind::Parametric),
        other => anyhow::bail!("unknown diagram kind: {other}"),
    }
}

fn diagram_kind_str(k: &DiagramKind) -> &'static str {
    match k {
        DiagramKind::Bdd => "bdd",
        DiagramKind::Ibd => "ibd",
        DiagramKind::UseCase => "usecase",
        DiagramKind::Sequence => "sequence",
        DiagramKind::StateMachine => "statemachine",
        DiagramKind::Parametric => "parametric",
    }
}

// ── Node data flatten/build ───────────────────────────────────────────────────

fn requirement_snapshot_from_node(node: &Node) -> Option<RequirementSnapshot> {
    let req = match &node.data {
        NodeData::Requirement(r) => r,
        _ => return None,
    };

    Some(RequirementSnapshot {
        req_id: req.req_id.clone().unwrap_or_default(),
        name: node.name.clone(),
        text: req.text.clone().unwrap_or_default(),
        rationale: req.rationale.clone().unwrap_or_default(),
        priority: format!("{:?}", req.priority).to_lowercase(),
        status: format!("{:?}", req.status).to_lowercase(),
        verification_method: req
            .verification_method
            .as_ref()
            .map(|v| format!("{v:?}").to_lowercase())
            .unwrap_or_default(),
        source: req.source.clone().unwrap_or_default(),
        allocations: req.allocations.clone().unwrap_or_default(),
        description: node.description.clone(),
    })
}

fn extract_history_actor(node: &Node) -> String {
    node.meta
        .get("actor")
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("system")
        .to_string()
}

fn extract_history_source(node: &Node) -> String {
    if let Some(source) = node
        .meta
        .get("change_source")
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
    {
        return source.to_string();
    }

    if node
        .meta
        .get("ai_generated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        || node
            .meta
            .get("ai_suggested")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        return "ai".to_string();
    }

    "manual".to_string()
}

#[allow(clippy::type_complexity)]
fn flatten_node_data(
    data: &NodeData,
) -> (
    // req fields (8)
    Option<String>, Option<String>, Option<String>, Option<String>,
    Option<String>, Option<String>, Option<String>, Option<String>,
    // block fields (2)
    Option<i64>, Option<String>,
    // port fields (4)
    Option<String>, Option<String>, Option<String>, Option<String>,
    // uc_level (1)
    Option<String>,
    // tc fields (3)
    Option<String>, Option<String>, Option<String>,
    // sim fields (2)
    Option<String>, Option<String>,
    // value_type fields (3)
    Option<String>, Option<String>, Option<String>,
    // constraint_block fields (2)
    Option<String>, Option<String>,
    // state fields (4)
    Option<String>, Option<String>, Option<String>, Option<String>,
) {
    let none29 = || (
        None, None, None, None, None, None, None, None,
        None, None,
        None, None, None, None,
        None,
        None, None, None,
        None, None,
        None, None, None,
        None, None,
        None, None, None, None,
    );

    match data {
        NodeData::Requirement(r) => (
            r.req_id.clone(),
            r.text.clone(),
            r.rationale.clone(),
            Some(format!("{:?}", r.priority).to_lowercase()),
            Some(format!("{:?}", r.status).to_lowercase()),
            r.source.clone(),
            r.allocations
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())),
            r.verification_method
                .as_ref()
                .map(|v| format!("{v:?}").to_lowercase()),
            None, None, None, None, None, None, None,
            None, None, None,
            None, None,
            None, None, None,
            None, None,
            None, None, None, None,
        ),
        NodeData::Block(b) => (
            None, None, None, None, None, None, None, None,
            Some(b.is_abstract as i64),
            b.multiplicity.clone(),
            None, None, None, None,
            None,
            None, None, None,
            b.sim_params.as_ref().and_then(|p| serde_json::to_string(p).ok()),
            b.sim_script.clone(),
            None, None, None,
            None, None,
            None, None, None, None,
        ),
        NodeData::Port(p) => (
            None, None, None, None, None, None, None, None,
            None, None,
            Some(format!("{:?}", p.direction).to_lowercase()),
            p.type_ref.map(|u| u.to_string()),
            p.type_name.clone(),
            p.multiplicity.clone(),
            None,
            None, None, None,
            None, None,
            None, None, None,
            None, None,
            None, None, None, None,
        ),
        NodeData::UseCase(u) => (
            None, None, None, None, None, None, None, None,
            None, None,
            None, None, None, None,
            Some(format!("{:?}", u.level).to_lowercase()),
            None, None, None,
            None, None,
            None, None, None,
            None, None,
            None, None, None, None,
        ),
        NodeData::TestCase(t) => (
            None, None, None, None, None, None, None, None,
            None, None,
            None, None, None, None,
            None,
            t.procedure.clone(),
            t.expected.clone(),
            Some(format!("{:?}", t.status).to_lowercase()),
            None, None,
            None, None, None,
            None, None,
            None, None, None, None,
        ),
        NodeData::ValueType(v) => (
            None, None, None, None, None, None, None, None,
            None, None,
            None, None, None, None,
            None,
            None, None, None,
            None, None,
            v.base_type.clone(),
            v.unit.clone(),
            v.constraint.clone(),
            None, None,
            None, None, None, None,
        ),
        NodeData::ConstraintBlock(c) => (
            None, None, None, None, None, None, None, None,
            None, None,
            None, None, None, None,
            None,
            None, None, None,
            None, None,
            None, None, None,
            c.expression.clone(),
            c.parameters.as_ref().and_then(|p| serde_json::to_string(p).ok()),
            None, None, None, None,
        ),
        NodeData::State(s) => (
            None, None, None, None, None, None, None, None,
            None, None,
            None, None, None, None,
            None,
            None, None, None,
            None, None,
            None, None, None,
            None, None,
            s.pseudo_kind.clone(),
            s.entry_action.clone(),
            s.exit_action.clone(),
            s.do_activity.clone(),
        ),
        _ => none29(),
    }
}

fn build_node_data(kind: &NodeKind, row: &sqlx::sqlite::SqliteRow) -> Result<NodeData> {
    match kind {
        NodeKind::Requirement => Ok(NodeData::Requirement(RequirementData {
            req_id: row.try_get("req_id")?,
            text: row.try_get("req_text")?,
            rationale: row.try_get("req_rationale")?,
            priority: parse_req_priority(
                row.try_get::<Option<String>, _>("req_priority")?.as_deref(),
            ),
            status: parse_req_status(row.try_get::<Option<String>, _>("req_status")?.as_deref()),
            source: row.try_get("req_source")?,
            allocations: row
                .try_get::<Option<String>, _>("req_allocations")?
                .as_deref()
                .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                .and_then(|list| if list.is_empty() { None } else { Some(list) }),
            verification_method: row
                .try_get::<Option<String>, _>("req_verification_method")?
                .as_deref()
                .map(parse_verification_method)
                .transpose()?,
        })),
        NodeKind::Block => Ok(NodeData::Block(BlockData {
            is_abstract: row
                .try_get::<Option<i64>, _>("block_is_abstract")?
                .unwrap_or(0)
                != 0,
            multiplicity: row.try_get("block_multiplicity")?,
            sim_params: row
                .try_get::<Option<String>, _>("sim_params")?
                .as_deref()
                .and_then(|s| serde_json::from_str::<SimParams>(s).ok()),
            sim_script: row.try_get("sim_script")?,
        })),
        NodeKind::Port => Ok(NodeData::Port(PortData {
            direction: parse_port_direction(
                row.try_get::<Option<String>, _>("port_direction")?
                    .as_deref(),
            ),
            type_ref: row
                .try_get::<Option<String>, _>("port_type_ref")?
                .as_deref()
                .map(|s| s.parse())
                .transpose()?,
            type_name: row.try_get::<Option<String>, _>("port_type_name").unwrap_or(None),
            multiplicity: row.try_get::<Option<String>, _>("port_multiplicity").unwrap_or(None),
        })),
        NodeKind::UseCase => Ok(NodeData::UseCase(UseCaseData {
            level: parse_uc_level(row.try_get::<Option<String>, _>("uc_level")?.as_deref()),
        })),
        NodeKind::TestCase => Ok(NodeData::TestCase(TestCaseData {
            procedure: row.try_get("tc_procedure")?,
            expected: row.try_get("tc_expected")?,
            status: parse_test_status(row.try_get::<Option<String>, _>("tc_status")?.as_deref()),
        })),
        NodeKind::Interface => Ok(NodeData::Interface),
        NodeKind::Actor => Ok(NodeData::Actor),
        NodeKind::Stakeholder => Ok(NodeData::Stakeholder),
        NodeKind::Function => Ok(NodeData::Function),
        NodeKind::External => Ok(NodeData::External),
        NodeKind::ValueType => Ok(NodeData::ValueType(ValueTypeData {
            base_type: row.try_get::<Option<String>, _>("vt_base_type").unwrap_or(None),
            unit: row.try_get::<Option<String>, _>("vt_unit").unwrap_or(None),
            constraint: row.try_get::<Option<String>, _>("vt_constraint").unwrap_or(None),
        })),
        NodeKind::ConstraintBlock => Ok(NodeData::ConstraintBlock(ConstraintBlockData {
            expression: row.try_get::<Option<String>, _>("cb_expression").unwrap_or(None),
            parameters: row
                .try_get::<Option<String>, _>("cb_parameters")
                .unwrap_or(None)
                .as_deref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()),
        })),
        NodeKind::State => Ok(NodeData::State(StateData {
            pseudo_kind: row.try_get::<Option<String>, _>("state_pseudo_kind").unwrap_or(None),
            entry_action: row.try_get::<Option<String>, _>("state_entry").unwrap_or(None),
            exit_action: row.try_get::<Option<String>, _>("state_exit").unwrap_or(None),
            do_activity: row.try_get::<Option<String>, _>("state_do").unwrap_or(None),
        })),
    }
}

fn parse_req_priority(s: Option<&str>) -> RequirementPriority {
    match s {
        Some("shall") => RequirementPriority::Shall,
        Some("may") => RequirementPriority::May,
        _ => RequirementPriority::Should,
    }
}

fn parse_req_status(s: Option<&str>) -> RequirementStatus {
    match s {
        Some("approved") => RequirementStatus::Approved,
        Some("obsolete") => RequirementStatus::Obsolete,
        _ => RequirementStatus::Draft,
    }
}

fn parse_verification_method(s: &str) -> Result<VerificationMethod> {
    match s {
        "analysis" => Ok(VerificationMethod::Analysis),
        "test" => Ok(VerificationMethod::Test),
        "inspection" => Ok(VerificationMethod::Inspection),
        "demonstration" => Ok(VerificationMethod::Demonstration),
        other => anyhow::bail!("unknown verification method: {other}"),
    }
}

fn parse_port_direction(s: Option<&str>) -> PortDirection {
    match s {
        Some("in") => PortDirection::In,
        Some("out") => PortDirection::Out,
        _ => PortDirection::InOut,
    }
}

fn parse_uc_level(s: Option<&str>) -> UseCaseLevel {
    match s {
        Some("summary") => UseCaseLevel::Summary,
        Some("subfunction") => UseCaseLevel::Subfunction,
        _ => UseCaseLevel::User,
    }
}

fn parse_test_status(s: Option<&str>) -> TestStatus {
    match s {
        Some("pass") => TestStatus::Pass,
        Some("fail") => TestStatus::Fail,
        _ => TestStatus::NotRun,
    }
}

fn row_to_simulation_scenario(row: &sqlx::sqlite::SqliteRow) -> Result<SimulationScenario> {
    let events_raw: String = row.try_get("events")?;
    Ok(SimulationScenario {
        id: row.try_get::<String, _>("id")?.parse()?,
        project_id: row.try_get::<String, _>("project_id")?.parse()?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        duration_ms: row.try_get("duration_ms")?,
        events: serde_json::from_str(&events_raw).unwrap_or_default(),
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&Utc),
        modified_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("modified_at")?.as_str(),
        )?
        .with_timezone(&Utc),
    })
}

fn row_to_baseline(row: &sqlx::sqlite::SqliteRow) -> Result<ModelBaseline> {
    let snapshot_raw: String = row.try_get("snapshot")?;
    Ok(ModelBaseline {
        id: Uuid::parse_str(row.try_get::<String, _>("id")?.as_str())?,
        project_id: Uuid::parse_str(row.try_get::<String, _>("project_id")?.as_str())?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        created_by: row.try_get("created_by")?,
        created_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("created_at")?.as_str(),
        )?
        .with_timezone(&Utc),
        snapshot: serde_json::from_str(&snapshot_raw).unwrap_or_default(),
    })
}

fn row_to_simulation_result(row: &sqlx::sqlite::SqliteRow) -> Result<SimulationResult> {
    Ok(SimulationResult {
        id: row.try_get::<String, _>("id")?.parse()?,
        scenario_id: row.try_get::<String, _>("scenario_id")?.parse()?,
        ran_at: chrono::DateTime::parse_from_rfc3339(
            row.try_get::<String, _>("ran_at")?.as_str(),
        )?
        .with_timezone(&Utc),
        status: row.try_get("status")?,
        metrics: serde_json::from_str::<serde_json::Value>(
            row.try_get::<String, _>("metrics")?.as_str(),
        )
        .unwrap_or_default(),
        timeline: serde_json::from_str::<serde_json::Value>(
            row.try_get::<String, _>("timeline")?.as_str(),
        )
        .unwrap_or_default(),
        errors: serde_json::from_str::<serde_json::Value>(
            row.try_get::<String, _>("errors")?.as_str(),
        )
        .unwrap_or_default(),
    })
}

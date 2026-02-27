use anyhow::{Context, Result};
use graphrag_core::GraphRAG;

/// Configuration for GraphRAG context enrichment.
pub struct GraphRagExtractorConfig {
    pub ollama_base_url: String,
    pub ollama_chat_model: String,
    pub ollama_embed_model: String,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
}

impl Default for GraphRagExtractorConfig {
    fn default() -> Self {
        Self {
            ollama_base_url: "http://localhost:11434".to_string(),
            ollama_chat_model: "qwen2.5:7b".to_string(),
            ollama_embed_model: "nomic-embed-text".to_string(),
            chunk_size: 800,
            chunk_overlap: 150,
        }
    }
}

/// Build a knowledge graph from text and return entity/relationship context
/// that can be appended to the extraction prompt.
pub async fn build_requirement_enrichment_context(
    text: &str,
    cfg: &GraphRagExtractorConfig,
) -> Result<String> {
    let base_url = cfg.ollama_base_url.trim_end_matches('/');
    let (host, port) = if let Some(last_colon) = base_url.rfind(':') {
        let after_colon = &base_url[last_colon + 1..];
        if let Ok(parsed_port) = after_colon.parse::<u16>() {
            (base_url[..last_colon].to_string(), parsed_port)
        } else {
            (base_url.to_string(), 11434)
        }
    } else {
        (base_url.to_string(), 11434)
    };

    let mut graphrag = GraphRAG::builder()
        .with_ollama_enabled(true)
        .with_ollama_host(&host)
        .with_ollama_port(port)
        .with_chat_model(&cfg.ollama_chat_model)
        .with_ollama_embedding_model(&cfg.ollama_embed_model)
        .with_chunk_size(cfg.chunk_size)
        .with_chunk_overlap(cfg.chunk_overlap)
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    graphrag.initialize().map_err(|e| anyhow::anyhow!("{e}"))?;

    graphrag
        .add_document_from_text(text)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    graphrag
        .build_graph()
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let kg = graphrag
        .knowledge_graph()
        .context("knowledge graph not available after build")?;

    let entity_lines: Vec<String> = kg
        .entities()
        .take(60)
        .map(|entity| format!("- {} ({})", entity.name, entity.entity_type))
        .collect();

    let rel_lines: Vec<String> = kg
        .relationships()
        .take(40)
        .map(|rel| {
            format!(
                "- {} --[{}]--> {}",
                rel.source.0, rel.relation_type, rel.target.0
            )
        })
        .collect();

    if entity_lines.is_empty() && rel_lines.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();
    if !entity_lines.is_empty() {
        out.push_str("Entity hints from the document graph:\n");
        out.push_str(&entity_lines.join("\n"));
    }

    if !rel_lines.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str("Relationship hints from the document graph:\n");
        out.push_str(&rel_lines.join("\n"));
    }

    Ok(out)
}

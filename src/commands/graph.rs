//! Graph CLI commands for knowledge graph operations.
//!
//! Provides commands for interacting with the knowledge graph:
//! - `entities`: List or search entities
//! - `relationships`: Show relationships for an entity
//! - `stats`: Show graph statistics

use std::error::Error;

use subcog::config::SubcogConfig;
use subcog::models::graph::{Entity, EntityId, EntityQuery, EntityType};
use subcog::services::GraphService;
use subcog::storage::graph::SqliteGraphBackend;

/// Graph action subcommands.
#[derive(clap::Subcommand)]
pub enum GraphAction {
    /// List or search entities in the knowledge graph.
    Entities {
        /// Search query to filter entities by name.
        #[arg(short, long)]
        query: Option<String>,

        /// Filter by entity type (person, organization, technology, concept, file).
        #[arg(short = 't', long)]
        entity_type: Option<String>,

        /// Maximum number of results.
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Output format: table or json.
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Show relationships for an entity.
    Relationships {
        /// Entity ID or name.
        entity: String,

        /// Maximum depth for relationship traversal.
        #[arg(short, long, default_value = "1")]
        depth: u32,

        /// Output format: table or json.
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Show knowledge graph statistics.
    Stats,

    /// Show details for a specific entity.
    Get {
        /// Entity ID or name.
        entity: String,

        /// Output format: table or json.
        #[arg(short, long, default_value = "table")]
        format: String,
    },
}

/// Execute a graph command.
///
/// # Errors
///
/// Returns an error if the graph storage cannot be accessed or the operation fails.
pub fn cmd_graph(config: &SubcogConfig, action: GraphAction) -> Result<(), Box<dyn Error>> {
    let service = open_graph_service(config)?;

    match action {
        GraphAction::Entities {
            query,
            entity_type,
            limit,
            format,
        } => cmd_entities(&service, query, entity_type, limit, &format),
        GraphAction::Relationships {
            entity,
            depth,
            format,
        } => cmd_relationships(&service, &entity, depth, &format),
        GraphAction::Stats => cmd_stats(&service),
        GraphAction::Get { entity, format } => cmd_get_entity(&service, &entity, &format),
    }
}

/// Open the graph service with `SQLite` backend.
fn open_graph_service(
    config: &SubcogConfig,
) -> Result<GraphService<SqliteGraphBackend>, Box<dyn Error>> {
    let db_path = config.data_dir.join("graph.db");
    let backend =
        SqliteGraphBackend::new(&db_path).map_err(|e| format!("Failed to open graph: {e}"))?;
    Ok(GraphService::new(backend))
}

/// List or search entities.
fn cmd_entities(
    service: &GraphService<SqliteGraphBackend>,
    query: Option<String>,
    entity_type: Option<String>,
    limit: usize,
    format: &str,
) -> Result<(), Box<dyn Error>> {
    let entity_type_filter = entity_type
        .as_ref()
        .map(|t| parse_entity_type(t))
        .transpose()?;

    let entities = if let Some(ref q) = query {
        // Search by name
        service.find_by_name(q, entity_type_filter, None, limit)?
    } else if let Some(et) = entity_type_filter {
        // Filter by type only
        service.find_by_type(et, limit)?
    } else {
        // List all entities (use EntityQuery)
        let query = EntityQuery::new().with_limit(limit);
        service.query_entities(&query)?
    };

    if entities.is_empty() {
        println!("No entities found.");
        return Ok(());
    }

    match format {
        "json" => print_entities_json(&entities)?,
        _ => print_entities_table(&entities),
    }

    Ok(())
}

/// Show relationships for an entity.
fn cmd_relationships(
    service: &GraphService<SqliteGraphBackend>,
    entity: &str,
    depth: u32,
    format: &str,
) -> Result<(), Box<dyn Error>> {
    // Try to find entity by ID or name
    let entity_id = resolve_entity(service, entity)?;

    // Get neighbors via traversal
    let traversal = service.traverse(&entity_id, depth, None, None)?;

    if traversal.relationships.is_empty() {
        println!("No relationships found for entity '{entity}'.");
        return Ok(());
    }

    // Build relationship display data
    let mut display_data = Vec::new();
    for rel in &traversal.relationships {
        let source = traversal
            .entities
            .iter()
            .find(|e| e.id == rel.from_entity)
            .map_or_else(|| rel.from_entity.as_str().to_string(), |e| e.name.clone());
        let target = traversal
            .entities
            .iter()
            .find(|e| e.id == rel.to_entity)
            .map_or_else(|| rel.to_entity.as_str().to_string(), |e| e.name.clone());
        display_data.push((source, rel.relationship_type.as_str().to_string(), target));
    }

    match format {
        "json" => print_relationships_json(&display_data)?,
        _ => print_relationships_table(&display_data),
    }

    Ok(())
}

/// Show graph statistics.
fn cmd_stats(service: &GraphService<SqliteGraphBackend>) -> Result<(), Box<dyn Error>> {
    let stats = service.get_stats()?;

    println!("Knowledge Graph Statistics");
    println!("==========================");
    println!();
    println!("Entities:       {:>8}", stats.entity_count);
    println!("Relationships:  {:>8}", stats.relationship_count);
    println!("Mentions:       {:>8}", stats.mention_count);
    println!();

    if !stats.entities_by_type.is_empty() {
        println!("Entities by Type:");
        for (entity_type, count) in &stats.entities_by_type {
            println!("  {entity_type:<15} {count:>6}");
        }
    }

    Ok(())
}

/// Get details for a specific entity.
fn cmd_get_entity(
    service: &GraphService<SqliteGraphBackend>,
    entity: &str,
    format: &str,
) -> Result<(), Box<dyn Error>> {
    let entity_id = resolve_entity(service, entity)?;
    let entity_data = service
        .get_entity(&entity_id)?
        .ok_or_else(|| format!("Entity '{entity}' not found"))?;

    let mentions = service.get_mentions(&entity_id)?;
    let outgoing = service.get_outgoing_relationships(&entity_id)?;
    let incoming = service.get_incoming_relationships(&entity_id)?;
    let total_relationships = outgoing.len() + incoming.len();

    if format == "json" {
        let output = serde_json::json!({
            "entity": {
                "id": entity_data.id.as_str(),
                "name": entity_data.name,
                "entity_type": entity_data.entity_type.as_str(),
                "aliases": entity_data.aliases,
                "properties": entity_data.properties,
                "confidence": entity_data.confidence,
                "mention_count": entity_data.mention_count,
            },
            "mentions_count": mentions.len(),
            "relationships_count": total_relationships,
            "outgoing_relationships": outgoing.len(),
            "incoming_relationships": incoming.len(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("Entity: {}", entity_data.name);
        println!("========{}", "=".repeat(entity_data.name.len()));
        println!();
        println!("ID:          {}", entity_data.id.as_str());
        println!("Type:        {}", entity_data.entity_type.as_str());
        if !entity_data.aliases.is_empty() {
            println!("Aliases:     {}", entity_data.aliases.join(", "));
        }
        println!("Confidence:  {:.0}%", entity_data.confidence * 100.0);
        println!();
        println!("Mentions:          {:>4}", mentions.len());
        println!("Relationships:     {total_relationships:>4}");
        println!("  - Outgoing:      {:>4}", outgoing.len());
        println!("  - Incoming:      {:>4}", incoming.len());
    }

    Ok(())
}

/// Parse entity type from string.
fn parse_entity_type(s: &str) -> Result<EntityType, Box<dyn Error>> {
    match s.to_lowercase().as_str() {
        "person" => Ok(EntityType::Person),
        "organization" | "org" => Ok(EntityType::Organization),
        "technology" | "tech" => Ok(EntityType::Technology),
        "concept" => Ok(EntityType::Concept),
        "file" => Ok(EntityType::File),
        _ => Err(format!(
            "Invalid entity type: '{s}'. Valid types: person, organization, technology, concept, file"
        )
        .into()),
    }
}

/// Resolve an entity by ID or name.
fn resolve_entity(
    service: &GraphService<SqliteGraphBackend>,
    entity: &str,
) -> Result<EntityId, Box<dyn Error>> {
    // First try as an ID
    let entity_id = EntityId::new(entity);
    if service.get_entity(&entity_id)?.is_some() {
        return Ok(entity_id);
    }

    // Try to find by name
    let entities = service.find_by_name(entity, None, None, 1)?;
    entities
        .into_iter()
        .next()
        .map(|e| e.id)
        .ok_or_else(|| format!("Entity '{entity}' not found").into())
}

/// Print entities as a table.
fn print_entities_table(entities: &[Entity]) {
    println!(
        "{:<36}  {:<20}  {:<12}  {:>6}",
        "ID", "NAME", "TYPE", "CONF%"
    );
    println!("{}", "-".repeat(80));
    for entity in entities {
        let name = if entity.name.len() > 20 {
            format!("{}...", &entity.name[..17])
        } else {
            entity.name.clone()
        };
        println!(
            "{:<36}  {:<20}  {:<12}  {:>5.0}%",
            entity.id.as_str(),
            name,
            entity.entity_type.as_str(),
            entity.confidence * 100.0
        );
    }
    println!();
    println!("{} entities", entities.len());
}

/// Print entities as JSON.
fn print_entities_json(entities: &[Entity]) -> Result<(), Box<dyn Error>> {
    let output: Vec<serde_json::Value> = entities
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id.as_str(),
                "name": e.name,
                "entity_type": e.entity_type.as_str(),
                "aliases": e.aliases,
                "confidence": e.confidence,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// Print relationships as a table.
fn print_relationships_table(relationships: &[(String, String, String)]) {
    println!("{:<25}  {:<15}  {:<25}", "SOURCE", "RELATIONSHIP", "TARGET");
    println!("{}", "-".repeat(70));
    for (source, rel_type, target) in relationships {
        let source_name = if source.len() > 25 {
            format!("{}...", &source[..22])
        } else {
            source.clone()
        };
        let target_name = if target.len() > 25 {
            format!("{}...", &target[..22])
        } else {
            target.clone()
        };
        println!("{source_name:<25}  {rel_type:<15}  {target_name:<25}");
    }
    println!();
    println!("{} relationships", relationships.len());
}

/// Print relationships as JSON.
fn print_relationships_json(
    relationships: &[(String, String, String)],
) -> Result<(), Box<dyn Error>> {
    let output: Vec<serde_json::Value> = relationships
        .iter()
        .map(|(source, rel_type, target)| {
            serde_json::json!({
                "source": source,
                "relationship": rel_type,
                "target": target,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entity_type_person() {
        assert!(matches!(
            parse_entity_type("person").unwrap(),
            EntityType::Person
        ));
    }

    #[test]
    fn test_parse_entity_type_org() {
        assert!(matches!(
            parse_entity_type("org").unwrap(),
            EntityType::Organization
        ));
        assert!(matches!(
            parse_entity_type("organization").unwrap(),
            EntityType::Organization
        ));
    }

    #[test]
    fn test_parse_entity_type_tech() {
        assert!(matches!(
            parse_entity_type("tech").unwrap(),
            EntityType::Technology
        ));
        assert!(matches!(
            parse_entity_type("technology").unwrap(),
            EntityType::Technology
        ));
    }

    #[test]
    fn test_parse_entity_type_file() {
        assert!(matches!(
            parse_entity_type("file").unwrap(),
            EntityType::File
        ));
    }

    #[test]
    fn test_parse_entity_type_invalid() {
        assert!(parse_entity_type("invalid").is_err());
    }
}

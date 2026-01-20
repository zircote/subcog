//! Graph tool execution handlers.
//!
//! Implements MCP tool handlers for knowledge graph operations:
//! - Entity CRUD operations
//! - Relationship management
//! - Graph traversal and queries
//! - LLM-powered entity extraction
//! - Entity deduplication
//! - Relationship inference
//! - Graph visualization

use crate::cli::build_llm_provider_for_entity_extraction;
use crate::config::SubcogConfig;
use crate::mcp::tool_types::{
    EntitiesArgs, EntityMergeArgs, ExtractEntitiesArgs, GraphArgs, GraphQueryArgs,
    GraphVisualizeArgs, RelationshipInferArgs, RelationshipsArgs, parse_entity_type,
    parse_relationship_type,
};
use crate::mcp::tools::{ToolContent, ToolResult};
use crate::models::Domain;
use crate::models::graph::{Entity, EntityId, EntityQuery, Relationship, RelationshipQuery};
use crate::services::ServiceContainer;
use crate::{Error, Result};
use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// Entity Operations
// ============================================================================

/// Executes the entities tool (CRUD operations on entities).
///
/// # Arguments
///
/// * `arguments` - JSON arguments containing action and entity parameters
///
/// # Returns
///
/// A tool result with the operation outcome.
///
/// # Errors
///
/// Returns an error if argument parsing or the operation fails.
pub fn execute_entities(arguments: Value) -> Result<ToolResult> {
    let args: EntitiesArgs = serde_json::from_value(arguments)
        .map_err(|e| Error::InvalidInput(format!("Invalid entities arguments: {e}")))?;

    match args.action.as_str() {
        "create" => execute_entity_create(&args),
        "get" => execute_entity_get(&args),
        "list" => execute_entity_list(&args),
        "delete" => execute_entity_delete(&args),
        "extract" => execute_entity_extract(&args),
        "merge" => execute_entity_merge_action(&args),
        _ => Err(Error::InvalidInput(format!(
            "Unknown entity action: {}. Valid actions: create, get, list, delete, extract, merge",
            args.action
        ))),
    }
}

fn execute_entity_create(args: &EntitiesArgs) -> Result<ToolResult> {
    let name = args.name.as_ref().ok_or_else(|| {
        Error::InvalidInput("Entity name is required for create action".to_string())
    })?;

    let entity_type_str = args.entity_type.as_deref().unwrap_or("Concept");
    let entity_type = parse_entity_type(entity_type_str).ok_or_else(|| {
        Error::InvalidInput(format!(
            "Invalid entity type: {entity_type_str}. Valid types: Person, Organization, Technology, Concept, File"
        ))
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let mut entity = Entity::new(entity_type, name, Domain::new());
    if let Some(aliases) = &args.aliases {
        entity.aliases.clone_from(aliases);
    }

    graph.store_entity(&entity)?;

    let text = format!(
        "**Entity Created**\n\n\
         - **ID**: `{}`\n\
         - **Name**: {}\n\
         - **Type**: {:?}\n\
         - **Aliases**: {}\n",
        entity.id,
        entity.name,
        entity.entity_type,
        if entity.aliases.is_empty() {
            "none".to_string()
        } else {
            entity.aliases.join(", ")
        }
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

fn execute_entity_get(args: &EntitiesArgs) -> Result<ToolResult> {
    let entity_id = args
        .entity_id
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("Entity ID is required for get action".to_string()))?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let id = EntityId::new(entity_id);
    match graph.get_entity(&id)? {
        Some(entity) => {
            let relationships = graph.get_outgoing_relationships(&id).unwrap_or_default();

            let text = format!(
                "**Entity: {}**\n\n\
                 - **ID**: `{}`\n\
                 - **Type**: {:?}\n\
                 - **Confidence**: {:.2}\n\
                 - **Aliases**: {}\n\
                 - **Outgoing Relationships**: {}\n",
                entity.name,
                entity.id,
                entity.entity_type,
                entity.confidence,
                if entity.aliases.is_empty() {
                    "none".to_string()
                } else {
                    entity.aliases.join(", ")
                },
                relationships.len()
            );

            Ok(ToolResult {
                content: vec![ToolContent::Text { text }],
                is_error: false,
            })
        },
        None => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Entity not found: {entity_id}"),
            }],
            is_error: true,
        }),
    }
}

fn execute_entity_list(args: &EntitiesArgs) -> Result<ToolResult> {
    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let limit = args.limit.unwrap_or(20);
    let mut query = EntityQuery::new().with_limit(limit);

    if let Some(type_str) = &args.entity_type
        && let Some(entity_type) = parse_entity_type(type_str)
    {
        query = query.with_type(entity_type);
    }

    let entities = graph.query_entities(&query)?;

    if entities.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "No entities found.".to_string(),
            }],
            is_error: false,
        });
    }

    let mut text = format!("**Found {} entities**\n\n", entities.len());
    for entity in &entities {
        text.push_str(&format!(
            "- **{}** ({:?}) `{}`\n",
            entity.name, entity.entity_type, entity.id
        ));
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

fn execute_entity_delete(args: &EntitiesArgs) -> Result<ToolResult> {
    let entity_id = args.entity_id.as_ref().ok_or_else(|| {
        Error::InvalidInput("Entity ID is required for delete action".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let id = EntityId::new(entity_id);
    let deleted = graph.delete_entity(&id)?;

    let text = if deleted {
        format!("Entity `{entity_id}` deleted successfully (including relationships and mentions).")
    } else {
        format!("Entity `{entity_id}` not found.")
    };

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: !deleted,
    })
}

/// Handles the `extract` action for `subcog_entities`.
///
/// Extracts entities from text content using pattern-based extraction.
fn execute_entity_extract(args: &EntitiesArgs) -> Result<ToolResult> {
    let content = args.content.as_ref().ok_or_else(|| {
        Error::InvalidInput("'content' is required for extract action".to_string())
    })?;

    if content.trim().is_empty() {
        return Err(Error::InvalidInput(
            "Content is required for entity extraction".to_string(),
        ));
    }

    let container = ServiceContainer::from_current_dir_or_user()?;

    // Load config and build LLM provider if available
    let config = SubcogConfig::load_default();
    tracing::info!(
        llm_features = config.features.llm_features,
        provider = ?config.llm.provider,
        "execute_entity_extract: loaded config"
    );
    let extractor = if let Some(llm) = build_llm_provider_for_entity_extraction(&config) {
        tracing::info!("execute_entity_extract: using LLM-powered extraction");
        container.entity_extractor_with_llm(llm)
    } else {
        tracing::info!("execute_entity_extract: LLM provider not available, using fallback");
        container.entity_extractor()
    };

    let min_confidence = args.min_confidence.unwrap_or(0.5);
    let extractor = extractor.with_min_confidence(min_confidence);

    let result = extractor.extract(content)?;

    let mut text = format!(
        "**Entity Extraction Results**{}{}\n\n",
        if result.used_fallback {
            " (fallback mode)"
        } else {
            ""
        },
        if let Some(ref memory_id) = args.memory_id {
            format!("\nSource memory: `{memory_id}`")
        } else {
            String::new()
        }
    );

    if result.entities.is_empty() && result.relationships.is_empty() {
        text.push_str("No entities or relationships extracted.\n");
    } else {
        if !result.entities.is_empty() {
            text.push_str(&format!("**Entities ({}):**\n", result.entities.len()));
            for entity in &result.entities {
                text.push_str(&format!(
                    "- **{}** ({}) - confidence: {:.2}\n",
                    entity.name, entity.entity_type, entity.confidence
                ));
            }
            text.push('\n');
        }

        if !result.relationships.is_empty() {
            text.push_str(&format!(
                "**Relationships ({}):**\n",
                result.relationships.len()
            ));
            for rel in &result.relationships {
                text.push_str(&format!(
                    "- {} --[{}]--> {} (confidence: {:.2})\n",
                    rel.from, rel.relationship_type, rel.to, rel.confidence
                ));
            }
        }
    }

    // Store entities if requested (with automatic deduplication)
    if args.store && !result.entities.is_empty() {
        let graph = container.graph()?;
        let graph_entities = extractor.to_graph_entities(&result);

        // Store entities with deduplication and build map with actual IDs
        let mut entity_map: HashMap<String, Entity> = HashMap::new();
        for entity in &graph_entities {
            let actual_id = graph.store_entity_deduped(entity)?;
            // Create entity with the actual ID for relationship mapping
            let mut stored_entity = entity.clone();
            stored_entity.id = actual_id;
            entity_map.insert(entity.name.clone(), stored_entity);
        }

        // Store relationships using the actual entity IDs
        let graph_rels = extractor.to_graph_relationships(&result, &entity_map);

        for rel in &graph_rels {
            graph.store_relationship(rel)?;
        }

        text.push_str(&format!(
            "\n✓ Stored {} entities and {} relationships in graph (with deduplication).\n",
            graph_entities.len(),
            graph_rels.len()
        ));
    }

    if !result.warnings.is_empty() {
        text.push_str("\n**Warnings:**\n");
        for warning in &result.warnings {
            text.push_str(&format!("- {warning}\n"));
        }
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

/// Handles the `merge` action for `subcog_entities`.
///
/// Supports sub-actions: `find_duplicates`, merge.
fn execute_entity_merge_action(args: &EntitiesArgs) -> Result<ToolResult> {
    let merge_action = args.merge_action.as_deref().unwrap_or("find_duplicates");

    match merge_action {
        "find_duplicates" => execute_entity_find_duplicates(args),
        "merge" => execute_entity_merge_impl(args),
        _ => Err(Error::InvalidInput(format!(
            "Unknown merge sub-action: '{merge_action}'. Valid sub-actions: find_duplicates, merge"
        ))),
    }
}

/// Finds duplicate entities for the `merge` action.
fn execute_entity_find_duplicates(args: &EntitiesArgs) -> Result<ToolResult> {
    let entity_id = args.entity_id.as_ref().ok_or_else(|| {
        Error::InvalidInput("'entity_id' is required for find_duplicates".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let id = EntityId::new(entity_id);
    let entity = graph
        .get_entity(&id)?
        .ok_or_else(|| Error::OperationFailed {
            operation: "find_duplicates".to_string(),
            cause: format!("Entity not found: {entity_id}"),
        })?;

    let threshold = args.threshold.unwrap_or(0.7);
    let duplicates = graph.find_duplicates(&entity, threshold)?;

    if duplicates.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "No potential duplicates found for '{}' (threshold: {:.0}%)",
                    entity.name,
                    threshold * 100.0
                ),
            }],
            is_error: false,
        });
    }

    let mut text = format!(
        "**Potential duplicates for '{}'** (threshold: {:.0}%)\n\n",
        entity.name,
        threshold * 100.0
    );
    for dup in &duplicates {
        text.push_str(&format!(
            "- **{}** ({:?}) `{}`\n",
            dup.name, dup.entity_type, dup.id
        ));
    }
    text.push_str(&format!(
        "\nTo merge, use: action=merge, merge_action=merge, entity_ids=[\"{}\", {}], canonical_name=\"...\"",
        entity_id,
        duplicates
            .iter()
            .map(|e| format!("\"{}\"", e.id))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

/// Merges entities for the `merge` action.
fn execute_entity_merge_impl(args: &EntitiesArgs) -> Result<ToolResult> {
    let entity_ids = args.entity_ids.as_ref().ok_or_else(|| {
        Error::InvalidInput("'entity_ids' is required for merge (minimum 2)".to_string())
    })?;

    if entity_ids.len() < 2 {
        return Err(Error::InvalidInput(
            "At least 2 entity IDs are required for merge".to_string(),
        ));
    }

    let canonical_name = args
        .canonical_name
        .as_ref()
        .ok_or_else(|| Error::InvalidInput("'canonical_name' is required for merge".to_string()))?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let ids: Vec<EntityId> = entity_ids.iter().map(EntityId::new).collect();
    let merged = graph.merge_entities(&ids, canonical_name)?;

    let text = format!(
        "**Entities Merged Successfully**\n\n\
         - **Canonical Entity**: {} `{}`\n\
         - **Merged IDs**: {}\n\n\
         All relationships and mentions have been transferred to the canonical entity.",
        merged.name,
        merged.id,
        entity_ids.join(", ")
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

// ============================================================================
// Relationship Operations
// ============================================================================

/// Executes the relationships tool (CRUD operations on relationships).
///
/// # Errors
///
/// Returns an error if argument parsing or the operation fails.
pub fn execute_relationships(arguments: Value) -> Result<ToolResult> {
    let args: RelationshipsArgs = serde_json::from_value(arguments)
        .map_err(|e| Error::InvalidInput(format!("Invalid relationships arguments: {e}")))?;

    match args.action.as_str() {
        "create" => execute_relationship_create(&args),
        "get" | "list" => execute_relationship_list(&args),
        "delete" => execute_relationship_delete(&args),
        "infer" => execute_relationship_infer_action(&args),
        _ => Err(Error::InvalidInput(format!(
            "Unknown relationship action: {}. Valid actions: create, get, list, delete, infer",
            args.action
        ))),
    }
}

fn execute_relationship_create(args: &RelationshipsArgs) -> Result<ToolResult> {
    let from_id = args.from_entity.as_ref().ok_or_else(|| {
        Error::InvalidInput("from_entity is required for create action".to_string())
    })?;
    let to_id = args.to_entity.as_ref().ok_or_else(|| {
        Error::InvalidInput("to_entity is required for create action".to_string())
    })?;
    let rel_type_str = args.relationship_type.as_deref().ok_or_else(|| {
        Error::InvalidInput("relationship_type is required for create action".to_string())
    })?;

    let rel_type = parse_relationship_type(rel_type_str).ok_or_else(|| {
        Error::InvalidInput(format!(
            "Invalid relationship type: {rel_type_str}. Valid types: WorksAt, Created, Uses, Implements, PartOf, RelatesTo, MentionedIn, Supersedes, ConflictsWith"
        ))
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let from = EntityId::new(from_id);
    let to = EntityId::new(to_id);

    let relationship = graph.relate(&from, &to, rel_type)?;

    let text = format!(
        "**Relationship Created**\n\n\
         - **From**: `{}`\n\
         - **To**: `{}`\n\
         - **Type**: {:?}\n",
        relationship.from_entity, relationship.to_entity, relationship.relationship_type
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

fn execute_relationship_list(args: &RelationshipsArgs) -> Result<ToolResult> {
    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let limit = args.limit.unwrap_or(20);
    let direction = args.direction.as_deref().unwrap_or("both");

    let relationships = if let Some(entity_id) = &args.entity_id {
        let id = EntityId::new(entity_id);
        match direction {
            "outgoing" => graph.get_outgoing_relationships(&id)?,
            "incoming" => graph.get_incoming_relationships(&id)?,
            _ => {
                let mut rels = graph.get_outgoing_relationships(&id)?;
                rels.extend(graph.get_incoming_relationships(&id)?);
                rels
            },
        }
    } else {
        // List all relationships with limit
        let query = RelationshipQuery::new().with_limit(limit);
        graph.query_relationships(&query)?
    };

    if relationships.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "No relationships found.".to_string(),
            }],
            is_error: false,
        });
    }

    let display_count = relationships.len().min(limit);
    let mut text = format!("**Found {} relationships**\n\n", relationships.len());
    for rel in relationships.iter().take(display_count) {
        text.push_str(&format!(
            "- `{}` --[{:?}]--> `{}`\n",
            rel.from_entity, rel.relationship_type, rel.to_entity
        ));
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

fn execute_relationship_delete(args: &RelationshipsArgs) -> Result<ToolResult> {
    let from_id = args.from_entity.as_ref().ok_or_else(|| {
        Error::InvalidInput("from_entity is required for delete action".to_string())
    })?;
    let to_id = args.to_entity.as_ref().ok_or_else(|| {
        Error::InvalidInput("to_entity is required for delete action".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let from = EntityId::new(from_id);
    let to = EntityId::new(to_id);

    let mut query = RelationshipQuery::new().from(from).to(to);
    if let Some(rel_type_str) = &args.relationship_type
        && let Some(rel_type) = parse_relationship_type(rel_type_str)
    {
        query = query.with_type(rel_type);
    }

    let deleted = graph.delete_relationships(&query)?;

    let text = format!("Deleted {deleted} relationship(s).");

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

/// Handles the `infer` action for `subcog_relationships`.
///
/// Infers implicit relationships between entities using pattern-based extraction.
fn execute_relationship_infer_action(args: &RelationshipsArgs) -> Result<ToolResult> {
    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    // Get entities to analyze
    let entities = if let Some(entity_ids) = &args.entity_ids {
        let mut entities = Vec::new();
        for id_str in entity_ids {
            let id = EntityId::new(id_str);
            if let Some(entity) = graph.get_entity(&id)? {
                entities.push(entity);
            }
        }
        entities
    } else {
        // Get recent entities
        let limit = args.limit.unwrap_or(50);
        let query = EntityQuery::new().with_limit(limit);
        graph.query_entities(&query)?
    };

    if entities.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "No entities found to analyze.".to_string(),
            }],
            is_error: false,
        });
    }

    // Load config and build LLM provider if available
    let config = SubcogConfig::load_default();
    let extractor = if let Some(llm) = build_llm_provider_for_entity_extraction(&config) {
        container.entity_extractor_with_llm(llm)
    } else {
        container.entity_extractor()
    };

    let min_confidence = args.min_confidence.unwrap_or(0.6);
    let extractor = extractor.with_min_confidence(min_confidence);

    let result = extractor.infer_relationships(&entities)?;

    let mut text = format!(
        "**Relationship Inference Results**{}\n\n",
        if result.used_fallback {
            " (fallback mode)"
        } else {
            ""
        }
    );

    if result.relationships.is_empty() {
        text.push_str("No relationships inferred.\n");
    } else {
        text.push_str(&format!(
            "**Inferred Relationships ({}):**\n",
            result.relationships.len()
        ));
        for rel in &result.relationships {
            text.push_str(&format!(
                "- {} --[{}]--> {} (confidence: {:.2})",
                rel.from, rel.relationship_type, rel.to, rel.confidence
            ));
            if let Some(reasoning) = &rel.reasoning {
                text.push_str(&format!("\n  Reasoning: {reasoning}"));
            }
            text.push('\n');
        }
    }

    // Store relationships if requested
    if args.store && !result.relationships.is_empty() {
        // Build entity map for lookup
        let entity_map: HashMap<String, &Entity> =
            entities.iter().map(|e| (e.name.clone(), e)).collect();

        let mut stored = 0;
        for inferred in &result.relationships {
            if let (Some(from), Some(to)) =
                (entity_map.get(&inferred.from), entity_map.get(&inferred.to))
                && let Some(rel_type) = parse_relationship_type(&inferred.relationship_type)
            {
                let mut rel = Relationship::new(from.id.clone(), to.id.clone(), rel_type);
                rel.confidence = inferred.confidence;
                graph.store_relationship(&rel)?;
                stored += 1;
            }
        }

        text.push_str(&format!("\n✓ Stored {stored} relationships in graph.\n"));
    }

    if !result.warnings.is_empty() {
        text.push_str("\n**Warnings:**\n");
        for warning in &result.warnings {
            text.push_str(&format!("- {warning}\n"));
        }
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

// ============================================================================
// Consolidated Graph Tool
// ============================================================================

/// Executes the consolidated `subcog_graph` tool.
///
/// Combines graph query (neighbors, path, stats) and visualization operations.
///
/// # Errors
///
/// Returns an error if argument parsing or the operation fails.
pub fn execute_graph(arguments: Value) -> Result<ToolResult> {
    let args: GraphArgs = serde_json::from_value(arguments)
        .map_err(|e| Error::InvalidInput(format!("Invalid graph arguments: {e}")))?;

    match args.operation.as_str() {
        "neighbors" => execute_graph_neighbors(&args),
        "path" => execute_graph_path(&args),
        "stats" => execute_graph_stats(),
        "visualize" => execute_graph_visualize_action(&args),
        _ => Err(Error::InvalidInput(format!(
            "Unknown graph operation: {}. Valid operations: neighbors, path, stats, visualize",
            args.operation
        ))),
    }
}

/// Handles the `neighbors` operation for `subcog_graph`.
fn execute_graph_neighbors(args: &GraphArgs) -> Result<ToolResult> {
    let entity_id = args.entity_id.as_ref().ok_or_else(|| {
        Error::InvalidInput("'entity_id' is required for neighbors operation".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let id = EntityId::new(entity_id);
    let depth = u32::try_from(args.depth.unwrap_or(2).min(5)).unwrap_or(2);

    let neighbors = graph.get_neighbors(&id, depth)?;

    if neighbors.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("No neighbors found for entity `{entity_id}` within depth {depth}."),
            }],
            is_error: false,
        });
    }

    let mut text = format!(
        "**Neighbors of `{entity_id}` (depth {depth})**\n\nFound {} entities:\n\n",
        neighbors.len()
    );
    for entity in &neighbors {
        text.push_str(&format!(
            "- **{}** ({:?}) `{}`\n",
            entity.name, entity.entity_type, entity.id
        ));
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

/// Handles the `path` operation for `subcog_graph`.
fn execute_graph_path(args: &GraphArgs) -> Result<ToolResult> {
    let from_id = args.from_entity.as_ref().ok_or_else(|| {
        Error::InvalidInput("'from_entity' is required for path operation".to_string())
    })?;
    let to_id = args.to_entity.as_ref().ok_or_else(|| {
        Error::InvalidInput("'to_entity' is required for path operation".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let from = EntityId::new(from_id);
    let to = EntityId::new(to_id);
    let max_depth = u32::try_from(args.depth.unwrap_or(5).min(5)).unwrap_or(5);

    match graph.find_path(&from, &to, max_depth)? {
        Some(result) => {
            let mut text = format!(
                "**Path from `{from_id}` to `{to_id}`**\n\n\
                 Path length: {} entities, {} relationships\n\n",
                result.entities.len(),
                result.relationships.len()
            );

            text.push_str("**Entities in path:**\n");
            for entity in &result.entities {
                text.push_str(&format!("- {} ({:?})\n", entity.name, entity.entity_type));
            }

            if !result.relationships.is_empty() {
                text.push_str("\n**Relationships:**\n");
                for rel in &result.relationships {
                    text.push_str(&format!(
                        "- `{}` --[{:?}]--> `{}`\n",
                        rel.from_entity, rel.relationship_type, rel.to_entity
                    ));
                }
            }

            Ok(ToolResult {
                content: vec![ToolContent::Text { text }],
                is_error: false,
            })
        },
        None => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "No path found from `{from_id}` to `{to_id}` within depth {max_depth}."
                ),
            }],
            is_error: false,
        }),
    }
}

/// Handles the `stats` operation for `subcog_graph`.
fn execute_graph_stats() -> Result<ToolResult> {
    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let stats = graph.get_stats()?;

    let text = format!(
        "**Knowledge Graph Statistics**\n\n\
         - **Entities**: {}\n\
         - **Relationships**: {}\n\
         - **Mentions**: {}\n\n\
         **Entity Types:**\n{}\n\
         **Relationship Types:**\n{}",
        stats.entity_count,
        stats.relationship_count,
        stats.mention_count,
        format_type_counts(&stats.entities_by_type),
        format_type_counts(&stats.relationships_by_type),
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

/// Handles the `visualize` operation for `subcog_graph`.
fn execute_graph_visualize_action(args: &GraphArgs) -> Result<ToolResult> {
    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let format = args.format.as_deref().unwrap_or("mermaid");
    let limit = args.limit.unwrap_or(50);
    let depth = u32::try_from(args.depth.unwrap_or(2).min(4)).unwrap_or(2);

    // Get entities and relationships to visualize
    let (entities, relationships) = if let Some(entity_id) = &args.entity_id {
        // Center on specific entity
        let id = EntityId::new(entity_id);
        let result = graph.traverse(&id, depth, None, None)?;
        (result.entities, result.relationships)
    } else {
        // Get all entities up to limit
        let query = EntityQuery::new().with_limit(limit);
        let entities = graph.query_entities(&query)?;

        // Get all relationships between these entities
        let entity_ids: std::collections::HashSet<_> = entities.iter().map(|e| &e.id).collect();
        let all_rels =
            graph.query_relationships(&RelationshipQuery::new().with_limit(limit * 2))?;
        let relationships: Vec<_> = all_rels
            .into_iter()
            .filter(|r| entity_ids.contains(&r.from_entity) && entity_ids.contains(&r.to_entity))
            .collect();

        (entities, relationships)
    };

    // Apply type filters if specified
    let entities: Vec<_> = if let Some(type_filter) = &args.entity_types {
        let allowed_types: std::collections::HashSet<_> = type_filter
            .iter()
            .filter_map(|s| parse_entity_type(s))
            .collect();
        entities
            .into_iter()
            .filter(|e| allowed_types.is_empty() || allowed_types.contains(&e.entity_type))
            .collect()
    } else {
        entities
    };

    let relationships: Vec<_> = if let Some(type_filter) = &args.relationship_types {
        let allowed_types: std::collections::HashSet<_> = type_filter
            .iter()
            .filter_map(|s| parse_relationship_type(s))
            .collect();
        relationships
            .into_iter()
            .filter(|r| allowed_types.is_empty() || allowed_types.contains(&r.relationship_type))
            .collect()
    } else {
        relationships
    };

    if entities.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "No entities to visualize.".to_string(),
            }],
            is_error: false,
        });
    }

    let visualization = match format {
        "mermaid" => generate_mermaid(&entities, &relationships),
        "dot" => generate_dot(&entities, &relationships),
        "ascii" => generate_ascii(&entities, &relationships),
        _ => {
            return Err(Error::InvalidInput(format!(
                "Unknown visualization format: {format}. Valid formats: mermaid, dot, ascii"
            )));
        },
    };

    let text = format!(
        "**Graph Visualization ({} entities, {} relationships)**\n\n```{}\n{}\n```",
        entities.len(),
        relationships.len(),
        if format == "dot" { "dot" } else { format },
        visualization
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

// ============================================================================
// Graph Query Operations (Legacy)
// ============================================================================

/// Executes the graph query tool (traversal operations).
///
/// # Errors
///
/// Returns an error if argument parsing or the operation fails.
pub fn execute_graph_query(arguments: Value) -> Result<ToolResult> {
    let args: GraphQueryArgs = serde_json::from_value(arguments)
        .map_err(|e| Error::InvalidInput(format!("Invalid graph query arguments: {e}")))?;

    match args.operation.as_str() {
        "neighbors" => execute_query_neighbors(&args),
        "path" => execute_query_path(&args),
        "stats" => execute_query_stats(),
        _ => Err(Error::InvalidInput(format!(
            "Unknown graph query operation: {}. Valid operations: neighbors, path, stats",
            args.operation
        ))),
    }
}

fn execute_query_neighbors(args: &GraphQueryArgs) -> Result<ToolResult> {
    let entity_id = args.entity_id.as_ref().ok_or_else(|| {
        Error::InvalidInput("entity_id is required for neighbors operation".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let id = EntityId::new(entity_id);
    let depth = u32::try_from(args.depth.unwrap_or(2).min(5)).unwrap_or(2);

    let neighbors = graph.get_neighbors(&id, depth)?;

    if neighbors.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("No neighbors found for entity `{entity_id}` within depth {depth}."),
            }],
            is_error: false,
        });
    }

    let mut text = format!(
        "**Neighbors of `{entity_id}` (depth {depth})**\n\nFound {} entities:\n\n",
        neighbors.len()
    );
    for entity in &neighbors {
        text.push_str(&format!(
            "- **{}** ({:?}) `{}`\n",
            entity.name, entity.entity_type, entity.id
        ));
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

fn execute_query_path(args: &GraphQueryArgs) -> Result<ToolResult> {
    let from_id = args.from_entity.as_ref().ok_or_else(|| {
        Error::InvalidInput("from_entity is required for path operation".to_string())
    })?;
    let to_id = args.to_entity.as_ref().ok_or_else(|| {
        Error::InvalidInput("to_entity is required for path operation".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let from = EntityId::new(from_id);
    let to = EntityId::new(to_id);
    let max_depth = u32::try_from(args.depth.unwrap_or(5).min(5)).unwrap_or(5);

    match graph.find_path(&from, &to, max_depth)? {
        Some(result) => {
            let mut text = format!(
                "**Path from `{from_id}` to `{to_id}`**\n\n\
                 Path length: {} entities, {} relationships\n\n",
                result.entities.len(),
                result.relationships.len()
            );

            text.push_str("**Entities in path:**\n");
            for entity in &result.entities {
                text.push_str(&format!("- {} ({:?})\n", entity.name, entity.entity_type));
            }

            if !result.relationships.is_empty() {
                text.push_str("\n**Relationships:**\n");
                for rel in &result.relationships {
                    text.push_str(&format!(
                        "- `{}` --[{:?}]--> `{}`\n",
                        rel.from_entity, rel.relationship_type, rel.to_entity
                    ));
                }
            }

            Ok(ToolResult {
                content: vec![ToolContent::Text { text }],
                is_error: false,
            })
        },
        None => Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "No path found from `{from_id}` to `{to_id}` within depth {max_depth}."
                ),
            }],
            is_error: false,
        }),
    }
}

fn execute_query_stats() -> Result<ToolResult> {
    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let stats = graph.get_stats()?;

    let text = format!(
        "**Knowledge Graph Statistics**\n\n\
         - **Entities**: {}\n\
         - **Relationships**: {}\n\
         - **Mentions**: {}\n\n\
         **Entity Types:**\n{}\n\
         **Relationship Types:**\n{}",
        stats.entity_count,
        stats.relationship_count,
        stats.mention_count,
        format_type_counts(&stats.entities_by_type),
        format_type_counts(&stats.relationships_by_type),
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

fn format_type_counts<K: std::fmt::Debug>(counts: &HashMap<K, usize>) -> String {
    if counts.is_empty() {
        return "  (none)\n".to_string();
    }
    let mut result = String::new();
    for (type_name, count) in counts {
        result.push_str(&format!("  - {type_name:?}: {count}\n"));
    }
    result
}

// ============================================================================
// Entity Extraction
// ============================================================================

/// Executes the extract entities tool (LLM-powered extraction).
///
/// # Errors
///
/// Returns an error if argument parsing or extraction fails.
pub fn execute_extract_entities(arguments: Value) -> Result<ToolResult> {
    let args: ExtractEntitiesArgs = serde_json::from_value(arguments)
        .map_err(|e| Error::InvalidInput(format!("Invalid extract entities arguments: {e}")))?;

    if args.content.trim().is_empty() {
        return Err(Error::InvalidInput(
            "Content is required for entity extraction".to_string(),
        ));
    }

    let container = ServiceContainer::from_current_dir_or_user()?;

    // Load config and build LLM provider if available
    let config = SubcogConfig::load_default();
    let extractor = if let Some(llm) = build_llm_provider_for_entity_extraction(&config) {
        container.entity_extractor_with_llm(llm)
    } else {
        container.entity_extractor()
    };

    let min_confidence = args.min_confidence.unwrap_or(0.5);
    let extractor = extractor.with_min_confidence(min_confidence);

    let result = extractor.extract(&args.content)?;

    let mut text = format!(
        "**Entity Extraction Results**{}{}\n\n",
        if result.used_fallback {
            " (fallback mode)"
        } else {
            ""
        },
        if let Some(ref memory_id) = args.memory_id {
            format!("\nSource memory: `{memory_id}`")
        } else {
            String::new()
        }
    );

    if result.entities.is_empty() && result.relationships.is_empty() {
        text.push_str("No entities or relationships extracted.\n");
    } else {
        if !result.entities.is_empty() {
            text.push_str(&format!("**Entities ({}):**\n", result.entities.len()));
            for entity in &result.entities {
                text.push_str(&format!(
                    "- **{}** ({}) - confidence: {:.2}\n",
                    entity.name, entity.entity_type, entity.confidence
                ));
            }
            text.push('\n');
        }

        if !result.relationships.is_empty() {
            text.push_str(&format!(
                "**Relationships ({}):**\n",
                result.relationships.len()
            ));
            for rel in &result.relationships {
                text.push_str(&format!(
                    "- {} --[{}]--> {} (confidence: {:.2})\n",
                    rel.from, rel.relationship_type, rel.to, rel.confidence
                ));
            }
        }
    }

    // Store entities if requested (with automatic deduplication)
    if args.store && !result.entities.is_empty() {
        let graph = container.graph()?;
        let graph_entities = extractor.to_graph_entities(&result);

        // Store entities with deduplication and build map with actual IDs
        let mut entity_map: HashMap<String, Entity> = HashMap::new();
        for entity in &graph_entities {
            let actual_id = graph.store_entity_deduped(entity)?;
            // Create entity with the actual ID for relationship mapping
            let mut stored_entity = entity.clone();
            stored_entity.id = actual_id;
            entity_map.insert(entity.name.clone(), stored_entity);
        }

        // Store relationships using the actual entity IDs
        let graph_rels = extractor.to_graph_relationships(&result, &entity_map);

        for rel in &graph_rels {
            graph.store_relationship(rel)?;
        }

        text.push_str(&format!(
            "\n✓ Stored {} entities and {} relationships in graph (with deduplication).\n",
            graph_entities.len(),
            graph_rels.len()
        ));
    }

    if !result.warnings.is_empty() {
        text.push_str("\n**Warnings:**\n");
        for warning in &result.warnings {
            text.push_str(&format!("- {warning}\n"));
        }
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

// ============================================================================
// Entity Merge
// ============================================================================

/// Executes the entity merge tool (deduplication).
///
/// # Errors
///
/// Returns an error if argument parsing or the operation fails.
pub fn execute_entity_merge(arguments: Value) -> Result<ToolResult> {
    let args: EntityMergeArgs = serde_json::from_value(arguments)
        .map_err(|e| Error::InvalidInput(format!("Invalid entity merge arguments: {e}")))?;

    match args.action.as_str() {
        "find_duplicates" => execute_find_duplicates(&args),
        "merge" => execute_merge(&args),
        _ => Err(Error::InvalidInput(format!(
            "Unknown merge action: {}. Valid actions: find_duplicates, merge",
            args.action
        ))),
    }
}

fn execute_find_duplicates(args: &EntityMergeArgs) -> Result<ToolResult> {
    let entity_id = args.entity_id.as_ref().ok_or_else(|| {
        Error::InvalidInput("entity_id is required for find_duplicates action".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let id = EntityId::new(entity_id);
    let entity = graph
        .get_entity(&id)?
        .ok_or_else(|| Error::OperationFailed {
            operation: "find_duplicates".to_string(),
            cause: format!("Entity not found: {entity_id}"),
        })?;

    let threshold = args.threshold.unwrap_or(0.7);
    let duplicates = graph.find_duplicates(&entity, threshold)?;

    if duplicates.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!(
                    "No potential duplicates found for '{}' (threshold: {:.0}%)",
                    entity.name,
                    threshold * 100.0
                ),
            }],
            is_error: false,
        });
    }

    let mut text = format!(
        "**Potential duplicates for '{}'** (threshold: {:.0}%)\n\n",
        entity.name,
        threshold * 100.0
    );
    for dup in &duplicates {
        text.push_str(&format!(
            "- **{}** ({:?}) `{}`\n",
            dup.name, dup.entity_type, dup.id
        ));
    }
    text.push_str(&format!(
        "\nTo merge, use: `merge` action with entity_ids: [\"{}\", {}]",
        entity_id,
        duplicates
            .iter()
            .map(|e| format!("\"{}\"", e.id))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

fn execute_merge(args: &EntityMergeArgs) -> Result<ToolResult> {
    let entity_ids = args.entity_ids.as_ref().ok_or_else(|| {
        Error::InvalidInput("entity_ids is required for merge action (minimum 2)".to_string())
    })?;

    if entity_ids.len() < 2 {
        return Err(Error::InvalidInput(
            "At least 2 entity IDs are required for merge".to_string(),
        ));
    }

    let canonical_name = args.canonical_name.as_ref().ok_or_else(|| {
        Error::InvalidInput("canonical_name is required for merge action".to_string())
    })?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let ids: Vec<EntityId> = entity_ids.iter().map(EntityId::new).collect();
    let merged = graph.merge_entities(&ids, canonical_name)?;

    let text = format!(
        "**Entities Merged Successfully**\n\n\
         - **Canonical Entity**: {} `{}`\n\
         - **Merged IDs**: {}\n\n\
         All relationships and mentions have been transferred to the canonical entity.",
        merged.name,
        merged.id,
        entity_ids.join(", ")
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

// ============================================================================
// Relationship Inference
// ============================================================================

/// Executes the relationship inference tool.
///
/// # Errors
///
/// Returns an error if argument parsing or inference fails.
pub fn execute_relationship_infer(arguments: Value) -> Result<ToolResult> {
    let args: RelationshipInferArgs = serde_json::from_value(arguments)
        .map_err(|e| Error::InvalidInput(format!("Invalid relationship infer arguments: {e}")))?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    // Get entities to analyze
    let entities = if let Some(entity_ids) = &args.entity_ids {
        let mut entities = Vec::new();
        for id_str in entity_ids {
            let id = EntityId::new(id_str);
            if let Some(entity) = graph.get_entity(&id)? {
                entities.push(entity);
            }
        }
        entities
    } else {
        // Get recent entities
        let limit = args.limit.unwrap_or(50);
        let query = EntityQuery::new().with_limit(limit);
        graph.query_entities(&query)?
    };

    if entities.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "No entities found to analyze.".to_string(),
            }],
            is_error: false,
        });
    }

    // Load config and build LLM provider if available
    let config = SubcogConfig::load_default();
    let extractor = if let Some(llm) = build_llm_provider_for_entity_extraction(&config) {
        container.entity_extractor_with_llm(llm)
    } else {
        container.entity_extractor()
    };

    let min_confidence = args.min_confidence.unwrap_or(0.6);
    let extractor = extractor.with_min_confidence(min_confidence);

    let result = extractor.infer_relationships(&entities)?;

    let mut text = format!(
        "**Relationship Inference Results**{}\n\n",
        if result.used_fallback {
            " (fallback mode)"
        } else {
            ""
        }
    );

    if result.relationships.is_empty() {
        text.push_str("No relationships inferred.\n");
    } else {
        text.push_str(&format!(
            "**Inferred Relationships ({}):**\n",
            result.relationships.len()
        ));
        for rel in &result.relationships {
            text.push_str(&format!(
                "- {} --[{}]--> {} (confidence: {:.2})",
                rel.from, rel.relationship_type, rel.to, rel.confidence
            ));
            if let Some(reasoning) = &rel.reasoning {
                text.push_str(&format!("\n  Reasoning: {reasoning}"));
            }
            text.push('\n');
        }
    }

    // Store relationships if requested
    if args.store && !result.relationships.is_empty() {
        // Build entity map for lookup
        let entity_map: HashMap<String, &Entity> =
            entities.iter().map(|e| (e.name.clone(), e)).collect();

        let mut stored = 0;
        for inferred in &result.relationships {
            if let (Some(from), Some(to)) =
                (entity_map.get(&inferred.from), entity_map.get(&inferred.to))
                && let Some(rel_type) = parse_relationship_type(&inferred.relationship_type)
            {
                let mut rel = Relationship::new(from.id.clone(), to.id.clone(), rel_type);
                rel.confidence = inferred.confidence;
                graph.store_relationship(&rel)?;
                stored += 1;
            }
        }

        text.push_str(&format!("\n✓ Stored {stored} relationships in graph.\n"));
    }

    if !result.warnings.is_empty() {
        text.push_str("\n**Warnings:**\n");
        for warning in &result.warnings {
            text.push_str(&format!("- {warning}\n"));
        }
    }

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

// ============================================================================
// Graph Visualization
// ============================================================================

/// Executes the graph visualization tool.
///
/// # Errors
///
/// Returns an error if argument parsing or visualization fails.
pub fn execute_graph_visualize(arguments: Value) -> Result<ToolResult> {
    let args: GraphVisualizeArgs = serde_json::from_value(arguments)
        .map_err(|e| Error::InvalidInput(format!("Invalid graph visualize arguments: {e}")))?;

    let container = ServiceContainer::from_current_dir_or_user()?;
    let graph = container.graph()?;

    let format = args.format.as_deref().unwrap_or("mermaid");
    let limit = args.limit.unwrap_or(50);
    let depth = u32::try_from(args.depth.unwrap_or(2).min(4)).unwrap_or(2);

    // Get entities and relationships to visualize
    let (entities, relationships) = if let Some(entity_id) = &args.entity_id {
        // Center on specific entity
        let id = EntityId::new(entity_id);
        let result = graph.traverse(&id, depth, None, None)?;
        (result.entities, result.relationships)
    } else {
        // Get all entities up to limit
        let query = EntityQuery::new().with_limit(limit);
        let entities = graph.query_entities(&query)?;

        // Get all relationships between these entities
        let entity_ids: std::collections::HashSet<_> = entities.iter().map(|e| &e.id).collect();
        let all_rels =
            graph.query_relationships(&RelationshipQuery::new().with_limit(limit * 2))?;
        let relationships: Vec<_> = all_rels
            .into_iter()
            .filter(|r| entity_ids.contains(&r.from_entity) && entity_ids.contains(&r.to_entity))
            .collect();

        (entities, relationships)
    };

    // Apply type filters if specified
    let entities: Vec<_> = if let Some(type_filter) = &args.entity_types {
        let allowed_types: std::collections::HashSet<_> = type_filter
            .iter()
            .filter_map(|s| parse_entity_type(s))
            .collect();
        entities
            .into_iter()
            .filter(|e| allowed_types.is_empty() || allowed_types.contains(&e.entity_type))
            .collect()
    } else {
        entities
    };

    let relationships: Vec<_> = if let Some(type_filter) = &args.relationship_types {
        let allowed_types: std::collections::HashSet<_> = type_filter
            .iter()
            .filter_map(|s| parse_relationship_type(s))
            .collect();
        relationships
            .into_iter()
            .filter(|r| allowed_types.is_empty() || allowed_types.contains(&r.relationship_type))
            .collect()
    } else {
        relationships
    };

    if entities.is_empty() {
        return Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: "No entities to visualize.".to_string(),
            }],
            is_error: false,
        });
    }

    let visualization = match format {
        "mermaid" => generate_mermaid(&entities, &relationships),
        "dot" => generate_dot(&entities, &relationships),
        "ascii" => generate_ascii(&entities, &relationships),
        _ => {
            return Err(Error::InvalidInput(format!(
                "Unknown visualization format: {format}. Valid formats: mermaid, dot, ascii"
            )));
        },
    };

    let text = format!(
        "**Graph Visualization ({} entities, {} relationships)**\n\n```{}\n{}\n```",
        entities.len(),
        relationships.len(),
        if format == "dot" { "dot" } else { format },
        visualization
    );

    Ok(ToolResult {
        content: vec![ToolContent::Text { text }],
        is_error: false,
    })
}

fn generate_mermaid(entities: &[Entity], relationships: &[Relationship]) -> String {
    let mut output = String::from("graph LR\n");

    // Add entities as nodes
    for entity in entities {
        let shape = match entity.entity_type {
            crate::models::graph::EntityType::Person => ("((", "))"),
            crate::models::graph::EntityType::Organization => ("[", "]"),
            crate::models::graph::EntityType::Technology => ("{", "}"),
            crate::models::graph::EntityType::Concept => ("([", "])"),
            crate::models::graph::EntityType::File => ("[[", "]]"),
        };
        let id = sanitize_mermaid_id(entity.id.as_ref());
        let name = sanitize_mermaid_label(&entity.name);
        output.push_str(&format!("    {id}{}{name}{}\n", shape.0, shape.1));
    }

    // Add relationships as edges
    for rel in relationships {
        let from = sanitize_mermaid_id(rel.from_entity.as_ref());
        let to = sanitize_mermaid_id(rel.to_entity.as_ref());
        let label = format!("{:?}", rel.relationship_type);
        output.push_str(&format!("    {from} -->|{label}| {to}\n"));
    }

    output
}

fn generate_dot(entities: &[Entity], relationships: &[Relationship]) -> String {
    let mut output =
        String::from("digraph G {\n    rankdir=LR;\n    node [fontname=\"Arial\"];\n\n");

    // Add entities as nodes
    for entity in entities {
        let shape = match entity.entity_type {
            crate::models::graph::EntityType::Person => "ellipse",
            crate::models::graph::EntityType::Organization => "box",
            crate::models::graph::EntityType::Technology => "diamond",
            crate::models::graph::EntityType::Concept => "oval",
            crate::models::graph::EntityType::File => "note",
        };
        let id = sanitize_dot_id(entity.id.as_ref());
        let name = entity.name.replace('"', "\\\"");
        output.push_str(&format!("    {id} [label=\"{name}\" shape={shape}];\n"));
    }

    output.push('\n');

    // Add relationships as edges
    for rel in relationships {
        let from = sanitize_dot_id(rel.from_entity.as_ref());
        let to = sanitize_dot_id(rel.to_entity.as_ref());
        let label = format!("{:?}", rel.relationship_type);
        output.push_str(&format!("    {from} -> {to} [label=\"{label}\"];\n"));
    }

    output.push_str("}\n");
    output
}

fn generate_ascii(entities: &[Entity], relationships: &[Relationship]) -> String {
    let mut output = String::new();

    output.push_str("ENTITIES:\n");
    output.push_str(&"-".repeat(40));
    output.push('\n');
    for entity in entities {
        let type_char = match entity.entity_type {
            crate::models::graph::EntityType::Person => "👤",
            crate::models::graph::EntityType::Organization => "🏢",
            crate::models::graph::EntityType::Technology => "⚙️",
            crate::models::graph::EntityType::Concept => "💡",
            crate::models::graph::EntityType::File => "📄",
        };
        output.push_str(&format!("{} {} ({})\n", type_char, entity.name, entity.id));
    }

    output.push_str("\nRELATIONSHIPS:\n");
    output.push_str(&"-".repeat(40));
    output.push('\n');
    for rel in relationships {
        output.push_str(&format!(
            "{} --[{:?}]--> {}\n",
            rel.from_entity, rel.relationship_type, rel.to_entity
        ));
    }

    output
}

fn sanitize_mermaid_id(id: &str) -> String {
    id.replace(['-', '.'], "_")
}

fn sanitize_mermaid_label(label: &str) -> String {
    label.replace('"', "'").replace('[', "(").replace(']', ")")
}

fn sanitize_dot_id(id: &str) -> String {
    let sanitized = id.replace(['-', '.'], "_");
    // Ensure it starts with a letter or underscore
    if sanitized.chars().next().is_none_or(char::is_numeric) {
        format!("n_{sanitized}")
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::graph::{EntityType, RelationshipType};

    // ========== Sanitization Tests ==========

    #[test]
    fn test_sanitize_mermaid_id() {
        assert_eq!(sanitize_mermaid_id("entity-123.test"), "entity_123_test");
    }

    #[test]
    fn test_sanitize_mermaid_id_no_special_chars() {
        assert_eq!(sanitize_mermaid_id("simple_id"), "simple_id");
    }

    #[test]
    fn test_sanitize_mermaid_id_multiple_special() {
        assert_eq!(sanitize_mermaid_id("a-b.c-d.e"), "a_b_c_d_e");
    }

    #[test]
    fn test_sanitize_dot_id() {
        assert_eq!(sanitize_dot_id("entity-123"), "entity_123");
        assert_eq!(sanitize_dot_id("123-test"), "n_123_test");
    }

    #[test]
    fn test_sanitize_dot_id_empty() {
        assert_eq!(sanitize_dot_id(""), "n_");
    }

    #[test]
    fn test_sanitize_dot_id_numeric_start() {
        assert_eq!(sanitize_dot_id("42abc"), "n_42abc");
    }

    // ========== Entity Type Parsing Tests ==========

    #[test]
    fn test_parse_entity_type_person() {
        assert!(matches!(
            parse_entity_type("person"),
            Some(EntityType::Person)
        ));
        assert!(matches!(
            parse_entity_type("Person"),
            Some(EntityType::Person)
        ));
        assert!(matches!(
            parse_entity_type("PERSON"),
            Some(EntityType::Person)
        ));
    }

    #[test]
    fn test_parse_entity_type_organization() {
        assert!(matches!(
            parse_entity_type("organization"),
            Some(EntityType::Organization)
        ));
    }

    #[test]
    fn test_parse_entity_type_concept() {
        assert!(matches!(
            parse_entity_type("concept"),
            Some(EntityType::Concept)
        ));
    }

    #[test]
    fn test_parse_entity_type_technology() {
        assert!(matches!(
            parse_entity_type("technology"),
            Some(EntityType::Technology)
        ));
    }

    #[test]
    fn test_parse_entity_type_file() {
        assert!(matches!(parse_entity_type("file"), Some(EntityType::File)));
    }

    #[test]
    fn test_parse_entity_type_unknown() {
        assert!(parse_entity_type("unknown_type").is_none());
    }

    // ========== Relationship Type Parsing Tests ==========

    #[test]
    fn test_parse_relationship_type_works_at() {
        assert!(matches!(
            parse_relationship_type("works_at"),
            Some(RelationshipType::WorksAt)
        ));
    }

    #[test]
    fn test_parse_relationship_type_uses() {
        assert!(matches!(
            parse_relationship_type("uses"),
            Some(RelationshipType::Uses)
        ));
    }

    #[test]
    fn test_parse_relationship_type_relates_to() {
        assert!(matches!(
            parse_relationship_type("relates_to"),
            Some(RelationshipType::RelatesTo)
        ));
    }

    #[test]
    fn test_parse_relationship_type_unknown() {
        assert!(parse_relationship_type("some_random_rel").is_none());
    }

    // ========== EntitiesArgs Validation Tests ==========

    #[test]
    fn test_entities_args_rejects_unknown_fields() {
        let json = r#"{"action": "list", "unknown_field": true}"#;
        let result: std::result::Result<EntitiesArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_entities_args_accepts_valid_fields() {
        let json = r#"{"action": "create", "name": "Test", "entity_type": "Person"}"#;
        let result: std::result::Result<EntitiesArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_entities_args_list_action() {
        let json = r#"{"action": "list", "limit": 10}"#;
        let result: std::result::Result<EntitiesArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.action, "list");
        assert_eq!(args.limit, Some(10));
    }

    #[test]
    fn test_entities_args_with_aliases() {
        let json = r#"{"action": "create", "name": "Bob", "entity_type": "Person", "aliases": ["Robert", "Bobby"]}"#;
        let result: std::result::Result<EntitiesArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.aliases.expect("should have aliases").len(), 2);
    }

    // ========== RelationshipsArgs Validation Tests ==========

    #[test]
    fn test_relationships_args_list() {
        let json = r#"{"action": "list"}"#;
        let result: std::result::Result<RelationshipsArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_relationships_args_create() {
        let json = r#"{"action": "create", "from_entity": "e1", "to_entity": "e2", "relationship_type": "depends_on"}"#;
        let result: std::result::Result<RelationshipsArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.from_entity.expect("from"), "e1");
        assert_eq!(args.to_entity.expect("to"), "e2");
    }

    #[test]
    fn test_relationships_args_rejects_unknown() {
        let json = r#"{"action": "list", "extra": true}"#;
        let result: std::result::Result<RelationshipsArgs, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // ========== GraphQueryArgs Validation Tests ==========

    #[test]
    fn test_graph_query_args_accepts_valid_fields() {
        let json = r#"{"operation": "stats"}"#;
        let result: std::result::Result<GraphQueryArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_graph_query_args_traverse() {
        let json = r#"{"operation": "traverse", "entity_id": "e123", "depth": 3}"#;
        let result: std::result::Result<GraphQueryArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.operation, "traverse");
        assert_eq!(args.depth, Some(3));
    }

    #[test]
    fn test_graph_query_args_path() {
        let json = r#"{"operation": "path", "from_entity": "start", "to_entity": "end"}"#;
        let result: std::result::Result<GraphQueryArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.from_entity.expect("from"), "start");
        assert_eq!(args.to_entity.expect("to"), "end");
    }

    // ========== ExtractEntitiesArgs Validation Tests ==========

    #[test]
    fn test_extract_entities_args_accepts_valid_fields() {
        let json = r#"{"content": "Alice works at Acme", "store": true}"#;
        let result: std::result::Result<ExtractEntitiesArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert!(args.store);
    }

    #[test]
    fn test_extract_entities_args_with_memory_id() {
        let json = r#"{"content": "Test content", "memory_id": "mem_123"}"#;
        let result: std::result::Result<ExtractEntitiesArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.memory_id.expect("memory_id"), "mem_123");
    }

    #[test]
    fn test_extract_entities_args_min_confidence() {
        let json = r#"{"content": "Test", "min_confidence": 0.8}"#;
        let result: std::result::Result<ExtractEntitiesArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert!((args.min_confidence.expect("confidence") - 0.8).abs() < f32::EPSILON);
    }

    // ========== EntityMergeArgs Validation Tests ==========

    #[test]
    fn test_entity_merge_args_find_duplicates() {
        let json = r#"{"action": "find_duplicates", "entity_id": "e123"}"#;
        let result: std::result::Result<EntityMergeArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_entity_merge_args_merge() {
        let json =
            r#"{"action": "merge", "entity_ids": ["e1", "e2"], "canonical_name": "Merged Entity"}"#;
        let result: std::result::Result<EntityMergeArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.entity_ids.expect("ids").len(), 2);
    }

    #[test]
    fn test_entity_merge_args_threshold() {
        let json = r#"{"action": "find_duplicates", "threshold": 0.85}"#;
        let result: std::result::Result<EntityMergeArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert!((args.threshold.expect("threshold") - 0.85).abs() < f32::EPSILON);
    }

    // ========== RelationshipInferArgs Validation Tests ==========

    #[test]
    fn test_relationship_infer_args_basic() {
        let json = r#"{"store": false}"#;
        let result: std::result::Result<RelationshipInferArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_relationship_infer_args_with_entity_ids() {
        let json = r#"{"entity_ids": ["e1", "e2", "e3"], "store": true}"#;
        let result: std::result::Result<RelationshipInferArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert!(args.store);
        assert_eq!(args.entity_ids.expect("ids").len(), 3);
    }

    #[test]
    fn test_relationship_infer_args_confidence() {
        let json = r#"{"min_confidence": 0.7, "limit": 25}"#;
        let result: std::result::Result<RelationshipInferArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.limit, Some(25));
    }

    // ========== GraphVisualizeArgs Validation Tests ==========

    #[test]
    fn test_graph_visualize_args_accepts_valid_fields() {
        let json = r#"{"format": "mermaid", "depth": 3}"#;
        let result: std::result::Result<GraphVisualizeArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_graph_visualize_args_dot_format() {
        let json = r#"{"format": "dot"}"#;
        let result: std::result::Result<GraphVisualizeArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.format, Some("dot".to_string()));
    }

    #[test]
    fn test_graph_visualize_args_with_entity() {
        let json = r#"{"entity_id": "e123", "depth": 2}"#;
        let result: std::result::Result<GraphVisualizeArgs, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        let args = result.expect("should parse");
        assert_eq!(args.entity_id.expect("entity_id"), "e123");
    }

    // ========== Format Helper Tests ==========

    #[test]
    fn test_format_type_counts_empty() {
        let counts: HashMap<String, usize> = HashMap::new();
        assert_eq!(format_type_counts(&counts), "  (none)\n");
    }

    #[test]
    fn test_format_type_counts_single() {
        let mut counts: HashMap<String, usize> = HashMap::new();
        counts.insert("Person".to_string(), 5);
        // Uses Debug formatting, so strings get quotes
        assert_eq!(format_type_counts(&counts), "  - \"Person\": 5\n");
    }

    #[test]
    fn test_format_type_counts_multiple() {
        let mut counts: HashMap<String, usize> = HashMap::new();
        counts.insert("Person".to_string(), 3);
        counts.insert("Org".to_string(), 2);
        let result = format_type_counts(&counts);
        // Order is not guaranteed, so check both are present (with Debug formatting)
        assert!(result.contains("\"Person\": 3"));
        assert!(result.contains("\"Org\": 2"));
    }
}

//! Entity extraction service for extracting entities from text using LLM.
//!
//! Provides LLM-powered entity extraction with graceful degradation when
//! LLM is unavailable.
//!
//! # Example
//!
//! ```rust,ignore
//! use subcog::services::EntityExtractorService;
//! use subcog::llm::AnthropicClient;
//! use subcog::models::Domain;
//!
//! let llm = AnthropicClient::new();
//! let service = EntityExtractorService::new(Box::new(llm), Domain::for_user());
//!
//! let result = service.extract("Alice from Acme Corp uses Rust")?;
//! println!("Extracted {} entities", result.entities.len());
//! ```

use crate::llm::{LlmProvider, OperationMode, build_system_prompt};
use crate::models::Domain;
use crate::models::graph::{Entity, EntityType, Relationship, RelationshipType};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Technology patterns for fallback entity extraction.
///
/// Organized by category for maintainability.
static TECH_PATTERNS: &[&str] = &[
    // Programming Languages (18)
    "Rust",
    "Python",
    "Java",
    "JavaScript",
    "TypeScript",
    "Go",
    "C++",
    "C#",
    "Ruby",
    "PHP",
    "Swift",
    "Kotlin",
    "Scala",
    "Elixir",
    "Haskell",
    "Clojure",
    "F#",
    "Zig",
    // Databases (12)
    "PostgreSQL",
    "MySQL",
    "SQLite",
    "Redis",
    "MongoDB",
    "Cassandra",
    "DynamoDB",
    "CockroachDB",
    "ClickHouse",
    "Elasticsearch",
    "Neo4j",
    "Firestore",
    // Web Frameworks (14)
    "React",
    "Vue",
    "Angular",
    "Svelte",
    "Next.js",
    "Nuxt",
    "Express",
    "Django",
    "Rails",
    "Laravel",
    "Spring",
    "Flask",
    "FastAPI",
    "Actix",
    // Cloud Providers (9)
    "AWS",
    "Azure",
    "GCP",
    "Cloudflare",
    "Vercel",
    "Netlify",
    "Heroku",
    "DigitalOcean",
    "Linode",
    // Container/Orchestration (8)
    "Docker",
    "Kubernetes",
    "k8s",
    "Podman",
    "Nomad",
    "ECS",
    "EKS",
    "GKE",
    // Infrastructure (6)
    "Terraform",
    "Ansible",
    "Prometheus",
    "Grafana",
    "Datadog",
    "Jaeger",
    // Message Queues (6)
    "Kafka",
    "RabbitMQ",
    "NATS",
    "Pulsar",
    "SQS",
    "Pub/Sub",
    // Build Tools (10)
    "Webpack",
    "Vite",
    "esbuild",
    "Rollup",
    "Cargo",
    "npm",
    "yarn",
    "pnpm",
    "Maven",
    "Gradle",
    // Runtime Environments (4)
    "Node.js",
    "Deno",
    "Bun",
    "WASM",
    // APIs/Protocols (6)
    "REST",
    "GraphQL",
    "gRPC",
    "WebSocket",
    "MQTT",
    "OpenAPI",
];

/// Result of entity extraction from text.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractionResult {
    /// Extracted entities.
    pub entities: Vec<ExtractedEntity>,
    /// Extracted relationships.
    pub relationships: Vec<ExtractedRelationship>,
    /// Whether extraction used fallback (no LLM).
    pub used_fallback: bool,
    /// Any warnings during extraction.
    pub warnings: Vec<String>,
}

/// An entity extracted from text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    /// Entity name.
    pub name: String,
    /// Entity type as string (maps to [`EntityType`]).
    #[serde(rename = "type")]
    pub entity_type: String,
    /// Confidence score (0.0-1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Alternative names for this entity.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Brief description if available.
    #[serde(default)]
    pub description: Option<String>,
}

const fn default_confidence() -> f32 {
    0.8
}

/// A relationship extracted from text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRelationship {
    /// Source entity name.
    pub from: String,
    /// Target entity name.
    pub to: String,
    /// Relationship type as string.
    #[serde(rename = "type")]
    pub relationship_type: String,
    /// Confidence score (0.0-1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Evidence text supporting this relationship.
    #[serde(default)]
    pub evidence: Option<String>,
}

/// Result of relationship inference between entities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InferenceResult {
    /// Inferred relationships.
    pub relationships: Vec<InferredRelationship>,
    /// Whether inference used fallback (no LLM).
    pub used_fallback: bool,
    /// Any warnings during inference.
    pub warnings: Vec<String>,
}

/// A relationship inferred between existing entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferredRelationship {
    /// Source entity name.
    pub from: String,
    /// Target entity name.
    pub to: String,
    /// Relationship type as string.
    #[serde(rename = "type")]
    pub relationship_type: String,
    /// Confidence score (0.0-1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Reasoning for this inference.
    #[serde(default)]
    pub reasoning: Option<String>,
}

/// LLM response structure for entity extraction.
#[derive(Debug, Clone, Deserialize)]
struct LlmExtractionResponse {
    #[serde(default)]
    entities: Vec<ExtractedEntity>,
    #[serde(default)]
    relationships: Vec<ExtractedRelationship>,
}

/// LLM response structure for relationship inference.
#[derive(Debug, Clone, Deserialize)]
struct LlmInferenceResponse {
    #[serde(default)]
    relationships: Vec<InferredRelationship>,
}

/// Service for extracting entities from text content.
///
/// Uses an LLM to identify named entities and their relationships,
/// with graceful fallback when LLM is unavailable.
pub struct EntityExtractorService {
    /// LLM provider for extraction.
    llm: Option<Arc<dyn LlmProvider>>,
    /// Default domain for extracted entities.
    domain: Domain,
    /// Minimum confidence threshold for entities.
    min_confidence: f32,
}

impl EntityExtractorService {
    /// Creates a new entity extractor with an LLM provider.
    #[must_use]
    pub fn new(llm: Box<dyn LlmProvider>, domain: Domain) -> Self {
        Self {
            llm: Some(Arc::from(llm)),
            domain,
            min_confidence: 0.5,
        }
    }

    /// Creates an entity extractor without LLM (fallback mode only).
    #[must_use]
    pub const fn without_llm(domain: Domain) -> Self {
        Self {
            llm: None,
            domain,
            min_confidence: 0.5,
        }
    }

    /// Creates an entity extractor with a shared LLM provider.
    #[must_use]
    pub const fn with_shared_llm(llm: Arc<dyn LlmProvider>, domain: Domain) -> Self {
        Self {
            llm: Some(llm),
            domain,
            min_confidence: 0.5,
        }
    }

    /// Sets the minimum confidence threshold for extracted entities.
    #[must_use]
    pub const fn with_min_confidence(mut self, threshold: f32) -> Self {
        self.min_confidence = threshold;
        self
    }

    /// Extracts entities and relationships from text.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to extract entities from.
    ///
    /// # Returns
    ///
    /// An [`ExtractionResult`] containing extracted entities and relationships.
    ///
    /// # Errors
    ///
    /// Returns an error if LLM extraction fails and no fallback is possible.
    pub fn extract(&self, text: &str) -> Result<ExtractionResult> {
        if text.trim().is_empty() {
            return Ok(ExtractionResult::default());
        }

        match &self.llm {
            Some(llm) => self.extract_with_llm(llm, text),
            None => Ok(self.extract_fallback(text)),
        }
    }

    /// Extracts entities using LLM.
    fn extract_with_llm(&self, llm: &Arc<dyn LlmProvider>, text: &str) -> Result<ExtractionResult> {
        let system = build_system_prompt(OperationMode::EntityExtraction, None);
        let user = format!("Extract entities and relationships from this text:\n\n{text}");

        let response = match llm.complete_with_system(&system, &user) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "LLM extraction failed, using fallback");
                return Ok(self.extract_fallback(text));
            },
        };

        // Parse JSON response
        let parsed = self.parse_llm_response(&response)?;

        // Filter by confidence threshold
        let entities: Vec<_> = parsed
            .entities
            .into_iter()
            .filter(|e| e.confidence >= self.min_confidence)
            .collect();

        let relationships: Vec<_> = parsed
            .relationships
            .into_iter()
            .filter(|r| r.confidence >= self.min_confidence)
            .collect();

        Ok(ExtractionResult {
            entities,
            relationships,
            used_fallback: false,
            warnings: Vec::new(),
        })
    }

    /// Parses the LLM JSON response.
    fn parse_llm_response(&self, response: &str) -> Result<LlmExtractionResponse> {
        // Try to find JSON in the response (it might be wrapped in markdown)
        let json_str = self.extract_json(response);

        serde_json::from_str(&json_str).map_err(|e| {
            tracing::warn!(error = %e, response = %response, "Failed to parse LLM response");
            Error::OperationFailed {
                operation: "parse_entity_extraction".to_string(),
                cause: format!("Invalid JSON response: {e}"),
            }
        })
    }

    /// Extracts JSON from a response that may be wrapped in markdown.
    fn extract_json(&self, response: &str) -> String {
        let trimmed = response.trim();

        // Try markdown code block first
        if let Some(json) = self.extract_json_from_markdown(trimmed) {
            return json;
        }

        // Try raw JSON object
        if let Some(json) = self.extract_raw_json(trimmed) {
            return json;
        }

        // Return as-is if no JSON found
        trimmed.to_string()
    }

    /// Extracts JSON from a markdown code block.
    fn extract_json_from_markdown(&self, text: &str) -> Option<String> {
        let start = text.find("```json")?;
        let end_offset = text[start..]
            .find("```\n")
            .or_else(|| text[start..].rfind("```"))?;

        let json_start = start + 7; // len("```json")
        let json_end = start + end_offset;

        if json_start < json_end {
            Some(text[json_start..json_end].trim().to_string())
        } else {
            None
        }
    }

    /// Extracts a raw JSON object from text.
    fn extract_raw_json(&self, text: &str) -> Option<String> {
        let start = text.find('{')?;
        let end = text.rfind('}')?;

        if start < end {
            Some(text[start..=end].to_string())
        } else {
            None
        }
    }

    /// Fallback extraction when LLM is unavailable.
    ///
    /// Uses simple pattern matching for common entity patterns.
    fn extract_fallback(&self, text: &str) -> ExtractionResult {
        let mut entities = Vec::new();
        let mut warnings = vec!["LLM unavailable, using pattern-based fallback".to_string()];

        for pattern in TECH_PATTERNS {
            if text.contains(pattern) {
                entities.push(ExtractedEntity {
                    name: (*pattern).to_string(),
                    entity_type: "Technology".to_string(),
                    confidence: 0.7,
                    aliases: Vec::new(),
                    description: None,
                });
            }
        }

        if entities.is_empty() {
            warnings.push("No entities detected with fallback patterns".to_string());
        }

        ExtractionResult {
            entities,
            relationships: Vec::new(),
            used_fallback: true,
            warnings,
        }
    }

    /// Converts extracted entities to graph Entity objects.
    ///
    /// # Arguments
    ///
    /// * `extracted` - The extraction result.
    ///
    /// # Returns
    ///
    /// A vector of [`Entity`] objects ready for storage.
    #[must_use]
    pub fn to_graph_entities(&self, extracted: &ExtractionResult) -> Vec<Entity> {
        extracted
            .entities
            .iter()
            .map(|e| {
                let entity_type = parse_entity_type(&e.entity_type);
                let mut entity = Entity::new(entity_type, &e.name, self.domain.clone());
                entity.confidence = e.confidence;
                entity.aliases.clone_from(&e.aliases);
                if let Some(desc) = &e.description {
                    entity
                        .properties
                        .insert("description".to_string(), desc.clone());
                }
                entity
            })
            .collect()
    }

    /// Converts extracted relationships to graph Relationship objects.
    ///
    /// Requires a mapping from entity names to entity IDs.
    ///
    /// # Arguments
    ///
    /// * `extracted` - The extraction result.
    /// * `entity_map` - Map from entity name to Entity.
    ///
    /// # Returns
    ///
    /// A vector of [`Relationship`] objects ready for storage.
    #[must_use]
    pub fn to_graph_relationships(
        &self,
        extracted: &ExtractionResult,
        entity_map: &std::collections::HashMap<String, Entity>,
    ) -> Vec<Relationship> {
        extracted
            .relationships
            .iter()
            .filter_map(|r| {
                let from_entity = entity_map.get(&r.from)?;
                let to_entity = entity_map.get(&r.to)?;
                let rel_type = parse_relationship_type(&r.relationship_type);

                let mut rel =
                    Relationship::new(from_entity.id.clone(), to_entity.id.clone(), rel_type);
                rel.confidence = r.confidence;
                if let Some(evidence) = &r.evidence {
                    rel.properties
                        .insert("evidence".to_string(), evidence.clone());
                }
                Some(rel)
            })
            .collect()
    }

    /// Infers relationships between existing entities.
    ///
    /// Analyzes a set of entities and uses LLM to discover implicit relationships
    /// that weren't explicitly stated in text.
    ///
    /// # Arguments
    ///
    /// * `entities` - The entities to analyze for relationships.
    ///
    /// # Returns
    ///
    /// An [`InferenceResult`] containing inferred relationships.
    ///
    /// # Errors
    ///
    /// Returns an error if LLM inference fails and no fallback is possible.
    pub fn infer_relationships(&self, entities: &[Entity]) -> Result<InferenceResult> {
        if entities.is_empty() {
            return Ok(InferenceResult::default());
        }

        match &self.llm {
            Some(llm) => self.infer_with_llm(llm, entities),
            None => Ok(self.infer_fallback(entities)),
        }
    }

    /// Infers relationships using LLM.
    fn infer_with_llm(
        &self,
        llm: &Arc<dyn LlmProvider>,
        entities: &[Entity],
    ) -> Result<InferenceResult> {
        let system = build_system_prompt(OperationMode::RelationshipInference, None);
        let user = self.format_entities_for_inference(entities);

        let response = match llm.complete_with_system(&system, &user) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "LLM inference failed, using fallback");
                return Ok(self.infer_fallback(entities));
            },
        };

        // Parse JSON response
        let parsed = self.parse_inference_response(&response)?;

        // Filter by confidence threshold
        let relationships: Vec<_> = parsed
            .relationships
            .into_iter()
            .filter(|r| r.confidence >= self.min_confidence)
            .collect();

        Ok(InferenceResult {
            relationships,
            used_fallback: false,
            warnings: Vec::new(),
        })
    }

    /// Formats entities for LLM inference.
    fn format_entities_for_inference(&self, entities: &[Entity]) -> String {
        use std::fmt::Write;

        let mut output = String::from("Analyze these entities for potential relationships:\n\n");

        for entity in entities {
            let _ = writeln!(
                output,
                "- {} (type: {:?}, id: {})",
                entity.name, entity.entity_type, entity.id
            );
            if !entity.aliases.is_empty() {
                let _ = writeln!(output, "  Aliases: {}", entity.aliases.join(", "));
            }
        }

        output
    }

    /// Parses the LLM JSON response for inference.
    fn parse_inference_response(&self, response: &str) -> Result<LlmInferenceResponse> {
        let json_str = self.extract_json(response);

        serde_json::from_str(&json_str).map_err(|e| {
            tracing::warn!(error = %e, response = %response, "Failed to parse inference response");
            Error::OperationFailed {
                operation: "parse_relationship_inference".to_string(),
                cause: format!("Invalid JSON response: {e}"),
            }
        })
    }

    /// Fallback inference when LLM is unavailable.
    ///
    /// Uses heuristics to infer common relationships based on entity types.
    fn infer_fallback(&self, entities: &[Entity]) -> InferenceResult {
        let mut relationships = Vec::new();
        let warnings = vec!["LLM unavailable, using heuristic-based fallback".to_string()];

        // Build entity lookup by name
        let entity_map: HashMap<&str, &Entity> =
            entities.iter().map(|e| (e.name.as_str(), e)).collect();

        // Infer common technology relationships
        let tech_deps: &[(&str, &str)] = &[
            ("Rust", "cargo"),
            ("Python", "pip"),
            ("Node.js", "npm"),
            ("Java", "Maven"),
            ("Ruby", "bundler"),
            ("Go", "go modules"),
            ("PostgreSQL", "SQL"),
            ("MySQL", "SQL"),
            ("SQLite", "SQL"),
            ("Docker", "containers"),
            ("Kubernetes", "Docker"),
        ];

        for (from, to) in tech_deps {
            if entity_map.contains_key(*from) && entity_map.contains_key(*to) {
                relationships.push(InferredRelationship {
                    from: (*from).to_string(),
                    to: (*to).to_string(),
                    relationship_type: "Uses".to_string(),
                    confidence: 0.7,
                    reasoning: Some(format!("{from} commonly uses {to}")),
                });
            }
        }

        InferenceResult {
            relationships,
            used_fallback: true,
            warnings,
        }
    }

    /// Converts inferred relationships to graph [`Relationship`] objects.
    ///
    /// # Arguments
    ///
    /// * `inferred` - The inference result.
    /// * `entity_map` - Map from entity name to Entity.
    ///
    /// # Returns
    ///
    /// A vector of [`Relationship`] objects ready for storage.
    #[must_use]
    pub fn inferred_to_graph_relationships(
        &self,
        inferred: &InferenceResult,
        entity_map: &HashMap<String, Entity>,
    ) -> Vec<Relationship> {
        inferred
            .relationships
            .iter()
            .filter_map(|r| {
                let from_entity = entity_map.get(&r.from)?;
                let to_entity = entity_map.get(&r.to)?;
                let rel_type = parse_relationship_type(&r.relationship_type);

                let mut rel =
                    Relationship::new(from_entity.id.clone(), to_entity.id.clone(), rel_type);
                rel.confidence = r.confidence;
                if let Some(reasoning) = &r.reasoning {
                    rel.properties
                        .insert("reasoning".to_string(), reasoning.clone());
                }
                Some(rel)
            })
            .collect()
    }
}

/// Parses entity type string to [`EntityType`] enum.
fn parse_entity_type(s: &str) -> EntityType {
    match s.to_lowercase().as_str() {
        "person" => EntityType::Person,
        "organization" | "org" | "company" | "team" => EntityType::Organization,
        "technology" | "tech" | "framework" | "tool" | "language" => EntityType::Technology,
        "file" | "source" | "config" => EntityType::File,
        // Default to Concept for unknown types (including "concept", "pattern", "principle")
        _ => EntityType::Concept,
    }
}

/// Parses relationship type string to [`RelationshipType`] enum.
fn parse_relationship_type(s: &str) -> RelationshipType {
    match s.to_lowercase().as_str() {
        "worksat" | "works_at" | "employedby" => RelationshipType::WorksAt,
        "created" | "authored" | "wrote" => RelationshipType::Created,
        "uses" | "utilizes" | "employs" => RelationshipType::Uses,
        "implements" | "realizes" => RelationshipType::Implements,
        "partof" | "part_of" | "belongsto" => RelationshipType::PartOf,
        "mentionedin" | "mentioned_in" => RelationshipType::MentionedIn,
        "supersedes" | "replaces" => RelationshipType::Supersedes,
        "conflictswith" | "conflicts_with" | "contradicts" => RelationshipType::ConflictsWith,
        _ => RelationshipType::RelatesTo, // Default to general relation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extraction_result_default() {
        let result = ExtractionResult::default();
        assert!(result.entities.is_empty());
        assert!(result.relationships.is_empty());
        assert!(!result.used_fallback);
    }

    #[test]
    fn test_parse_entity_type() {
        assert_eq!(parse_entity_type("Person"), EntityType::Person);
        assert_eq!(parse_entity_type("PERSON"), EntityType::Person);
        assert_eq!(parse_entity_type("Organization"), EntityType::Organization);
        assert_eq!(parse_entity_type("company"), EntityType::Organization);
        assert_eq!(parse_entity_type("Technology"), EntityType::Technology);
        assert_eq!(parse_entity_type("framework"), EntityType::Technology);
        assert_eq!(parse_entity_type("Concept"), EntityType::Concept);
        assert_eq!(parse_entity_type("File"), EntityType::File);
        assert_eq!(parse_entity_type("unknown"), EntityType::Concept);
    }

    #[test]
    fn test_parse_relationship_type() {
        assert_eq!(
            parse_relationship_type("WorksAt"),
            RelationshipType::WorksAt
        );
        assert_eq!(
            parse_relationship_type("works_at"),
            RelationshipType::WorksAt
        );
        assert_eq!(
            parse_relationship_type("Created"),
            RelationshipType::Created
        );
        assert_eq!(parse_relationship_type("Uses"), RelationshipType::Uses);
        assert_eq!(
            parse_relationship_type("Implements"),
            RelationshipType::Implements
        );
        assert_eq!(parse_relationship_type("PartOf"), RelationshipType::PartOf);
        assert_eq!(
            parse_relationship_type("Supersedes"),
            RelationshipType::Supersedes
        );
        assert_eq!(
            parse_relationship_type("ConflictsWith"),
            RelationshipType::ConflictsWith
        );
        assert_eq!(
            parse_relationship_type("unknown"),
            RelationshipType::RelatesTo
        );
    }

    #[test]
    fn test_extract_json_raw() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let json = r#"{"entities": [], "relationships": []}"#;
        assert_eq!(service.extract_json(json), json);
    }

    #[test]
    fn test_extract_json_from_markdown() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let response = r#"Here's the extraction:

```json
{"entities": [{"name": "Alice", "type": "Person"}], "relationships": []}
```

Done!"#;
        let extracted = service.extract_json(response);
        assert!(extracted.contains("Alice"));
        assert!(extracted.starts_with('{'));
    }

    #[test]
    fn test_fallback_extraction() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let result = service
            .extract("We use Rust and PostgreSQL for the backend")
            .unwrap();

        assert!(result.used_fallback);
        assert!(!result.entities.is_empty());

        let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Rust"));
        assert!(names.contains(&"PostgreSQL"));
    }

    #[test]
    fn test_fallback_no_match() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let result = service.extract("Hello world").unwrap();

        assert!(result.used_fallback);
        assert!(result.entities.is_empty());
        assert!(result.warnings.len() >= 2);
    }

    #[test]
    fn test_empty_input() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let result = service.extract("").unwrap();

        assert!(result.entities.is_empty());
        assert!(!result.used_fallback);
    }

    #[test]
    fn test_to_graph_entities() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let result = ExtractionResult {
            entities: vec![ExtractedEntity {
                name: "Alice".to_string(),
                entity_type: "Person".to_string(),
                confidence: 0.9,
                aliases: vec!["A".to_string()],
                description: Some("A person".to_string()),
            }],
            relationships: Vec::new(),
            used_fallback: false,
            warnings: Vec::new(),
        };

        let entities = service.to_graph_entities(&result);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "Alice");
        assert_eq!(entities[0].entity_type, EntityType::Person);
        assert!((entities[0].confidence - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_min_confidence_threshold() {
        let service =
            EntityExtractorService::without_llm(Domain::for_user()).with_min_confidence(0.8);
        assert!((service.min_confidence - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_inference_result_default() {
        let result = InferenceResult::default();
        assert!(result.relationships.is_empty());
        assert!(!result.used_fallback);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_infer_relationships_empty() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let result = service.infer_relationships(&[]).unwrap();

        assert!(result.relationships.is_empty());
        assert!(!result.used_fallback);
    }

    #[test]
    fn test_infer_fallback_with_matching_entities() {
        let service = EntityExtractorService::without_llm(Domain::for_user());

        let entities = vec![
            Entity::new(EntityType::Technology, "Rust", Domain::for_user()),
            Entity::new(EntityType::Technology, "cargo", Domain::for_user()),
        ];

        let result = service.infer_relationships(&entities).unwrap();

        assert!(result.used_fallback);
        assert_eq!(result.relationships.len(), 1);
        assert_eq!(result.relationships[0].from, "Rust");
        assert_eq!(result.relationships[0].to, "cargo");
        assert_eq!(result.relationships[0].relationship_type, "Uses");
    }

    #[test]
    fn test_infer_fallback_no_matching_pairs() {
        let service = EntityExtractorService::without_llm(Domain::for_user());

        let entities = vec![
            Entity::new(EntityType::Person, "Alice", Domain::for_user()),
            Entity::new(EntityType::Organization, "Acme", Domain::for_user()),
        ];

        let result = service.infer_relationships(&entities).unwrap();

        assert!(result.used_fallback);
        assert!(result.relationships.is_empty());
    }

    #[test]
    fn test_format_entities_for_inference() {
        let service = EntityExtractorService::without_llm(Domain::for_user());

        let mut entity = Entity::new(EntityType::Technology, "Rust", Domain::for_user());
        entity.aliases = vec!["rust-lang".to_string()];

        let formatted = service.format_entities_for_inference(&[entity]);

        assert!(formatted.contains("Rust"));
        assert!(formatted.contains("Technology"));
        assert!(formatted.contains("rust-lang"));
    }

    #[test]
    fn test_inferred_to_graph_relationships() {
        let service = EntityExtractorService::without_llm(Domain::for_user());

        let rust = Entity::new(EntityType::Technology, "Rust", Domain::for_user());
        let cargo = Entity::new(EntityType::Technology, "cargo", Domain::for_user());

        let mut entity_map = HashMap::new();
        entity_map.insert("Rust".to_string(), rust.clone());
        entity_map.insert("cargo".to_string(), cargo.clone());

        let inferred = InferenceResult {
            relationships: vec![InferredRelationship {
                from: "Rust".to_string(),
                to: "cargo".to_string(),
                relationship_type: "Uses".to_string(),
                confidence: 0.8,
                reasoning: Some("Rust uses cargo as package manager".to_string()),
            }],
            used_fallback: false,
            warnings: Vec::new(),
        };

        let relationships = service.inferred_to_graph_relationships(&inferred, &entity_map);

        assert_eq!(relationships.len(), 1);
        assert_eq!(relationships[0].from_entity, rust.id);
        assert_eq!(relationships[0].to_entity, cargo.id);
        assert_eq!(relationships[0].relationship_type, RelationshipType::Uses);
        assert!(relationships[0].properties.contains_key("reasoning"));
    }

    #[test]
    fn test_inferred_to_graph_missing_entity() {
        let service = EntityExtractorService::without_llm(Domain::for_user());

        let rust = Entity::new(EntityType::Technology, "Rust", Domain::for_user());
        let mut entity_map = HashMap::new();
        entity_map.insert("Rust".to_string(), rust);
        // Note: "cargo" is missing from entity_map

        let inferred = InferenceResult {
            relationships: vec![InferredRelationship {
                from: "Rust".to_string(),
                to: "cargo".to_string(),
                relationship_type: "Uses".to_string(),
                confidence: 0.8,
                reasoning: None,
            }],
            used_fallback: false,
            warnings: Vec::new(),
        };

        let relationships = service.inferred_to_graph_relationships(&inferred, &entity_map);

        // Should skip relationships with missing entities
        assert!(relationships.is_empty());
    }

    #[test]
    fn test_to_graph_relationships() {
        let service = EntityExtractorService::without_llm(Domain::for_user());

        let result = ExtractionResult {
            entities: vec![
                ExtractedEntity {
                    name: "Alice".to_string(),
                    entity_type: "Person".to_string(),
                    confidence: 0.9,
                    aliases: Vec::new(),
                    description: None,
                },
                ExtractedEntity {
                    name: "Acme".to_string(),
                    entity_type: "Organization".to_string(),
                    confidence: 0.85,
                    aliases: Vec::new(),
                    description: None,
                },
            ],
            relationships: vec![ExtractedRelationship {
                from: "Alice".to_string(),
                to: "Acme".to_string(),
                relationship_type: "WorksAt".to_string(),
                confidence: 0.8,
                evidence: None,
            }],
            used_fallback: false,
            warnings: Vec::new(),
        };

        let entities = service.to_graph_entities(&result);
        // Create entity_map from entities Vec
        let entity_map: HashMap<String, Entity> =
            entities.into_iter().map(|e| (e.name.clone(), e)).collect();
        let relationships = service.to_graph_relationships(&result, &entity_map);

        assert_eq!(relationships.len(), 1);
        assert_eq!(
            relationships[0].relationship_type,
            RelationshipType::WorksAt
        );
    }

    #[test]
    fn test_extraction_with_various_technologies() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let result = service
            .extract("We built this using React, TypeScript, and Docker containers")
            .unwrap();

        assert!(result.used_fallback);
        let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"React"));
        assert!(names.contains(&"TypeScript"));
        assert!(names.contains(&"Docker"));
    }

    #[test]
    fn test_extraction_with_databases() {
        let service = EntityExtractorService::without_llm(Domain::for_user());
        let result = service
            .extract("Our stack uses PostgreSQL for persistence and Redis for caching")
            .unwrap();

        assert!(result.used_fallback);
        let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"PostgreSQL"));
        assert!(names.contains(&"Redis"));
    }

    #[test]
    fn test_extracted_entity_defaults() {
        let entity = ExtractedEntity {
            name: "Test".to_string(),
            entity_type: "Concept".to_string(),
            confidence: 0.5,
            aliases: Vec::new(),
            description: None,
        };

        assert_eq!(entity.name, "Test");
        assert!(entity.aliases.is_empty());
        assert!(entity.description.is_none());
    }

    #[test]
    fn test_inferred_relationship_with_reasoning() {
        let rel = InferredRelationship {
            from: "Rust".to_string(),
            to: "LLVM".to_string(),
            relationship_type: "Uses".to_string(),
            confidence: 0.9,
            reasoning: Some("Rust compiles through LLVM".to_string()),
        };

        assert_eq!(rel.from, "Rust");
        assert_eq!(rel.to, "LLVM");
        assert!(rel.reasoning.is_some());
    }

    #[test]
    fn test_service_domain() {
        let user_domain = Domain::for_user();
        let service = EntityExtractorService::without_llm(user_domain);

        // Verify domain is set correctly by extracting an entity
        let result = service.extract("Using Python for scripting").unwrap();
        let entities = service.to_graph_entities(&result);

        if !entities.is_empty() {
            assert!(entities[0].domain.is_user());
        }
    }

    /// Integration test that actually calls the LLM API.
    /// Run with: RUST_LOG=debug cargo test test_llm_extraction_integration -- --ignored --nocapture
    #[test]
    #[ignore = "requires OPENAI_API_KEY and makes real API calls"]
    fn test_llm_extraction_integration() {
        use crate::llm::{LlmHttpConfig, LlmProvider, OpenAiClient, build_http_client};
        use std::sync::Arc;

        // Initialize logging for debug output
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        // Check for API key
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
        assert!(!api_key.is_empty(), "OPENAI_API_KEY cannot be empty");

        // Build client with longer timeout for debugging
        let http_config = LlmHttpConfig {
            timeout_ms: 60_000,
            connect_timeout_ms: 10_000,
        };
        let client = OpenAiClient::new()
            .with_api_key(&api_key)
            .with_model("gpt-5-nano-2025-08-07")
            .with_http_config(http_config);

        let llm: Arc<dyn LlmProvider> = Arc::new(client);

        let service = EntityExtractorService::with_shared_llm(llm, Domain::for_user());

        // Test 1: Simple content (should work)
        println!("\n=== Test 1: Simple content ===");
        let simple_content = "PostgreSQL database with Redis cache";
        let result = service.extract(simple_content);
        match &result {
            Ok(r) => {
                println!("Simple content result: used_fallback={}, entities={:?}", r.used_fallback, r.entities);
                assert!(!r.used_fallback, "Simple content should use LLM, not fallback");
            }
            Err(e) => {
                println!("Simple content error: {e:?}");
                panic!("Simple content extraction failed: {e}");
            }
        }

        // Test 2: Complex code-heavy content (might trigger fallback)
        println!("\n=== Test 2: Complex content ===");
        let complex_content = r#"The EntityExtractorService::extract_with_llm() method at src/services/entity_extraction.rs:312 calls llm.complete_with_system(&system, &user) to process text. If the LLM fails, it falls back to extract_fallback() which uses TECH_PATTERNS regex matching against patterns like r"\b(Rust|Python|Go|Java)\b" defined in the static LAZY_STATIC block."#;
        let result = service.extract(complex_content);
        match &result {
            Ok(r) => {
                println!("Complex content result: used_fallback={}, entities={:?}, warnings={:?}",
                    r.used_fallback, r.entities, r.warnings);
                if r.used_fallback {
                    println!("WARNING: Complex content fell back to pattern matching!");
                }
            }
            Err(e) => {
                println!("Complex content error: {e:?}");
            }
        }
    }
}

//! MCP resource handlers.
//!
//! Provides resource access for the Model Context Protocol.
//! Resources are accessed via URN scheme.
//!
//! # URN Format Specification
//!
//! ```text
//! subcog://{domain}/{resource-type}[/{resource-id}]
//! ```
//!
//! ## Components
//!
//! | Component | Format | Description |
//! |-----------|--------|-------------|
//! | `domain` | `_` \| `project` \| `user` \| `org/{name}` | Scope for resolution |
//! | `resource-type` | `help` \| `memory` \| `search` \| `topics` \| `namespaces` | Type of resource |
//! | `resource-id` | alphanumeric with `-`, `_` | Optional identifier |
//!
//! ## Domain Scopes
//!
//! | Domain | Description |
//! |--------|-------------|
//! | `_` | Wildcard - all domains combined |
//! | `project` | Current project/repository (default) |
//! | `user` | User-specific (e.g., `~/.subcog/`) |
//! | `org/{name}` | Organization namespace |
//!
//! # Resource Types
//!
//! ## Help Resources
//! - `subcog://help` - Help index with all available topics
//! - `subcog://help/{topic}` - Topic-specific help (setup, concepts, capture, recall, etc.)
//!
//! ## Memory Resources
//! - `subcog://_` - All memories across all domains
//! - `subcog://_/{namespace}` - All memories in a namespace (e.g., `subcog://_/learnings`)
//! - `subcog://memory/{id}` - Get a specific memory by its unique ID
//! - `subcog://project/decisions/{id}` - Fully-qualified memory URN
//!
//! ## Search & Topic Resources
//! - `subcog://search/{query}` - Search memories with a query (URL-encoded)
//! - `subcog://topics` - List all indexed topics with memory counts
//! - `subcog://topics/{topic}` - Get memories for a specific topic
//! - `subcog://namespaces` - List all namespaces with descriptions and signal words
//!
//! ## Domain-Scoped Resources
//! - `subcog://project/_` - Project-scoped memories only
//! - `subcog://org/{org}/_` - Organization-scoped memories
//! - `subcog://user/_` - User-scoped memories
//!
//! # Examples
//!
//! ```text
//! subcog://help/capture          # Get capture help
//! subcog://_/decisions           # All decisions across domains
//! subcog://project/learnings     # Project learnings only
//! subcog://memory/abc123         # Specific memory by ID
//! subcog://search/postgres       # Search for "postgres"
//! subcog://topics/authentication # Memories about authentication
//! ```
//!
//! For advanced filtering and discovery, use the `subcog_browse` prompt
//! which supports filtering by namespace, tags, time, source, and status.

use super::help_content;
use crate::Namespace;
use crate::models::SearchMode;
use crate::services::{PromptService, RecallService, TopicIndexService};
use crate::storage::index::DomainScope;
use crate::{Error, Result, SearchFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Handler for MCP resources (URN scheme).
pub struct ResourceHandler {
    /// Help content by category.
    help_content: HashMap<String, HelpCategory>,
    /// Optional recall service for memory browsing.
    recall_service: Option<RecallService>,
    /// Topic index for topic-based resource access.
    topic_index: Option<TopicIndexService>,
    /// Optional prompt service for prompt resources.
    prompt_service: Option<PromptService>,
}

impl ResourceHandler {
    /// Creates a new resource handler.
    #[must_use]
    pub fn new() -> Self {
        let mut help_content = HashMap::new();

        // Setup category
        help_content.insert(
            "setup".to_string(),
            HelpCategory {
                name: "setup".to_string(),
                title: "Getting Started with Subcog".to_string(),
                description: "Installation and initial configuration guide".to_string(),
                content: help_content::SETUP.to_string(),
            },
        );

        // Concepts category
        help_content.insert(
            "concepts".to_string(),
            HelpCategory {
                name: "concepts".to_string(),
                title: "Core Concepts".to_string(),
                description: "Understanding namespaces, domains, URNs, and memory lifecycle"
                    .to_string(),
                content: help_content::CONCEPTS.to_string(),
            },
        );

        // Capture category
        help_content.insert(
            "capture".to_string(),
            HelpCategory {
                name: "capture".to_string(),
                title: "Capturing Memories".to_string(),
                description: "How to capture and store memories effectively".to_string(),
                content: help_content::CAPTURE.to_string(),
            },
        );

        // Search category
        help_content.insert(
            "search".to_string(),
            HelpCategory {
                name: "search".to_string(),
                title: "Searching Memories".to_string(),
                description: "Using hybrid search to find relevant memories".to_string(),
                content: help_content::SEARCH.to_string(),
            },
        );

        // Workflows category
        help_content.insert(
            "workflows".to_string(),
            HelpCategory {
                name: "workflows".to_string(),
                title: "Integration Workflows".to_string(),
                description: "Hooks, MCP server, and IDE integration".to_string(),
                content: help_content::WORKFLOWS.to_string(),
            },
        );

        // Troubleshooting category
        help_content.insert(
            "troubleshooting".to_string(),
            HelpCategory {
                name: "troubleshooting".to_string(),
                title: "Troubleshooting".to_string(),
                description: "Common issues and solutions".to_string(),
                content: help_content::TROUBLESHOOTING.to_string(),
            },
        );

        // Advanced category
        help_content.insert(
            "advanced".to_string(),
            HelpCategory {
                name: "advanced".to_string(),
                title: "Advanced Features".to_string(),
                description: "LLM integration, consolidation, and optimization".to_string(),
                content: help_content::ADVANCED.to_string(),
            },
        );

        // Prompts category
        help_content.insert(
            "prompts".to_string(),
            HelpCategory {
                name: "prompts".to_string(),
                title: "User-Defined Prompts".to_string(),
                description: "Save, manage, and run prompt templates with variables".to_string(),
                content: help_content::PROMPTS.to_string(),
            },
        );

        Self {
            help_content,
            recall_service: None,
            topic_index: None,
            prompt_service: None,
        }
    }

    /// Adds a prompt service to the resource handler.
    #[must_use]
    pub fn with_prompt_service(mut self, prompt_service: PromptService) -> Self {
        self.prompt_service = Some(prompt_service);
        self
    }

    /// Adds a recall service to the resource handler.
    #[must_use]
    pub fn with_recall_service(mut self, recall_service: RecallService) -> Self {
        self.recall_service = Some(recall_service);
        self
    }

    /// Adds a topic index to the resource handler.
    #[must_use]
    pub fn with_topic_index(mut self, topic_index: TopicIndexService) -> Self {
        self.topic_index = Some(topic_index);
        self
    }

    /// Builds and refreshes the topic index from the recall service.
    ///
    /// # Errors
    ///
    /// Returns an error if the topic index cannot be built.
    pub fn refresh_topic_index(&mut self) -> Result<()> {
        let recall = self.recall_service.as_ref().ok_or_else(|| {
            Error::InvalidInput("Topic indexing requires RecallService".to_string())
        })?;

        let topic_index = self.topic_index.get_or_insert_with(TopicIndexService::new);
        topic_index.build_index(recall)
    }

    /// Lists all available resources.
    ///
    /// Returns resources organized by type:
    /// - Help topics
    /// - Memory browsing patterns
    ///
    /// For advanced filtering, use the `subcog_browse` prompt.
    #[must_use]
    pub fn list_resources(&self) -> Vec<ResourceDefinition> {
        let mut resources = Vec::new();
        resources.extend(self.help_resource_definitions());
        resources.extend(self.memory_resource_definitions());
        resources.extend(Self::domain_resource_definitions());
        resources.extend(Self::search_topic_resource_definitions());
        resources.extend(Self::prompt_resource_definitions());
        resources
    }

    fn build_resource(
        uri: &str,
        name: &str,
        description: &str,
        mime_type: &str,
    ) -> ResourceDefinition {
        ResourceDefinition {
            uri: uri.to_string(),
            name: name.to_string(),
            description: Some(description.to_string()),
            mime_type: Some(mime_type.to_string()),
        }
    }

    fn help_resource_definitions(&self) -> Vec<ResourceDefinition> {
        let mut resources = vec![Self::build_resource(
            "subcog://help",
            "Help Index",
            "Help index with all available topics",
            "text/markdown",
        )];

        resources.extend(self.help_content.values().map(|cat| ResourceDefinition {
            uri: format!("subcog://help/{}", cat.name),
            name: cat.title.clone(),
            description: Some(cat.description.clone()),
            mime_type: Some("text/markdown".to_string()),
        }));

        resources
    }

    fn memory_resource_definitions(&self) -> Vec<ResourceDefinition> {
        let mut resources = vec![Self::build_resource(
            "subcog://_",
            "All Memories",
            "All memories across all domains",
            "application/json",
        )];

        for ns in Namespace::user_namespaces() {
            let ns_str = ns.as_str();
            resources.push(Self::build_resource(
                &format!("subcog://_/{ns_str}"),
                &format!("{ns_str} memories"),
                &format!("All memories in {ns_str} namespace"),
                "application/json",
            ));
        }

        resources.push(Self::build_resource(
            "subcog://memory/{id}",
            "Memory by ID",
            "Fetch a specific memory by ID",
            "application/json",
        ));

        resources
    }

    fn domain_resource_definitions() -> Vec<ResourceDefinition> {
        let items = [
            (
                "subcog://project",
                "Project Memories",
                "Project-scoped memories",
            ),
            (
                "subcog://project/_",
                "Project Memories (All Namespaces)",
                "Project memories, all namespaces",
            ),
            (
                "subcog://project/{namespace}",
                "Project Namespace",
                "Project memories by namespace",
            ),
            (
                "subcog://project/{namespace}/{id}",
                "Project Memory",
                "Fetch a project memory by ID",
            ),
            ("subcog://user", "User Memories", "User-scoped memories"),
            (
                "subcog://user/_",
                "User Memories (All Namespaces)",
                "User memories, all namespaces",
            ),
            (
                "subcog://user/{namespace}",
                "User Namespace",
                "User memories by namespace",
            ),
            (
                "subcog://user/{namespace}/{id}",
                "User Memory",
                "Fetch a user memory by ID",
            ),
            (
                "subcog://org",
                "Org Memories",
                "Organization-scoped memories",
            ),
            (
                "subcog://org/_",
                "Org Memories (All Namespaces)",
                "Org memories, all namespaces",
            ),
            (
                "subcog://org/{namespace}",
                "Org Namespace",
                "Org memories by namespace",
            ),
            (
                "subcog://org/{namespace}/{id}",
                "Org Memory",
                "Fetch an org memory by ID",
            ),
        ];

        items
            .iter()
            .map(|(uri, name, desc)| Self::build_resource(uri, name, desc, "application/json"))
            .collect()
    }

    fn search_topic_resource_definitions() -> Vec<ResourceDefinition> {
        let mut resources = vec![
            Self::build_resource(
                "subcog://search/{query}",
                "Search Memories",
                "Search memories with a query (replace {query})",
                "application/json",
            ),
            Self::build_resource(
                "subcog://topics",
                "All Topics",
                "List all indexed topics with memory counts",
                "application/json",
            ),
            Self::build_resource(
                "subcog://topics/{topic}",
                "Topic Memories",
                "Get memories for a specific topic (replace {topic})",
                "application/json",
            ),
        ];

        resources.push(Self::build_resource(
            "subcog://namespaces",
            "All Namespaces",
            "List all memory namespaces with descriptions and signal words",
            "application/json",
        ));

        resources
    }

    fn prompt_resource_definitions() -> Vec<ResourceDefinition> {
        let items = [
            (
                "subcog://_prompts",
                "All Prompts",
                "Aggregate prompts from all domains (project, user, org)",
            ),
            (
                "subcog://project/_prompts",
                "Project Prompts",
                "List all prompts in the project scope",
            ),
            (
                "subcog://user/_prompts",
                "User Prompts",
                "List all prompts in the user scope",
            ),
            (
                "subcog://project/_prompts/{name}",
                "Project Prompt",
                "Get a specific prompt by name from project scope",
            ),
            (
                "subcog://user/_prompts/{name}",
                "User Prompt",
                "Get a specific prompt by name from user scope",
            ),
            (
                "subcog://org/_prompts",
                "Org Prompts",
                "List all prompts in the org scope",
            ),
            (
                "subcog://org/_prompts/{name}",
                "Org Prompt",
                "Get a specific prompt by name from org scope",
            ),
        ];

        items
            .iter()
            .map(|(uri, name, desc)| Self::build_resource(uri, name, desc, "application/json"))
            .collect()
    }

    /// Gets a resource by URI.
    ///
    /// Supported URI patterns:
    /// - `subcog://help` - Help index
    /// - `subcog://help/{topic}` - Help topic
    /// - `subcog://_` - All memories across all domains
    /// - `subcog://_/{namespace}` - All memories in a namespace
    /// - `subcog://memory/{id}` - Get specific memory by ID
    /// - `subcog://project/_` - Project-scoped memories (alias for `subcog://_`)
    /// - `subcog://search/{query}` - Search memories with a query
    /// - `subcog://topics` - List all indexed topics
    /// - `subcog://topics/{topic}` - Get memories for a specific topic
    ///
    /// For advanced filtering, use the `subcog_browse` prompt instead.
    ///
    /// # Errors
    ///
    /// Returns an error if the resource is not found.
    pub fn get_resource(&mut self, uri: &str) -> Result<ResourceContent> {
        let uri = uri.trim();

        if !uri.starts_with("subcog://") {
            return Err(Error::InvalidInput(format!("Invalid URI scheme: {uri}")));
        }

        let path = &uri["subcog://".len()..];
        let parts: Vec<&str> = path.split('/').collect();

        if parts.is_empty() {
            return Err(Error::InvalidInput("Empty resource path".to_string()));
        }

        match parts[0] {
            "help" => self.get_help_resource(uri, &parts),
            "_" => self.get_all_memories_resource(uri, &parts),
            "project" => self.get_domain_scoped_resource(uri, &parts, DomainScope::Project),
            "user" => self.get_domain_scoped_resource(uri, &parts, DomainScope::User),
            "org" => self.get_domain_scoped_resource(uri, &parts, DomainScope::Org),
            "memory" => self.get_memory_resource(uri, &parts),
            "search" => self.get_search_resource(uri, &parts),
            "topics" => self.get_topics_resource(uri, &parts),
            "namespaces" => self.get_namespaces_resource(uri),
            "_prompts" => self.get_aggregate_prompts_resource(uri),
            _ => Err(Error::InvalidInput(format!(
                "Unknown resource type: {}. Valid: _, help, memory, project, user, org, search, topics, namespaces, _prompts",
                parts[0]
            ))),
        }
    }

    /// Gets a help resource.
    fn get_help_resource(&self, uri: &str, parts: &[&str]) -> Result<ResourceContent> {
        if parts.len() == 1 {
            // Return help index
            return Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("text/markdown".to_string()),
                text: Some(self.get_help_index()),
                blob: None,
            });
        }

        let category = parts[1];
        let content = self
            .help_content
            .get(category)
            .ok_or_else(|| Error::InvalidInput(format!("Unknown help category: {category}")))?;

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("text/markdown".to_string()),
            text: Some(format!("# {}\n\n{}", content.title, content.content)),
            blob: None,
        })
    }

    /// Gets all memories resource with optional namespace filter.
    ///
    /// URI patterns:
    /// - `subcog://_` - All memories across all domains
    /// - `subcog://_/{namespace}` - All memories in a namespace
    /// - `subcog://project/_` - Alias for `subcog://_` (project-scoped, future domain filter)
    ///
    /// For advanced filtering, use the `subcog_browse` prompt.
    fn get_all_memories_resource(&self, uri: &str, parts: &[&str]) -> Result<ResourceContent> {
        // Parse namespace filter from URI
        // subcog://_ -> no filter
        // subcog://_/learnings -> filter by namespace
        // subcog://project/_ -> no filter (legacy)
        let namespace_filter = if parts[0] == "_" && parts.len() >= 2 {
            Some(parts[1])
        } else {
            None
        };

        // Build filter
        let mut filter = SearchFilter::new();
        if let Some(ns_str) = namespace_filter {
            let ns = Namespace::parse(ns_str)
                .ok_or_else(|| Error::InvalidInput(format!("Unknown namespace: {ns_str}")))?;
            filter = filter.with_namespace(ns);
        }
        self.list_memories(uri, &filter)
    }

    /// Gets a specific memory by ID with full content (cross-domain lookup).
    ///
    /// This is the targeted fetch endpoint - returns complete memory data.
    /// Use `subcog://memory/{id}` for cross-domain lookups when ID is known.
    fn get_memory_resource(&self, uri: &str, parts: &[&str]) -> Result<ResourceContent> {
        use crate::models::MemoryId;

        if parts.len() < 2 {
            return Err(Error::InvalidInput(
                "Memory ID required: subcog://memory/{id}".to_string(),
            ));
        }

        let memory_id = parts[1];
        let recall = self.recall_service.as_ref().ok_or_else(|| {
            Error::InvalidInput("Memory browsing requires RecallService".to_string())
        })?;

        // Direct fetch by ID - returns full content
        let memory = recall
            .get_by_id(&MemoryId::new(memory_id))?
            .ok_or_else(|| Error::InvalidInput(format!("Memory not found: {memory_id}")))?;

        self.format_memory_response(uri, &memory)
    }

    /// Gets memories scoped to a namespace.
    fn get_namespace_memories_resource(
        &self,
        uri: &str,
        namespace: &str,
    ) -> Result<ResourceContent> {
        let ns = Namespace::parse(namespace)
            .ok_or_else(|| Error::InvalidInput(format!("Unknown namespace: {namespace}")))?;
        let filter = SearchFilter::new().with_namespace(ns);
        self.list_memories(uri, &filter)
    }

    /// Gets a specific memory by ID with namespace validation.
    fn get_scoped_memory_resource(
        &self,
        uri: &str,
        namespace: &str,
        memory_id: &str,
    ) -> Result<ResourceContent> {
        use crate::models::MemoryId;

        let recall = self.recall_service.as_ref().ok_or_else(|| {
            Error::InvalidInput("Memory browsing requires RecallService".to_string())
        })?;

        let memory = recall
            .get_by_id(&MemoryId::new(memory_id))?
            .ok_or_else(|| Error::InvalidInput(format!("Memory not found: {memory_id}")))?;

        if memory.namespace.as_str() != namespace {
            return Err(Error::InvalidInput(format!(
                "Memory {memory_id} is in namespace {}, not {namespace}",
                memory.namespace.as_str()
            )));
        }

        self.format_memory_response(uri, &memory)
    }

    /// Formats a memory as a JSON response.
    fn format_memory_response(
        &self,
        uri: &str,
        memory: &crate::models::Memory,
    ) -> Result<ResourceContent> {
        let response = serde_json::json!({
            "id": memory.id.as_str(),
            "namespace": memory.namespace.as_str(),
            "domain": memory.domain.to_string(),
            "content": memory.content,
            "tags": memory.tags,
            "source": memory.source,
            "status": memory.status.as_str(),
            "created_at": memory.created_at,
            "updated_at": memory.updated_at,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    fn list_memories(&self, uri: &str, filter: &SearchFilter) -> Result<ResourceContent> {
        let recall = self.recall_service.as_ref().ok_or_else(|| {
            Error::InvalidInput("Memory browsing requires RecallService".to_string())
        })?;

        let results = recall.list_all(filter, 500)?;

        // Bare minimum for informed selection: id, ns, tags, uri
        let memories: Vec<serde_json::Value> = results
            .memories
            .iter()
            .map(|hit| {
                serde_json::json!({
                    "id": hit.memory.id.as_str(),
                    "ns": hit.memory.namespace.as_str(),
                    "tags": hit.memory.tags,
                    "uri": format!("subcog://memory/{}", hit.memory.id.as_str()),
                })
            })
            .collect();

        let response = serde_json::json!({
            "count": memories.len(),
            "memories": memories,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    /// Searches memories and returns results.
    ///
    /// URI: `subcog://search/{query}`
    fn get_search_resource(&self, uri: &str, parts: &[&str]) -> Result<ResourceContent> {
        if parts.len() < 2 {
            return Err(Error::InvalidInput(
                "Search query required: subcog://search/<query>".to_string(),
            ));
        }

        let recall = self
            .recall_service
            .as_ref()
            .ok_or_else(|| Error::InvalidInput("Search requires RecallService".to_string()))?;

        // URL-decode the query (simple: replace + with space, handle %20)
        let query = parts[1..].join("/");
        let query = decode_uri_component(&query);

        // Perform search with hybrid mode
        let filter = SearchFilter::new();
        let results = recall.search(&query, SearchMode::Hybrid, &filter, 20)?;

        // Build response
        let memories: Vec<serde_json::Value> = results
            .memories
            .iter()
            .map(|hit| {
                serde_json::json!({
                    "id": hit.memory.id.as_str(),
                    "namespace": hit.memory.namespace.as_str(),
                    "score": hit.score,
                    "tags": hit.memory.tags,
                    "content_preview": truncate_content(&hit.memory.content, 200),
                    "uri": format!("subcog://memory/{}", hit.memory.id.as_str()),
                })
            })
            .collect();

        let response = serde_json::json!({
            "query": query,
            "count": memories.len(),
            "mode": "hybrid",
            "memories": memories,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    /// Gets topics resource (list or specific topic).
    ///
    /// URIs:
    /// - `subcog://topics` - List all topics
    /// - `subcog://topics/{topic}` - Get memories for a topic
    fn get_topics_resource(&self, uri: &str, parts: &[&str]) -> Result<ResourceContent> {
        let topic_index = self
            .topic_index
            .as_ref()
            .ok_or_else(|| Error::InvalidInput("Topic index not initialized".to_string()))?;

        if parts.len() == 1 {
            // List all topics
            let topics = topic_index.list_topics()?;

            let topics_json: Vec<serde_json::Value> = topics
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "memory_count": t.memory_count,
                        "namespaces": t.namespaces.iter().map(Namespace::as_str).collect::<Vec<_>>(),
                        "uri": format!("subcog://topics/{}", t.name),
                    })
                })
                .collect();

            let response = serde_json::json!({
                "count": topics_json.len(),
                "topics": topics_json,
            });

            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
                blob: None,
            })
        } else {
            // Get memories for specific topic
            let topic = parts[1..].join("/");
            let topic = decode_uri_component(&topic);

            let memory_ids = topic_index.get_topic_memories(&topic)?;

            if memory_ids.is_empty() {
                return Err(Error::InvalidInput(format!("Topic not found: {topic}")));
            }

            // Get topic info
            let topic_info = topic_index.get_topic_info(&topic)?;

            // Fetch full memories if recall service available
            let memories: Vec<serde_json::Value> = if let Some(recall) = &self.recall_service {
                memory_ids
                    .iter()
                    .filter_map(|id| recall.get_by_id(id).ok().flatten())
                    .map(|m| format_memory_preview(&m))
                    .collect()
            } else {
                // Return just IDs if no recall service
                memory_ids.iter().map(format_memory_id_only).collect()
            };

            let response = serde_json::json!({
                "topic": topic,
                "memory_count": topic_info.as_ref().map_or(memory_ids.len(), |i| i.memory_count),
                "namespaces": topic_info.as_ref().map_or_else(Vec::new, |i| {
                    i.namespaces.iter().map(Namespace::as_str).collect::<Vec<_>>()
                }),
                "memories": memories,
            });

            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
                blob: None,
            })
        }
    }

    /// Gets namespaces resource listing all available namespaces.
    ///
    /// URI: `subcog://namespaces`
    ///
    /// Returns namespace definitions with descriptions and signal words.
    fn get_namespaces_resource(&self, uri: &str) -> Result<ResourceContent> {
        use crate::cli::get_all_namespaces;

        let namespaces = get_all_namespaces();

        let namespaces_json: Vec<serde_json::Value> = namespaces
            .iter()
            .map(|ns| {
                serde_json::json!({
                    "namespace": ns.namespace,
                    "description": ns.description,
                    "signal_words": ns.signal_words,
                })
            })
            .collect();

        let response = serde_json::json!({
            "count": namespaces_json.len(),
            "namespaces": namespaces_json,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    /// Gets aggregate prompts resource listing prompts from all domains.
    ///
    /// URI: `subcog://_prompts`
    ///
    /// Returns prompts aggregated from project, user, and org domains.
    /// Prompts are deduplicated by name, with project scope taking priority.
    fn get_aggregate_prompts_resource(&mut self, uri: &str) -> Result<ResourceContent> {
        use crate::services::PromptFilter;
        use std::collections::HashSet;

        let prompt_service = self.prompt_service.as_mut().ok_or_else(|| {
            Error::InvalidInput("Prompt browsing requires PromptService".to_string())
        })?;

        // Collect prompts from all domains, deduplicating by name
        // Priority: project > user > org (first seen wins)
        let mut seen_names: HashSet<String> = HashSet::new();
        let mut prompts_json: Vec<serde_json::Value> = Vec::new();

        let domains = [
            (DomainScope::Project, "project"),
            (DomainScope::User, "user"),
            (DomainScope::Org, "org"),
        ];

        for (domain, domain_name) in domains {
            let filter = PromptFilter::new().with_domain(domain);
            let prompts = prompt_service.list(&filter).unwrap_or_default();
            Self::collect_unique_prompts(&mut seen_names, &mut prompts_json, prompts, domain_name);
        }

        let response = serde_json::json!({
            "count": prompts_json.len(),
            "prompts": prompts_json,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    /// Collects unique prompts, skipping those already seen.
    fn collect_unique_prompts(
        seen: &mut std::collections::HashSet<String>,
        output: &mut Vec<serde_json::Value>,
        prompts: Vec<crate::models::PromptTemplate>,
        domain_name: &str,
    ) {
        for p in prompts {
            if seen.contains(&p.name) {
                continue;
            }
            seen.insert(p.name.clone());
            output.push(serde_json::json!({
                "name": p.name,
                "description": p.description,
                "domain": domain_name,
                "tags": p.tags,
                "usage_count": p.usage_count,
                "variables": p.variables.iter().map(|v| v.name.clone()).collect::<Vec<_>>(),
            }));
        }
    }

    /// Gets domain-scoped resources (prompts or memories).
    ///
    /// URIs:
    /// - `subcog://{domain}/_prompts` - List prompts in domain
    /// - `subcog://{domain}/_prompts/{name}` - Get specific prompt by name
    /// - `subcog://{domain}/_` - List memories in domain (alias)
    fn get_domain_scoped_resource(
        &mut self,
        uri: &str,
        parts: &[&str],
        domain: DomainScope,
    ) -> Result<ResourceContent> {
        // Check if requesting prompts
        if parts.len() >= 2 && parts[1] == "_prompts" {
            // Check for specific prompt name
            if parts.len() >= 3 {
                let name = parts[2..].join("/");
                let name = decode_uri_component(&name);
                return self.get_single_prompt_resource(uri, domain, &name);
            }
            return self.get_prompts_resource(uri, domain);
        }

        if parts.len() == 1 {
            return self.get_all_memories_resource(uri, parts);
        }

        let namespace = decode_uri_component(parts[1]);
        if namespace == "_" {
            return self.get_all_memories_resource(uri, parts);
        }

        if parts.len() >= 3 {
            let memory_id = decode_uri_component(&parts[2..].join("/"));
            return self.get_scoped_memory_resource(uri, &namespace, &memory_id);
        }

        self.get_namespace_memories_resource(uri, &namespace)
    }

    /// Gets prompts for a specific domain scope.
    fn get_prompts_resource(&mut self, uri: &str, domain: DomainScope) -> Result<ResourceContent> {
        use crate::services::PromptFilter;

        let prompt_service = self.prompt_service.as_mut().ok_or_else(|| {
            Error::InvalidInput("Prompt browsing requires PromptService".to_string())
        })?;

        // Build filter for the specific domain
        let filter = PromptFilter::new().with_domain(domain);
        let prompts = prompt_service.list(&filter)?;

        let prompts_json: Vec<serde_json::Value> = prompts
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "description": p.description,
                    "tags": p.tags,
                    "usage_count": p.usage_count,
                    "variables": p.variables.iter().map(|v| v.name.clone()).collect::<Vec<_>>(),
                })
            })
            .collect();

        let response = serde_json::json!({
            "domain": domain.as_str(),
            "count": prompts_json.len(),
            "prompts": prompts_json,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    /// Gets a specific prompt by name from a domain scope.
    fn get_single_prompt_resource(
        &mut self,
        uri: &str,
        domain: DomainScope,
        name: &str,
    ) -> Result<ResourceContent> {
        let prompt_service = self.prompt_service.as_mut().ok_or_else(|| {
            Error::InvalidInput("Prompt browsing requires PromptService".to_string())
        })?;

        // Get the prompt from the specific domain
        let prompt = prompt_service.get(name, Some(domain))?.ok_or_else(|| {
            Error::InvalidInput(format!(
                "Prompt not found: {} in {} scope",
                name,
                domain.as_str()
            ))
        })?;

        // Build full response with all prompt details
        let variables_json: Vec<serde_json::Value> = prompt
            .variables
            .iter()
            .map(|v| {
                serde_json::json!({
                    "name": v.name,
                    "description": v.description,
                    "required": v.required,
                    "default": v.default,
                })
            })
            .collect();

        let response = serde_json::json!({
            "name": prompt.name,
            "description": prompt.description,
            "content": prompt.content,
            "domain": domain.as_str(),
            "tags": prompt.tags,
            "usage_count": prompt.usage_count,
            "variables": variables_json,
            "created_at": prompt.created_at,
            "updated_at": prompt.updated_at,
        });

        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json".to_string()),
            text: Some(serde_json::to_string_pretty(&response).unwrap_or_default()),
            blob: None,
        })
    }

    /// Gets the help index listing all categories.
    fn get_help_index(&self) -> String {
        let mut index = "# Subcog Help\n\nWelcome to Subcog, the persistent memory system for AI coding assistants.\n\n## Available Topics\n\n".to_string();

        for cat in self.help_content.values() {
            index.push_str(&format!(
                "- **[{}](subcog://help/{})**: {}\n",
                cat.title, cat.name, cat.description
            ));
        }

        index.push_str("\n## Quick Start (MCP Tools)\n\n");
        index
            .push_str("1. **Capture**: Use `subcog_capture` tool with `namespace` and `content`\n");
        index.push_str("2. **Search**: Use `subcog_recall` tool with `query` parameter\n");
        index.push_str("3. **Status**: Use `subcog_status` tool\n");
        index.push_str(
            "4. **Browse**: Use `subcog_browse` prompt or `subcog://project/_` resource\n",
        );

        index
    }

    /// Gets a list of help categories.
    #[must_use]
    pub fn list_categories(&self) -> Vec<&HelpCategory> {
        self.help_content.values().collect()
    }
}

impl Default for ResourceHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Definition of an MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    /// Resource URI.
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// MIME type of the resource.
    pub mime_type: Option<String>,
}

/// Content of an MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// Resource URI.
    pub uri: String,
    /// MIME type.
    pub mime_type: Option<String>,
    /// Text content (for text resources).
    pub text: Option<String>,
    /// Binary content as base64 (for binary resources).
    pub blob: Option<String>,
}

/// Help category definition.
#[derive(Debug, Clone)]
pub struct HelpCategory {
    /// Category identifier.
    pub name: String,
    /// Human-readable title.
    pub title: String,
    /// Short description.
    pub description: String,
    /// Full content in Markdown.
    pub content: String,
}

/// Formats a memory as a JSON preview for topic listings.
fn format_memory_preview(m: &crate::models::Memory) -> serde_json::Value {
    serde_json::json!({
        "id": m.id.as_str(),
        "namespace": m.namespace.as_str(),
        "content_preview": truncate_content(&m.content, 200),
        "tags": m.tags,
        "uri": format!("subcog://memory/{}", m.id.as_str()),
    })
}

/// Formats a memory ID as a minimal JSON object.
fn format_memory_id_only(id: &crate::models::MemoryId) -> serde_json::Value {
    serde_json::json!({
        "id": id.as_str(),
        "uri": format!("subcog://memory/{}", id.as_str()),
    })
}

/// Simple URL decoding for URI components.
///
/// Handles common escape sequences: %20 (space), %2F (/), etc.
fn decode_uri_component(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        match c {
            '%' => {
                let hex: String = chars.by_ref().take(2).collect();
                let decoded = (hex.len() == 2)
                    .then(|| u8::from_str_radix(&hex, 16).ok())
                    .flatten()
                    .map(char::from);

                if let Some(ch) = decoded {
                    result.push(ch);
                } else {
                    result.push('%');
                    result.push_str(&hex);
                }
            },
            '+' => result.push(' '),
            _ => result.push(c),
        }
    }

    result
}

/// Truncates content to a maximum length, breaking at word boundaries.
fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        return content.to_string();
    }

    let truncated = &content[..max_len];
    truncated.rfind(' ').map_or_else(
        || format!("{truncated}..."),
        |last_space| format!("{}...", &truncated[..last_space]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Domain, Memory, MemoryId, MemoryStatus};
    use crate::services::RecallService;
    use crate::storage::index::SqliteBackend;
    use crate::storage::traits::IndexBackend;

    fn build_handler_with_memories() -> ResourceHandler {
        let index = SqliteBackend::in_memory().expect("in-memory index");
        let now = 1_700_000_000;
        let memory = Memory {
            id: MemoryId::new("decisions-1"),
            content: "Decision content".to_string(),
            namespace: Namespace::Decisions,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: now,
            updated_at: now,
            tombstoned_at: None,
            embedding: None,
            tags: vec!["alpha".to_string()],
            source: None,
        };
        let other = Memory {
            id: MemoryId::new("patterns-1"),
            content: "Pattern content".to_string(),
            namespace: Namespace::Patterns,
            domain: Domain::new(),
            project_id: None,
            branch: None,
            file_path: None,
            status: MemoryStatus::Active,
            created_at: now,
            updated_at: now,
            tombstoned_at: None,
            embedding: None,
            tags: vec!["beta".to_string()],
            source: None,
        };

        index.index(&memory).expect("index memory");
        index.index(&other).expect("index memory");

        let recall = RecallService::with_index(index);
        ResourceHandler::new().with_recall_service(recall)
    }

    #[test]
    fn test_resource_handler_creation() {
        let handler = ResourceHandler::new();
        let resources = handler.list_resources();

        assert!(!resources.is_empty());
        assert!(resources.iter().any(|r| r.uri.contains("setup")));
        assert!(resources.iter().any(|r| r.uri.contains("concepts")));
    }

    #[test]
    fn test_get_help_index() {
        let mut handler = ResourceHandler::new();
        let result = handler.get_resource("subcog://help").unwrap();

        assert!(result.text.is_some());
        let text = result.text.unwrap();
        assert!(text.contains("Subcog Help"));
        assert!(text.contains("Quick Start"));
    }

    #[test]
    fn test_get_help_category() {
        let mut handler = ResourceHandler::new();

        let result = handler.get_resource("subcog://help/setup").unwrap();
        assert!(result.text.is_some());
        assert!(result.text.unwrap().contains("MCP Server Configuration"));

        let result = handler.get_resource("subcog://help/concepts").unwrap();
        assert!(result.text.is_some());
        assert!(result.text.unwrap().contains("Namespaces"));
    }

    #[test]
    fn test_invalid_uri() {
        let mut handler = ResourceHandler::new();

        let result = handler.get_resource("http://example.com");
        assert!(result.is_err());

        let result = handler.get_resource("subcog://unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_category() {
        let mut handler = ResourceHandler::new();

        let result = handler.get_resource("subcog://help/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_categories() {
        let handler = ResourceHandler::new();
        let categories = handler.list_categories();

        assert_eq!(categories.len(), 8); // Including prompts
    }

    #[test]
    fn test_prompts_help_category() {
        let mut handler = ResourceHandler::new();

        // Should be able to get the prompts help resource
        let result = handler.get_resource("subcog://help/prompts");
        assert!(result.is_ok());

        let content = result.unwrap();
        assert!(content.text.is_some());
        let text = content.text.unwrap();
        assert!(text.contains("User-Defined Prompts"));
        assert!(text.contains("prompt_save"));
        assert!(text.contains("Variable Syntax"));
    }

    #[test]
    fn test_decode_uri_component() {
        assert_eq!(decode_uri_component("hello%20world"), "hello world");
        assert_eq!(decode_uri_component("hello+world"), "hello world");
        assert_eq!(decode_uri_component("rust%2Ferror"), "rust/error");
        assert_eq!(decode_uri_component("no%change"), "no%change"); // Invalid hex
        assert_eq!(decode_uri_component("plain"), "plain");
    }

    #[test]
    fn test_truncate_content() {
        assert_eq!(truncate_content("short", 100), "short");
        assert_eq!(
            truncate_content("this is a longer sentence with words", 20),
            "this is a longer..."
        );
        assert_eq!(truncate_content("nospaces", 4), "nosp...");
    }

    #[test]
    fn test_list_resources_includes_search_and_topics() {
        let handler = ResourceHandler::new();
        let resources = handler.list_resources();

        assert!(resources.iter().any(|r| r.uri.contains("search")));
        assert!(resources.iter().any(|r| r.uri.contains("topics")));
    }

    #[test]
    fn test_list_resources_includes_help_and_domain_templates() {
        let handler = ResourceHandler::new();
        let resources = handler.list_resources();

        assert!(resources.iter().any(|r| r.uri == "subcog://help"));
        assert!(resources.iter().any(|r| r.uri == "subcog://memory/{id}"));
        assert!(
            resources
                .iter()
                .any(|r| r.uri == "subcog://project/{namespace}")
        );
        assert!(resources.iter().any(|r| r.uri == "subcog://org/_prompts"));
    }

    #[test]
    fn test_search_resource_requires_recall_service() {
        let mut handler = ResourceHandler::new();
        let result = handler.get_resource("subcog://search/test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("RecallService"));
    }

    #[test]
    fn test_topics_resource_requires_topic_index() {
        let mut handler = ResourceHandler::new();
        let result = handler.get_resource("subcog://topics");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Topic index not initialized")
        );
    }

    #[test]
    fn test_search_resource_requires_query() {
        let mut handler = ResourceHandler::new();
        let result = handler.get_resource("subcog://search/");
        // Empty query after search/ is still valid parts
        // Just need recall service
        assert!(result.is_err());
    }

    #[test]
    fn test_project_namespace_listing_filters() {
        let mut handler = build_handler_with_memories();
        let result = handler.get_resource("subcog://project/decisions").unwrap();
        let body = result.text.unwrap();
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["count"].as_u64(), Some(1));
        assert_eq!(value["memories"][0]["id"], "decisions-1");
        assert_eq!(value["memories"][0]["ns"], "decisions");
    }

    #[test]
    fn test_project_namespace_memory_fetch() {
        let mut handler = build_handler_with_memories();
        let result = handler
            .get_resource("subcog://project/decisions/decisions-1")
            .unwrap();
        let body = result.text.unwrap();
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["id"], "decisions-1");
        assert_eq!(value["namespace"], "decisions");
    }

    #[test]
    fn test_project_namespace_memory_fetch_rejects_mismatch() {
        let mut handler = build_handler_with_memories();
        let result = handler.get_resource("subcog://project/decisions/patterns-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_namespaces_resource() {
        let mut handler = ResourceHandler::new();
        let result = handler.get_resource("subcog://namespaces").unwrap();

        assert!(result.text.is_some());
        let text = result.text.unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert_eq!(value["count"].as_u64(), Some(11));
        assert!(value["namespaces"].is_array());

        let namespaces = value["namespaces"].as_array().unwrap();
        assert!(namespaces.iter().any(|ns| ns["namespace"] == "decisions"));
        assert!(namespaces.iter().any(|ns| ns["namespace"] == "patterns"));
        assert!(namespaces.iter().any(|ns| ns["namespace"] == "learnings"));

        // Verify signal words are included
        let decisions = namespaces
            .iter()
            .find(|ns| ns["namespace"] == "decisions")
            .unwrap();
        assert!(decisions["signal_words"].is_array());
        assert!(!decisions["signal_words"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_list_resources_includes_namespaces() {
        let handler = ResourceHandler::new();
        let resources = handler.list_resources();

        assert!(resources.iter().any(|r| r.uri == "subcog://namespaces"));
    }

    #[test]
    fn test_list_resources_includes_aggregate_prompts() {
        let handler = ResourceHandler::new();
        let resources = handler.list_resources();

        assert!(resources.iter().any(|r| r.uri == "subcog://_prompts"));
    }

    #[test]
    fn test_aggregate_prompts_requires_prompt_service() {
        let mut handler = ResourceHandler::new();
        let result = handler.get_resource("subcog://_prompts");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PromptService"));
    }
}

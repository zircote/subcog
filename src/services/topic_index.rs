//! Topic index service for memory organization.
//!
//! Maintains an index of topics extracted from memories for quick lookup
//! and topic-based resource access.

use crate::models::{MemoryId, Namespace};
use crate::services::RecallService;
use crate::{Error, Result, SearchFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Information about a topic in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
    /// Topic name (normalized, lowercase).
    pub name: String,
    /// Number of memories with this topic.
    pub memory_count: usize,
    /// Namespaces where this topic appears.
    pub namespaces: Vec<Namespace>,
}

/// Service for maintaining topic → memory mappings.
pub struct TopicIndexService {
    /// Topic to memory ID mappings.
    topics: Arc<RwLock<HashMap<String, Vec<MemoryId>>>>,
    /// Topic to namespace mappings.
    topic_namespaces: Arc<RwLock<HashMap<String, Vec<Namespace>>>>,
    /// Last refresh timestamp.
    last_refresh: Arc<RwLock<Option<Instant>>>,
    /// Refresh interval.
    refresh_interval: Duration,
}

impl TopicIndexService {
    /// Creates a new topic index service.
    #[must_use]
    pub fn new() -> Self {
        Self {
            topics: Arc::new(RwLock::new(HashMap::new())),
            topic_namespaces: Arc::new(RwLock::new(HashMap::new())),
            last_refresh: Arc::new(RwLock::new(None)),
            refresh_interval: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Sets the refresh interval.
    #[must_use]
    pub const fn with_refresh_interval(mut self, interval: Duration) -> Self {
        self.refresh_interval = interval;
        self
    }

    /// Checks if the index needs refreshing.
    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        let last = self.last_refresh.read().ok().and_then(|guard| *guard);
        match last {
            Some(t) => t.elapsed() > self.refresh_interval,
            None => true,
        }
    }

    /// Builds the topic index from a recall service.
    ///
    /// Extracts topics from:
    /// - Memory tags
    /// - Memory namespace names
    /// - Keywords in memory content
    ///
    /// # Errors
    ///
    /// Returns an error if memory retrieval fails.
    pub fn build_index(&self, recall: &RecallService) -> Result<()> {
        let filter = SearchFilter::new();
        let result = recall.list_all(&filter, 10000)?;

        let mut topics_map: HashMap<String, Vec<MemoryId>> = HashMap::new();
        let mut namespace_map: HashMap<String, Vec<Namespace>> = HashMap::new();

        for hit in &result.memories {
            let memory = &hit.memory;

            // Extract topics from tags
            for tag in &memory.tags {
                let topic = normalize_topic(tag);
                add_topic_entry(
                    &topic,
                    &memory.id,
                    memory.namespace,
                    &mut topics_map,
                    &mut namespace_map,
                );
            }

            // Extract topics from namespace name
            let ns_topic = normalize_topic(memory.namespace.as_str());
            add_topic_entry(
                &ns_topic,
                &memory.id,
                memory.namespace,
                &mut topics_map,
                &mut namespace_map,
            );

            // Extract keyword topics from content (top 5 keywords)
            let keywords = extract_content_keywords(&memory.content);
            for keyword in keywords.into_iter().take(5) {
                let topic = normalize_topic(&keyword);
                add_topic_with_min_length(
                    &topic,
                    3,
                    &memory.id,
                    memory.namespace,
                    &mut topics_map,
                    &mut namespace_map,
                );
            }
        }

        // Deduplicate memory IDs per topic
        for ids in topics_map.values_mut() {
            ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            ids.dedup_by(|a, b| a.as_str() == b.as_str());
        }

        // Update the index
        {
            let mut guard = self.topics.write().map_err(|_| Error::OperationFailed {
                operation: "build_index".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;
            *guard = topics_map;
        }

        {
            let mut guard = self
                .topic_namespaces
                .write()
                .map_err(|_| Error::OperationFailed {
                    operation: "build_index".to_string(),
                    cause: "Lock poisoned".to_string(),
                })?;
            *guard = namespace_map;
        }

        // Update last refresh time
        {
            let mut guard = self
                .last_refresh
                .write()
                .map_err(|_| Error::OperationFailed {
                    operation: "build_index".to_string(),
                    cause: "Lock poisoned".to_string(),
                })?;
            *guard = Some(Instant::now());
        }

        Ok(())
    }

    /// Lists all topics with their metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is poisoned.
    pub fn list_topics(&self) -> Result<Vec<TopicInfo>> {
        let topics_guard = self.topics.read().map_err(|_| Error::OperationFailed {
            operation: "list_topics".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let ns_guard = self
            .topic_namespaces
            .read()
            .map_err(|_| Error::OperationFailed {
                operation: "list_topics".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        let mut topics: Vec<TopicInfo> = topics_guard
            .iter()
            .map(|(name, ids)| TopicInfo {
                name: name.clone(),
                memory_count: ids.len(),
                namespaces: ns_guard.get(name).cloned().unwrap_or_default(),
            })
            .collect();

        // Sort by memory count descending
        topics.sort_by(|a, b| b.memory_count.cmp(&a.memory_count));

        Ok(topics)
    }

    /// Gets memory IDs for a specific topic.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is poisoned.
    pub fn get_topic_memories(&self, topic: &str) -> Result<Vec<MemoryId>> {
        let normalized = normalize_topic(topic);
        let guard = self.topics.read().map_err(|_| Error::OperationFailed {
            operation: "get_topic_memories".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        Ok(guard.get(&normalized).cloned().unwrap_or_default())
    }

    /// Gets topic info for a specific topic.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is poisoned.
    pub fn get_topic_info(&self, topic: &str) -> Result<Option<TopicInfo>> {
        let normalized = normalize_topic(topic);

        let topics_guard = self.topics.read().map_err(|_| Error::OperationFailed {
            operation: "get_topic_info".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let ns_guard = self
            .topic_namespaces
            .read()
            .map_err(|_| Error::OperationFailed {
                operation: "get_topic_info".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        match topics_guard.get(&normalized) {
            Some(ids) => Ok(Some(TopicInfo {
                name: normalized,
                memory_count: ids.len(),
                namespaces: ns_guard.get(topic).cloned().unwrap_or_default(),
            })),
            None => Ok(None),
        }
    }

    /// Adds a memory to the topic index.
    ///
    /// Call this when a new memory is captured to keep the index up to date.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is poisoned.
    pub fn add_memory(
        &self,
        memory_id: &MemoryId,
        tags: &[String],
        namespace: Namespace,
    ) -> Result<()> {
        let mut topics_guard = self.topics.write().map_err(|_| Error::OperationFailed {
            operation: "add_memory".to_string(),
            cause: "Lock poisoned".to_string(),
        })?;

        let mut ns_guard = self
            .topic_namespaces
            .write()
            .map_err(|_| Error::OperationFailed {
                operation: "add_memory".to_string(),
                cause: "Lock poisoned".to_string(),
            })?;

        // Add from tags
        for tag in tags {
            let topic = normalize_topic(tag);
            add_topic_entry_guarded(
                &topic,
                memory_id,
                namespace,
                &mut topics_guard,
                &mut ns_guard,
            );
        }

        // Add from namespace
        let ns_topic = normalize_topic(namespace.as_str());
        add_topic_entry_guarded(
            &ns_topic,
            memory_id,
            namespace,
            &mut topics_guard,
            &mut ns_guard,
        );

        Ok(())
    }

    /// Returns the number of topics in the index.
    #[must_use]
    pub fn topic_count(&self) -> usize {
        self.topics.read().map(|guard| guard.len()).unwrap_or(0)
    }

    /// Returns the total number of topic-memory associations.
    #[must_use]
    pub fn association_count(&self) -> usize {
        self.topics
            .read()
            .map(|guard| guard.values().map(Vec::len).sum())
            .unwrap_or(0)
    }
}

impl Default for TopicIndexService {
    fn default() -> Self {
        Self::new()
    }
}

/// Adds a topic entry to the maps (helper to reduce nesting).
fn add_topic_entry(
    topic: &str,
    memory_id: &MemoryId,
    namespace: Namespace,
    topics_map: &mut HashMap<String, Vec<MemoryId>>,
    namespace_map: &mut HashMap<String, Vec<Namespace>>,
) {
    if topic.is_empty() {
        return;
    }
    topics_map
        .entry(topic.to_string())
        .or_default()
        .push(memory_id.clone());
    insert_namespace_if_missing(namespace_map, topic, namespace);
}

/// Adds a topic entry only if it meets minimum length requirement.
fn add_topic_with_min_length(
    topic: &str,
    min_len: usize,
    memory_id: &MemoryId,
    namespace: Namespace,
    topics_map: &mut HashMap<String, Vec<MemoryId>>,
    namespace_map: &mut HashMap<String, Vec<Namespace>>,
) {
    if topic.len() >= min_len {
        add_topic_entry(topic, memory_id, namespace, topics_map, namespace_map);
    }
}

/// Adds a topic entry to guarded maps (for use with lock guards).
fn add_topic_entry_guarded(
    topic: &str,
    memory_id: &MemoryId,
    namespace: Namespace,
    topics_guard: &mut HashMap<String, Vec<MemoryId>>,
    ns_guard: &mut HashMap<String, Vec<Namespace>>,
) {
    if topic.is_empty() {
        return;
    }
    topics_guard
        .entry(topic.to_string())
        .or_default()
        .push(memory_id.clone());
    insert_namespace_if_missing(ns_guard, topic, namespace);
}

/// Inserts a namespace into the list if not already present.
fn insert_namespace_if_missing(
    map: &mut HashMap<String, Vec<Namespace>>,
    topic: &str,
    namespace: Namespace,
) {
    let ns_list = map.entry(topic.to_string()).or_default();
    if !ns_list.contains(&namespace) {
        ns_list.push(namespace);
    }
}

/// Normalizes a topic name for consistent indexing.
fn normalize_topic(topic: &str) -> String {
    topic
        .trim()
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "")
}

/// Extracts keyword topics from content.
fn extract_content_keywords(content: &str) -> Vec<String> {
    // Stop words to filter out
    static STOP_WORDS: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall",
        "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
        "from", "as", "into", "through", "during", "before", "after", "above", "below", "between",
        "under", "again", "further", "then", "once", "here", "there", "when", "where", "why",
        "how", "all", "each", "few", "more", "most", "other", "some", "such", "no", "nor", "not",
        "only", "own", "same", "so", "than", "too", "very", "just", "also", "now", "and", "but",
        "or", "if", "because", "until", "while", "this", "that", "these", "those", "what", "which",
        "who", "whom", "whose", "it", "its", "they", "them", "their", "we", "us", "our", "you",
        "your", "i", "my", "me", "he", "him", "his", "she", "her",
    ];

    let words: Vec<String> = content
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3 && w.len() <= 30)
        .map(str::to_lowercase)
        .filter(|w| !STOP_WORDS.contains(&w.as_str()))
        .filter(|w| !w.chars().all(char::is_numeric))
        .collect();

    // Count word frequencies
    let mut freq: HashMap<String, usize> = HashMap::new();
    for word in words {
        *freq.entry(word).or_insert(0) += 1;
    }

    // Sort by frequency and return top keywords
    let mut sorted: Vec<_> = freq.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.into_iter().map(|(w, _)| w).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_index_creation() {
        let service = TopicIndexService::new();
        assert!(service.needs_refresh());
        assert_eq!(service.topic_count(), 0);
    }

    #[test]
    fn test_normalize_topic() {
        assert_eq!(normalize_topic("Rust"), "rust");
        assert_eq!(normalize_topic("  Python  "), "python");
        assert_eq!(normalize_topic("error-handling"), "error-handling");
        assert_eq!(normalize_topic("test_case"), "testcase");
    }

    #[test]
    fn test_extract_content_keywords() {
        let content = "The Rust programming language is great for systems programming";
        let keywords = extract_content_keywords(content);

        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"programming".to_string()));
        assert!(!keywords.contains(&"the".to_string())); // stop word
    }

    #[test]
    fn test_add_memory() {
        let service = TopicIndexService::new();
        let id = MemoryId::new("test-123");
        let tags = vec!["rust".to_string(), "error-handling".to_string()];

        service.add_memory(&id, &tags, Namespace::Patterns).unwrap();

        let rust_memories = service.get_topic_memories("rust").unwrap();
        assert_eq!(rust_memories.len(), 1);
        assert_eq!(rust_memories[0].as_str(), "test-123");

        let patterns_memories = service.get_topic_memories("patterns").unwrap();
        assert_eq!(patterns_memories.len(), 1);
    }

    #[test]
    fn test_list_topics() {
        let service = TopicIndexService::new();
        let id1 = MemoryId::new("test-1");
        let id2 = MemoryId::new("test-2");

        service
            .add_memory(&id1, &["rust".to_string()], Namespace::Decisions)
            .unwrap();
        service
            .add_memory(
                &id2,
                &["rust".to_string(), "async".to_string()],
                Namespace::Patterns,
            )
            .unwrap();

        let topics = service.list_topics().unwrap();

        // rust should have 2 memories, async should have 1
        let rust_topic = topics.iter().find(|t| t.name == "rust").unwrap();
        assert_eq!(rust_topic.memory_count, 2);

        let async_topic = topics.iter().find(|t| t.name == "async").unwrap();
        assert_eq!(async_topic.memory_count, 1);
    }

    #[test]
    fn test_get_topic_info() {
        let service = TopicIndexService::new();
        let id = MemoryId::new("test-1");

        service
            .add_memory(&id, &["authentication".to_string()], Namespace::Decisions)
            .unwrap();

        let info = service.get_topic_info("authentication").unwrap();
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.name, "authentication");
        assert_eq!(info.memory_count, 1);
        assert!(info.namespaces.contains(&Namespace::Decisions));
    }

    #[test]
    fn test_get_topic_info_not_found() {
        let service = TopicIndexService::new();
        let info = service.get_topic_info("nonexistent").unwrap();
        assert!(info.is_none());
    }

    #[test]
    fn test_topic_info_serialization() {
        let info = TopicInfo {
            name: "rust".to_string(),
            memory_count: 5,
            namespaces: vec![Namespace::Decisions, Namespace::Patterns],
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("rust"));
        assert!(json.contains('5'));
    }

    #[test]
    fn test_refresh_interval() {
        let service = TopicIndexService::new().with_refresh_interval(Duration::from_secs(60));
        assert!(service.needs_refresh());

        // After setting last_refresh, should not need refresh
        {
            let mut guard = service.last_refresh.write().unwrap();
            *guard = Some(Instant::now());
        }
        assert!(!service.needs_refresh());
    }
}

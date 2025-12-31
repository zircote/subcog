//! Redis-based prompt storage using `RediSearch`.
//!
//! Stores prompts in Redis with full-text search support via `RediSearch`.

#[cfg(feature = "redis")]
mod implementation {
    use crate::models::PromptTemplate;
    use crate::storage::prompt::PromptStorage;
    use crate::{Error, Result};
    use redis::{Client, Commands, Connection};

    /// Redis-based prompt storage using `RediSearch`.
    pub struct RedisPromptStorage {
        /// Redis client.
        client: Client,
        /// Index name for prompts.
        index_name: String,
    }

    impl RedisPromptStorage {
        /// Creates a new Redis prompt storage.
        ///
        /// # Errors
        ///
        /// Returns an error if the Redis connection fails.
        pub fn new(connection_url: &str, index_name: impl Into<String>) -> Result<Self> {
            let client = Client::open(connection_url).map_err(|e| Error::OperationFailed {
                operation: "redis_connect".to_string(),
                cause: e.to_string(),
            })?;

            let storage = Self {
                client,
                index_name: index_name.into(),
            };

            // Ensure index exists
            storage.ensure_index()?;

            Ok(storage)
        }

        /// Creates a storage with default settings.
        ///
        /// # Errors
        ///
        /// Returns an error if the Redis connection fails.
        pub fn with_defaults() -> Result<Self> {
            Self::new("redis://localhost:6379", "subcog_prompts")
        }

        /// Gets a connection from the client.
        fn get_connection(&self) -> Result<Connection> {
            self.client
                .get_connection()
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_get_connection".to_string(),
                    cause: e.to_string(),
                })
        }

        /// Ensures the `RediSearch` index exists.
        fn ensure_index(&self) -> Result<()> {
            let mut conn = self.get_connection()?;

            // Check if index exists
            if self.index_exists(&mut conn) {
                return Ok(());
            }

            // Create the index with schema
            self.create_index(&mut conn)
        }

        /// Checks if the index already exists.
        fn index_exists(&self, conn: &mut Connection) -> bool {
            let result: redis::RedisResult<Vec<String>> = redis::cmd("FT._LIST").query(conn);

            result
                .map(|indices| indices.iter().any(|i| i == &self.index_name))
                .unwrap_or(false)
        }

        /// Creates the `RediSearch` index for prompts.
        fn create_index(&self, conn: &mut Connection) -> Result<()> {
            let result: redis::RedisResult<String> = redis::cmd("FT.CREATE")
                .arg(&self.index_name)
                .arg("ON")
                .arg("HASH")
                .arg("PREFIX")
                .arg(1)
                .arg("prompt:")
                .arg("SCHEMA")
                .arg("name")
                .arg("TEXT")
                .arg("WEIGHT")
                .arg("2.0")
                .arg("description")
                .arg("TEXT")
                .arg("WEIGHT")
                .arg("1.0")
                .arg("content")
                .arg("TEXT")
                .arg("WEIGHT")
                .arg("0.5")
                .arg("tags")
                .arg("TAG")
                .arg("author")
                .arg("TAG")
                .arg("usage_count")
                .arg("NUMERIC")
                .arg("SORTABLE")
                .arg("created_at")
                .arg("NUMERIC")
                .arg("SORTABLE")
                .arg("updated_at")
                .arg("NUMERIC")
                .arg("SORTABLE")
                .query(conn);

            match result {
                Ok(_) => Ok(()),
                Err(e) if e.to_string().contains("Index already exists") => Ok(()),
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_create_prompt_index".to_string(),
                    cause: e.to_string(),
                }),
            }
        }

        /// Builds the Redis key for a prompt.
        fn prompt_key(name: &str) -> String {
            format!("prompt:{name}")
        }

        /// Serializes a prompt to Redis hash fields.
        fn serialize_prompt(template: &PromptTemplate) -> Vec<(String, String)> {
            let variables_json = serde_json::to_string(&template.variables).unwrap_or_default();
            let tags_str = template.tags.join(",");

            vec![
                ("name".to_string(), template.name.clone()),
                ("description".to_string(), template.description.clone()),
                ("content".to_string(), template.content.clone()),
                ("variables".to_string(), variables_json),
                ("tags".to_string(), tags_str),
                (
                    "author".to_string(),
                    template.author.clone().unwrap_or_default(),
                ),
                ("usage_count".to_string(), template.usage_count.to_string()),
                ("created_at".to_string(), template.created_at.to_string()),
                ("updated_at".to_string(), template.updated_at.to_string()),
            ]
        }

        /// Deserializes a prompt from Redis hash fields.
        fn deserialize_prompt(
            fields: &std::collections::HashMap<String, String>,
        ) -> Option<PromptTemplate> {
            let name = fields.get("name")?.clone();
            let description = fields.get("description").cloned().unwrap_or_default();
            let content = fields.get("content").cloned().unwrap_or_default();
            let variables_json = fields.get("variables").cloned().unwrap_or_default();
            let tags_str = fields.get("tags").cloned().unwrap_or_default();
            let author = fields.get("author").cloned().filter(|s| !s.is_empty());
            let usage_count: u64 = fields
                .get("usage_count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let created_at: u64 = fields
                .get("created_at")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let updated_at: u64 = fields
                .get("updated_at")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            let variables = serde_json::from_str(&variables_json).unwrap_or_default();
            let tags: Vec<String> = tags_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            Some(PromptTemplate {
                name,
                description,
                content,
                variables,
                tags,
                author,
                usage_count,
                created_at,
                updated_at,
            })
        }
    }

    impl PromptStorage for RedisPromptStorage {
        fn save(&self, template: &PromptTemplate) -> Result<String> {
            let mut conn = self.get_connection()?;
            let key = Self::prompt_key(&template.name);

            // Serialize and store as hash
            let fields = Self::serialize_prompt(template);
            let field_refs: Vec<(&str, &str)> = fields
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();

            let _: () =
                conn.hset_multiple(&key, &field_refs)
                    .map_err(|e| Error::OperationFailed {
                        operation: "redis_save_prompt".to_string(),
                        cause: e.to_string(),
                    })?;

            Ok(format!("prompt_redis_{}", template.name))
        }

        fn get(&self, name: &str) -> Result<Option<PromptTemplate>> {
            let mut conn = self.get_connection()?;
            let key = Self::prompt_key(name);

            // Get all fields from hash
            let result: redis::RedisResult<std::collections::HashMap<String, String>> =
                conn.hgetall(&key);

            match result {
                Ok(fields) if fields.is_empty() => Ok(None),
                Ok(fields) => Ok(Self::deserialize_prompt(&fields)),
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_get_prompt".to_string(),
                    cause: e.to_string(),
                }),
            }
        }

        fn list(
            &self,
            tags: Option<&[String]>,
            name_pattern: Option<&str>,
        ) -> Result<Vec<PromptTemplate>> {
            let mut conn = self.get_connection()?;

            // Build query
            let mut query_parts = vec!["*".to_string()];

            // Add tag filter
            if let Some(tag_list) = tags.filter(|t| !t.is_empty()) {
                query_parts.extend(tag_list.iter().map(|tag| format!("@tags:{{{tag}}}")));
            }

            // Add name pattern filter
            if let Some(pattern) = name_pattern {
                query_parts.push(format!("@name:{pattern}"));
            }

            let query = if query_parts.len() == 1 {
                "*".to_string()
            } else {
                query_parts[1..].join(" ")
            };

            // FT.SEARCH idx "query" LIMIT 0 1000 SORTBY usage_count DESC
            let result: redis::RedisResult<Vec<redis::Value>> = redis::cmd("FT.SEARCH")
                .arg(&self.index_name)
                .arg(&query)
                .arg("LIMIT")
                .arg(0)
                .arg(1000)
                .arg("SORTBY")
                .arg("usage_count")
                .arg("DESC")
                .query(&mut conn);

            match result {
                Ok(values) => Ok(self.parse_search_results(&values)),
                Err(e) => Err(Error::OperationFailed {
                    operation: "redis_list_prompts".to_string(),
                    cause: e.to_string(),
                }),
            }
        }

        fn delete(&self, name: &str) -> Result<bool> {
            let mut conn = self.get_connection()?;
            let key = Self::prompt_key(name);

            let deleted: i32 = conn.del(&key).map_err(|e| Error::OperationFailed {
                operation: "redis_delete_prompt".to_string(),
                cause: e.to_string(),
            })?;

            Ok(deleted > 0)
        }

        #[allow(clippy::cast_sign_loss)]
        fn increment_usage(&self, name: &str) -> Result<u64> {
            let mut conn = self.get_connection()?;
            let key = Self::prompt_key(name);

            // Increment usage_count field
            let count: i64 =
                conn.hincr(&key, "usage_count", 1)
                    .map_err(|e| Error::OperationFailed {
                        operation: "redis_increment_usage".to_string(),
                        cause: e.to_string(),
                    })?;

            // Update updated_at timestamp
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let _: () = conn
                .hset(&key, "updated_at", now)
                .map_err(|e| Error::OperationFailed {
                    operation: "redis_update_timestamp".to_string(),
                    cause: e.to_string(),
                })?;

            Ok(count as u64)
        }
    }

    impl RedisPromptStorage {
        /// Parses FT.SEARCH results into prompt templates.
        #[allow(clippy::excessive_nesting)]
        fn parse_search_results(&self, values: &[redis::Value]) -> Vec<PromptTemplate> {
            if values.is_empty() {
                return Vec::new();
            }

            let mut results = Vec::new();
            let mut i = 1; // Skip count
            while i < values.len() {
                // Skip key
                i += 1;

                // Parse fields array and collect template if valid
                if let Some(t) = self.try_parse_value_at(values, i) {
                    results.push(t);
                }
                i += 1;
            }
            results
        }

        /// Tries to parse a value at the given index as a template.
        fn try_parse_value_at(
            &self,
            values: &[redis::Value],
            idx: usize,
        ) -> Option<PromptTemplate> {
            let value = values.get(idx)?;
            match value {
                redis::Value::Array(fields) => self.parse_field_array(fields),
                _ => None,
            }
        }

        /// Parses a field array from FT.SEARCH results.
        #[allow(clippy::excessive_nesting)]
        fn parse_field_array(&self, fields: &[redis::Value]) -> Option<PromptTemplate> {
            let mut map = std::collections::HashMap::new();

            for pair in fields.chunks(2) {
                if let [
                    redis::Value::BulkString(key),
                    redis::Value::BulkString(value),
                ] = pair
                {
                    let key_str = String::from_utf8_lossy(key).to_string();
                    let value_str = String::from_utf8_lossy(value).to_string();
                    map.insert(key_str, value_str);
                }
            }

            Self::deserialize_prompt(&map)
        }
    }
}

#[cfg(feature = "redis")]
pub use implementation::RedisPromptStorage;

#[cfg(not(feature = "redis"))]
mod stub {
    use crate::models::PromptTemplate;
    use crate::storage::prompt::PromptStorage;
    use crate::{Error, Result};

    /// Stub Redis prompt storage when feature is not enabled.
    pub struct RedisPromptStorage;

    impl RedisPromptStorage {
        /// Creates a new Redis prompt storage (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn new(_connection_url: &str, _index_name: impl Into<String>) -> Result<Self> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        /// Creates a storage with default settings (stub).
        ///
        /// # Errors
        ///
        /// Always returns an error because the feature is not enabled.
        pub fn with_defaults() -> Result<Self> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }
    }

    impl PromptStorage for RedisPromptStorage {
        fn save(&self, _template: &PromptTemplate) -> Result<String> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn get(&self, _name: &str) -> Result<Option<PromptTemplate>> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn list(
            &self,
            _tags: Option<&[String]>,
            _name_pattern: Option<&str>,
        ) -> Result<Vec<PromptTemplate>> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn delete(&self, _name: &str) -> Result<bool> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }

        fn increment_usage(&self, _name: &str) -> Result<u64> {
            Err(Error::FeatureNotEnabled("redis".to_string()))
        }
    }
}

#[cfg(not(feature = "redis"))]
pub use stub::RedisPromptStorage;

//! Index backend implementations.

mod domain;
mod org_router;
mod postgresql;
mod sqlite;

pub use domain::{
    DomainIndexConfig, DomainIndexManager, DomainScope, OrgIndexConfig, find_repo_root,
    get_user_data_dir, is_in_git_repo, is_path_in_git_repo,
};
pub use org_router::{OrgBackendType, OrgIndexRouter, OrgIndexStatus};
pub use postgresql::PostgresIndexBackend;
pub use sqlite::SqliteBackend;

// Redis backend available with feature flag
#[cfg(feature = "redis")]
mod redis;
#[cfg(feature = "redis")]
pub use redis::RedisBackend;

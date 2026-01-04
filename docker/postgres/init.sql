-- Subcog PostgreSQL Initialization Script
-- Enables pgvector extension and creates schema for memory storage

-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Create memories table with vector support
CREATE TABLE IF NOT EXISTS memories (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    namespace TEXT NOT NULL,
    domain TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    embedding vector(384),  -- all-MiniLM-L6-v2 produces 384-dim embeddings
    tags TEXT[] DEFAULT '{}',
    source TEXT,
    project_id TEXT,
    branch TEXT,
    file_path TEXT,
    tombstoned_at BIGINT
);

-- Create indexes for common queries
CREATE INDEX IF NOT EXISTS idx_memories_namespace ON memories(namespace);
CREATE INDEX IF NOT EXISTS idx_memories_domain ON memories(domain);
CREATE INDEX IF NOT EXISTS idx_memories_status ON memories(status);
CREATE INDEX IF NOT EXISTS idx_memories_project_id ON memories(project_id);
CREATE INDEX IF NOT EXISTS idx_memories_branch ON memories(branch);
CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memories_tags ON memories USING GIN(tags);

-- Create HNSW index for vector similarity search
-- Using cosine distance (vector_cosine_ops) for semantic similarity
CREATE INDEX IF NOT EXISTS idx_memories_embedding ON memories
    USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- Create prompts table for prompt template storage
CREATE TABLE IF NOT EXISTS prompts (
    name TEXT NOT NULL,
    domain TEXT NOT NULL,
    content TEXT NOT NULL,
    description TEXT,
    tags TEXT[] DEFAULT '{}',
    variables JSONB DEFAULT '[]',
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    usage_count INTEGER DEFAULT 0,
    last_used_at BIGINT,
    PRIMARY KEY (name, domain)
);

CREATE INDEX IF NOT EXISTS idx_prompts_domain ON prompts(domain);
CREATE INDEX IF NOT EXISTS idx_prompts_tags ON prompts USING GIN(tags);

-- Full-text search configuration
CREATE INDEX IF NOT EXISTS idx_memories_content_fts ON memories
    USING GIN(to_tsvector('english', content));

-- Grant permissions (if running as superuser creating for app user)
-- GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO subcog;
-- GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO subcog;

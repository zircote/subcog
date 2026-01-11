# Ollama Consolidation Integration Tests

## Overview

Added comprehensive integration tests to verify that the consolidation service works correctly with the Ollama (local) LLM provider. Tests are located in `tests/integration_test.rs` in the `consolidation_integration_tests` module.

## Test Cases

### 1. `test_consolidation_with_ollama_provider`
**Purpose:** Verify consolidation works with local Ollama server.

**What it tests:**
- Creates 2 related memories about caching decisions with embeddings
- Uses real Ollama client to generate summaries
- Verifies summary generation succeeds
- Verifies summary is non-empty and detailed
- Verifies summary contains relevant caching terms (Redis, cache, LRU, memory)

**Requires:** Ollama server running locally
**Skips gracefully:** If Ollama server not available (prints message to stderr)

### 2. `test_consolidation_end_to_end_with_ollama`
**Purpose:** End-to-end integration test with Ollama including edge storage.

**What it tests:**
- Creates 3 related memories about in-memory caching
- Sets up both persistence backend (Filesystem) and index backend (SQLite)
- Uses real Ollama client with index backend for edge storage
- Runs full `consolidate_memories()` orchestration
- Verifies consolidation stats (processed count)
- Tests complete flow: find groups → summarize → create nodes → store edges

**Requires:** Ollama server running locally
**Skips gracefully:** If Ollama server not available (prints message to stderr)

## Prerequisites

### Starting Ollama Server

**Option 1: Ollama CLI (macOS/Linux)**
```bash
# Install Ollama
curl https://ollama.ai/install.sh | sh

# Start Ollama server
ollama serve

# In another terminal, pull a model (if not already downloaded)
ollama pull llama3.2
```

**Option 2: Docker**
```bash
# Run Ollama in Docker
docker run -d -v ollama:/root/.ollama -p 11434:11434 --name ollama ollama/ollama

# Pull a model
docker exec -it ollama ollama pull llama3.2
```

**Verify Ollama is running:**
```bash
curl http://localhost:11434/api/tags
```

### Environment Variables

Ollama tests use these environment variables (all optional with defaults):
- `OLLAMA_HOST` - Ollama server endpoint (default: `http://localhost:11434`)
- `OLLAMA_MODEL` - Model to use (default: `llama3.2`)

## Running the Tests

### Run all consolidation integration tests:
```bash
cargo test --test integration_test consolidation_integration_tests
```

### Run specific Ollama test:
```bash
cargo test --test integration_test test_consolidation_with_ollama_provider
```

### Run with output:
```bash
cargo test --test integration_test consolidation_integration_tests -- --nocapture
```

### Run without Ollama (skips Ollama tests):
```bash
# Stop Ollama server to test skip behavior
# Tests will skip gracefully with stderr message
cargo test --test integration_test consolidation_integration_tests
```

### Run with Ollama server:
```bash
# In one terminal:
ollama serve

# In another terminal:
cargo test --test integration_test consolidation_integration_tests -- --nocapture
```

### Run with custom Ollama configuration:
```bash
export OLLAMA_HOST="http://localhost:11434"
export OLLAMA_MODEL="llama3.2"
cargo test --test integration_test consolidation_integration_tests -- --nocapture
```

## Expected Output

### Without Ollama server:
```
test consolidation_integration_tests::test_consolidation_with_mock_provider ... ok
test consolidation_integration_tests::test_consolidation_creates_summary_with_source_ids ... ok
Skipping Ollama consolidation test - Ollama server not running
To run this test: start Ollama server with 'ollama serve' or Docker
test consolidation_integration_tests::test_consolidation_with_ollama_provider ... ok
Skipping Ollama end-to-end consolidation test - Ollama server not running
To run this test: start Ollama server with 'ollama serve' or Docker
test consolidation_integration_tests::test_consolidation_end_to_end_with_ollama ... ok
```

### With Ollama server:
```
test consolidation_integration_tests::test_consolidation_with_mock_provider ... ok
test consolidation_integration_tests::test_consolidation_creates_summary_with_source_ids ... ok
Ollama consolidation test passed. Summary: [Generated summary from Ollama]
test consolidation_integration_tests::test_consolidation_with_ollama_provider ... ok
Consolidation stats: Processed: 3, Archived: 0, Merged: 0, Contradictions: 0, Summaries created: X
Ollama end-to-end consolidation test passed
test consolidation_integration_tests::test_consolidation_end_to_end_with_ollama ... ok
```

## Test Implementation Details

### Ollama Availability Check
- Uses `OllamaClient::is_available()` method to check server connectivity
- Makes GET request to `/api/tags` endpoint
- Returns `true` if server responds with success status
- Tests skip gracefully if `is_available()` returns `false`

### Helper Functions
- `create_test_memory()`: Creates test memories with optional embeddings
- Memories include realistic content about caching decisions
- Embeddings are close enough to trigger similarity matching (threshold 0.7)

### Graceful Skipping
- Ollama tests check server availability before running
- If not available, prints helpful message to stderr and returns early
- Tests still pass (not marked as failure)
- Allows CI/CD to run without local Ollama server

### Coverage
The tests cover:
- ✅ Ollama provider integration (conditional on server availability)
- ✅ Summary generation with local LLM
- ✅ Memory grouping by similarity
- ✅ Summary node creation with source IDs
- ✅ Edge storage (when index backend available)
- ✅ End-to-end orchestration
- ✅ Graceful degradation (skip when server unavailable)

## Verification Checklist

Before marking subtask 3.3 complete, verify:
- [x] Tests added to `tests/integration_test.rs`
- [x] Tests compile without errors
- [ ] Tests skip gracefully without Ollama server (manual verification required)
- [ ] Tests pass with Ollama server running (manual verification required)
- [x] Documentation created for running tests
- [x] Graceful degradation when server not available
- [x] Test output documented

## Notes

1. **Local Inference**: Ollama runs models locally - no API keys required
2. **Model Requirements**: Tests use default model (llama3.2) - ensure model is downloaded
3. **Performance**: Local inference may be slower than cloud APIs (30-60s per test)
4. **Deterministic Mock**: Mock provider ensures tests are deterministic for CI/CD
5. **Realistic Scenarios**: Test memories use realistic content (caching decisions)
6. **Multiple Providers**: Framework supports testing with Anthropic, OpenAI, etc.

## Troubleshooting

### Test times out
- Increase timeout: `export SUBCOG_LLM_TIMEOUT_MS=60000`
- Use a faster model: `export OLLAMA_MODEL=llama3.2`

### "Ollama server not running" message
- Verify Ollama is running: `curl http://localhost:11434/api/tags`
- Check endpoint: `echo $OLLAMA_HOST` (should be `http://localhost:11434`)
- Start Ollama: `ollama serve`

### Model not found
- Pull the model: `ollama pull llama3.2`
- Or use different model: `export OLLAMA_MODEL=llama2`

### Connection refused
- Check Ollama is listening on correct port: `lsof -i :11434`
- Restart Ollama server
- Check firewall settings

# Search Intent Detection

The search intent detection system analyzes user prompts to surface the most relevant memories.

## Overview

When a user submits a prompt, the system:
1. Detects the type of question/request
2. Assigns namespace weights based on intent
3. Searches with weighted namespaces
4. Returns contextually relevant memories

## Intent Types

### HowTo

**Purpose:** User wants to know how to do something

**Detection Patterns:**
- "how do I..."
- "how to..."
- "implement..."
- "create..."
- "build..."
- "make..."
- "add..."

**Examples:**
- "How do I implement authentication?"
- "Create a new API endpoint"
- "Add error handling to this function"

**Namespace Weights:**
| Namespace | Weight |
|-----------|--------|
| patterns | 1.5 |
| learnings | 1.3 |
| apis | 1.2 |
| decisions | 1.0 |
| context | 0.8 |

---

### Location

**Purpose:** User wants to find something

**Detection Patterns:**
- "where is..."
- "find..."
- "locate..."
- "which file..."
- "what file..."
- "show me..."

**Examples:**
- "Where is the database configuration?"
- "Find the authentication module"
- "Which file handles user creation?"

**Namespace Weights:**
| Namespace | Weight |
|-----------|--------|
| apis | 1.5 |
| config | 1.5 |
| context | 1.2 |
| decisions | 1.0 |
| patterns | 0.8 |

---

### Explanation

**Purpose:** User wants to understand something

**Detection Patterns:**
- "what is..."
- "explain..."
- "describe..."
- "what does..."
- "why is..."
- "tell me about..."

**Examples:**
- "What is the ServiceContainer?"
- "Explain the three-layer storage"
- "Why is RRF used for search fusion?"

**Namespace Weights:**
| Namespace | Weight |
|-----------|--------|
| decisions | 1.5 |
| context | 1.4 |
| patterns | 1.2 |
| learnings | 1.0 |
| apis | 0.8 |

---

### Comparison

**Purpose:** User wants to compare options

**Detection Patterns:**
- "difference between..."
- "...vs..."
- "...versus..."
- "compare..."
- "which is better..."
- "should I use...or..."

**Examples:**
- "PostgreSQL vs SQLite for this use case?"
- "Difference between hybrid and vector search"
- "Should I use REST or GraphQL?"

**Namespace Weights:**
| Namespace | Weight |
|-----------|--------|
| decisions | 1.5 |
| patterns | 1.3 |
| learnings | 1.1 |
| context | 1.0 |
| apis | 0.9 |

---

### Troubleshoot

**Purpose:** User is debugging an issue

**Detection Patterns:**
- "error..."
- "fix..."
- "not working..."
- "debug..."
- "issue..."
- "problem..."
- "fails..."
- "broken..."

**Examples:**
- "Why is this test failing?"
- "Error: connection refused"
- "The build is broken"

**Namespace Weights:**
| Namespace | Weight |
|-----------|--------|
| blockers | 1.5 |
| learnings | 1.4 |
| testing | 1.2 |
| patterns | 1.0 |
| context | 0.9 |

---

### General

**Purpose:** General search or unclassified query

**Detection Patterns:**
- "search..."
- "show me..."
- "list..."
- "find memories..."
- Default for unmatched queries

**Examples:**
- "Search for recent patterns"
- "Show me decisions about the API"
- "List all security-related memories"

**Namespace Weights:**
All namespaces weighted equally (1.0)

---

## Detection Methods

### Keyword Detection

Fast pattern matching using regex:

```rust
fn detect_intent(prompt: &str) -> (IntentType, f32) {
    let lower = prompt.to_lowercase();

    if lower.starts_with("how do") || lower.contains("implement") {
        return (IntentType::HowTo, 0.9);
    }
    // ... more patterns
}
```

**Performance:** <10ms

### LLM Detection

Optional enhanced detection using LLM:

```json
{
  "prompt": "Classify the intent: {user_prompt}",
  "response": {
    "intent": "HowTo",
    "confidence": 0.95,
    "keywords": ["implement", "authentication"]
  }
}
```

**Performance:** ~200ms (with timeout)

### Hybrid Detection

Combines keyword and LLM:

1. Run keyword detection (fast)
2. If confidence < 0.8, optionally use LLM
3. Merge results, prefer LLM if available

## Confidence Scoring

| Score | Meaning | Behavior |
|-------|---------|----------|
| 0.9-1.0 | Very high | Full memory injection (15) |
| 0.7-0.9 | High | Full injection (15) |
| 0.5-0.7 | Medium | Standard injection (10) |
| 0.3-0.5 | Low | Minimal injection (5) |
| 0.0-0.3 | Very low | Skip injection |

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBCOG_SEARCH_INTENT_ENABLED` | Enable detection | `true` |
| `SUBCOG_SEARCH_INTENT_USE_LLM` | Use LLM detection | `true` |
| `SUBCOG_SEARCH_INTENT_LLM_TIMEOUT_MS` | LLM timeout | `200` |
| `SUBCOG_SEARCH_INTENT_MIN_CONFIDENCE` | Min confidence | `0.5` |

### Config File

```yaml
search_intent:
  enabled: true
  use_llm: true
  llm_timeout_ms: 200
  min_confidence: 0.5
```

## Customization

### Custom Patterns

Add custom detection patterns via config:

```yaml
search_intent:
  custom_patterns:
    - pattern: "deploy.*to.*production"
      intent: HowTo
      confidence: 0.9
    - pattern: "security (review|audit)"
      intent: Troubleshoot
      confidence: 0.8
```

### Custom Namespace Weights

Override weights for specific intents:

```yaml
search_intent:
  weights:
    HowTo:
      patterns: 2.0  # Higher weight
      learnings: 1.5
    Troubleshoot:
      security: 1.8  # Custom namespace priority
```

## Performance

| Method | Latency | Accuracy |
|--------|---------|----------|
| Keyword only | <10ms | ~80% |
| LLM only | ~200ms | ~95% |
| Hybrid | ~50ms* | ~90% |

*Hybrid uses LLM only when keyword confidence is low

## Graceful Degradation

| Failure | Fallback |
|---------|----------|
| LLM timeout | Use keyword result |
| LLM error | Use keyword result |
| No pattern match | General intent |
| All failures | Skip injection |

## See Also

- [user-prompt-submit](user-prompt-submit.md) - Hook using intent detection
- [MCP subcog_recall](../mcp/tools.md#subcog_recall) - Search with intent
- [Query Syntax](../QUERY_SYNTAX.md) - Manual filtering

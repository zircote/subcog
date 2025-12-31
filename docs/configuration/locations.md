# File Locations

Subcog stores configuration, data, and cache files in platform-specific locations.

## Configuration Files

### Project Configuration

Always in the project root:

```
<project>/.subcog/config.yaml
```

### User Configuration

| Platform | Path |
|----------|------|
| macOS | `~/.subcog/config.yaml` |
| Linux | `~/.subcog/config.yaml` or `$XDG_CONFIG_HOME/subcog/config.yaml` |
| Windows | `%USERPROFILE%\.subcog\config.yaml` |

### System Configuration

| Platform | Path |
|----------|------|
| macOS | `/etc/subcog/config.yaml` |
| Linux | `/etc/subcog/config.yaml` |
| Windows | `C:\ProgramData\subcog\config.yaml` |

## Data Files

### SQLite Index

| Platform | Default Path |
|----------|--------------|
| macOS | `~/.subcog/index.db` |
| Linux | `~/.subcog/index.db` or `$XDG_DATA_HOME/subcog/index.db` |
| Windows | `%USERPROFILE%\.subcog\index.db` |

Override: `SUBCOG_STORAGE_SQLITE_PATH`

### Vector Index

| Platform | Default Path |
|----------|--------------|
| macOS | `~/.subcog/vectors.usearch` |
| Linux | `~/.subcog/vectors.usearch` or `$XDG_DATA_HOME/subcog/vectors.usearch` |
| Windows | `%USERPROFILE%\.subcog\vectors.usearch` |

Override: `SUBCOG_STORAGE_VECTOR_PATH`

### Git Notes

Stored in the git repository:

```
<project>/.git/refs/notes/subcog
<project>/.git/refs/notes/_prompts
```

## Cache Files

### Embedding Cache

| Platform | Default Path |
|----------|--------------|
| macOS | `~/Library/Caches/subcog/embeddings/` |
| Linux | `~/.cache/subcog/embeddings/` or `$XDG_CACHE_HOME/subcog/embeddings/` |
| Windows | `%LOCALAPPDATA%\subcog\cache\embeddings\` |

### Model Cache

| Platform | Default Path |
|----------|--------------|
| macOS | `~/Library/Caches/subcog/models/` |
| Linux | `~/.cache/subcog/models/` |
| Windows | `%LOCALAPPDATA%\subcog\cache\models\` |

## Log Files

| Platform | Default Path |
|----------|--------------|
| macOS | `~/Library/Logs/subcog/subcog.log` |
| Linux | `~/.local/share/subcog/logs/subcog.log` or `$XDG_DATA_HOME/subcog/logs/` |
| Windows | `%LOCALAPPDATA%\subcog\logs\subcog.log` |

## Claude Code Integration

### Claude Desktop Configuration

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\Claude\claude_desktop_config.json` |

### Claude Code Hooks

Project-specific hooks:

```
<project>/hooks/hooks.json
```

User-level hooks:

| Platform | Path |
|----------|------|
| macOS | `~/.claude/hooks/hooks.json` |
| Linux | `~/.claude/hooks/hooks.json` |
| Windows | `%USERPROFILE%\.claude\hooks\hooks.json` |

## Project Structure

Typical project with Subcog:

```
project/
├── .git/
│   └── refs/
│       └── notes/
│           ├── subcog        # Memory storage
│           └── _prompts      # Prompt templates
├── .subcog/
│   └── config.yaml           # Project configuration
├── hooks/
│   └── hooks.json            # Claude Code hooks
└── ...
```

## User Directory Structure

```
~/.subcog/
├── config.yaml               # User configuration
├── index.db                  # SQLite index
├── vectors.usearch           # Vector index
└── prompts/                  # User prompt templates

~/Library/Caches/subcog/      # macOS
~/.cache/subcog/              # Linux
├── embeddings/               # Embedding cache
│   └── all-MiniLM-L6-v2/
└── models/                   # Model cache
    └── fastembed/
```

## Environment Variable Overrides

| File Type | Variable |
|-----------|----------|
| Config file | `SUBCOG_CONFIG_PATH` |
| SQLite index | `SUBCOG_STORAGE_SQLITE_PATH` |
| Vector index | `SUBCOG_STORAGE_VECTOR_PATH` |
| Git directory | `SUBCOG_GIT_DIR` |

## XDG Base Directory Support (Linux)

Subcog respects XDG Base Directory specification:

| XDG Variable | Used For | Default |
|--------------|----------|---------|
| `$XDG_CONFIG_HOME` | Configuration | `~/.config` |
| `$XDG_DATA_HOME` | Data files | `~/.local/share` |
| `$XDG_CACHE_HOME` | Cache files | `~/.cache` |

## Permissions

### Recommended Permissions

| File Type | Permission |
|-----------|------------|
| Config files | `600` (user read/write only) |
| Data files | `600` |
| Cache directories | `700` |
| Log files | `600` |

### Security Notes

1. **Never store API keys in config files** - Use environment variables
2. **Restrict access to data files** - They may contain sensitive memories
3. **Exclude from backups** if containing secrets

## Disk Usage

Typical disk usage per 1000 memories:

| Component | Size |
|-----------|------|
| Git notes | ~500KB |
| SQLite index | ~2MB |
| Vector index | ~5MB |
| Embedding cache | ~50MB |

## See Also

- [Config File](config-file.md) - Configuration format
- [Environment Variables](environment.md) - Override paths

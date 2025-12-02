# FS MCP Server Tools Guide

This document describes the `fs` MCP server tools and common usage patterns.
All examples assume the server root is the current project directory.

## Tools Overview

- `fs.list_files` — list files/directories (gitignore-aware, with optional metadata).
- `fs.read_file` — read a file or a range from it (bytes or lines).
- `fs.search_text` — search text across files (literal/regex, gitignore-aware).

All tool arguments are JSON objects.

---

## fs.list_files

List files and (optionally) directories relative to the server root.
Respects `.gitignore` / `.ignore` etc. by default.

### Arguments

- `root?: string` — root directory **relative to server root** (e.g. `"."`, `"src"`). Default: `"."`.
- `recursive?: boolean` — recurse into subdirectories. Default: `true`.
- `include_globs?: string[]` — only include paths matching any of these globs.
- `exclude_globs?: string[]` — exclude paths matching any of these globs.
- `max_results?: number` — max entries to return. Default: 500.
- `include_dirs?: boolean` — include directories in results. Default: `false` (files only).
- `include_metadata?: boolean` — include `size`/`modified` fields. Default: `false`.
- `skip?: number` — number of matching entries to skip (for simple paging). Default: `0`.

### Result

```jsonc
{
  "entries": [
    {
      "path": "src/backend.rs",
      "is_dir": false,
      "size": 1234,          // present only if include_metadata = true
      "modified": 1730500000 // UNIX timestamp seconds, only if include_metadata = true
    }
  ],
  "has_more": true
}
```

### Usage Examples

**List project root files (non-recursive):**

```json
{
  "root": ".",
  "recursive": false,
  "max_results": 100
}
```

**List all Rust files under `src/`:**

```json
{
  "root": "src",
  "recursive": true,
  "include_globs": ["**/*.rs"]
}
```

**List entries with basic metadata and simple paging:**

Page 1:
```json
{
  "root": ".",
  "recursive": true,
  "include_metadata": true,
  "max_results": 50,
  "skip": 0
}
```

Page 2:
```json
{
  "root": ".",
  "recursive": true,
  "include_metadata": true,
  "max_results": 50,
  "skip": 50
}
```

---

## fs.read_file

Read a file from the server root, either by bytes or by lines. Designed for
safe, incremental reading of large files.

### Arguments

- `path: string` — file path relative to server root (e.g. `"src/main.rs"`).
- `range_type?: "bytes" | "lines"`
  - If omitted and **line fields** are set → `"lines"`.
  - If omitted otherwise → `"bytes"`.
- **Bytes mode fields**:
  - `offset_bytes?: number` — 0-based byte offset. Default: `0`.
  - `max_bytes?: number` — maximum bytes to read. Default: 64 KiB.
- **Lines mode fields**:
  - `start_line?: number` — 1-based start line. Default: `1`.
  - `max_lines?: number` — maximum number of lines. Default: 200.

> Note: `offset_bytes`/`max_bytes` may only be used with `range_type = "bytes"`.
> `start_line`/`max_lines` may only be used with `range_type = "lines"`.

### Result

```jsonc
{
  "path": "src/backend.rs",
  "content": "file content here ...",
  "is_truncated": true,
  "range": {
    "range_type": "lines",
    "offset_bytes": null,
    "max_bytes": null,
    "start_line": 1,
    "max_lines": 40
  }
}
```

### Usage Examples

**Read config file in one shot (bytes):**

```json
{
  "path": "Cargo.toml",
  "range_type": "bytes",
  "offset_bytes": 0,
  "max_bytes": 4096
}
```

**Page through a log file by bytes:**

Page 1:
```json
{
  "path": "logs/app.log",
  "range_type": "bytes",
  "offset_bytes": 0,
  "max_bytes": 65536
}
```

Page 2:
```json
{
  "path": "logs/app.log",
  "range_type": "bytes",
  "offset_bytes": 65536,
  "max_bytes": 65536
}
```

**Read first 40 lines of a source file (lines, implicit):**

```json
{
  "path": "src/backend.rs",
  "start_line": 1,
  "max_lines": 40
}
```

**Read next 40 lines of the same file:**

```json
{
  "path": "src/backend.rs",
  "range_type": "lines",
  "start_line": 41,
  "max_lines": 40
}
```

---

## fs.search_text

Search for text across files, respecting `.gitignore` / `.ignore` etc.
Supports literal and regex modes, with context lines.

### Arguments

- `query: string` — search query.
- `mode?: "literal" | "regex"` — search mode. Default: `"literal"`.
- `case_sensitive?: boolean` — case sensitivity. Default: `false`.
- `root?: string` — root directory relative to server root (e.g. `"src"`). Default: `"."`.
- `include_globs?: string[]` — only include paths matching any of these globs.
- `exclude_globs?: string[]` — exclude paths matching any of these globs.
- `max_results?: number` — max hits to return. Default: 200.
- `context_lines?: number` — lines of context before/after each hit. Default: 2.
- `skip?: number` — number of initial matches to skip (for simple paging). Default: 0.

> Note: regex mode is **line-based**. Each line is matched independently; `.` does not cross line boundaries.

### Result

```jsonc
{
  "hits": [
    {
      "path": "src/backend.rs",
      "line": 39,
      "column": 17,
      "line_text": "                \"root directory is not a directory: {}\",",
      "context_before": ["            return Err(anyhow!("],
      "context_after": ["                root.display()"]
    }
  ],
  "has_more": false
}
```

### Usage Examples

**Simple literal search in Rust files:**

```json
{
  "query": "serve_server",
  "mode": "literal",
  "case_sensitive": false,
  "root": "rmcp-sdk",
  "include_globs": ["**/*.rs"],
  "max_results": 10,
  "context_lines": 1
}
```

**Regex search for constant definitions:**

```json
{
  "query": "const DEFAULT_MAX_[A-Z_]+: \\\\w+ = \\\\d+",
  "mode": "regex",
  "case_sensitive": false,
  "root": "src",
  "include_globs": ["**/*.rs"],
  "max_results": 10,
  "context_lines": 0
}
```

**Paged search using skip:**

Page 1:
```json
{
  "query": "serve_server",
  "mode": "literal",
  "root": "rmcp-sdk",
  "include_globs": ["**/*.rs"],
  "max_results": 5,
  "skip": 0
}
```

Page 2:
```json
{
  "query": "serve_server",
  "mode": "literal",
  "root": "rmcp-sdk",
  "include_globs": ["**/*.rs"],
  "max_results": 5,
  "skip": 5
}
```

This pattern can be repeated with `skip = page_index * page_size` for simple, deterministic paging.


# FS MCP Server Tools Guide

This document describes the `fs` MCP server tools and common usage patterns.
All examples assume the server root is the current project directory.

## Tools Overview

- `fs.list_files` — list files/directories (gitignore-aware, with optional metadata).
- `fs.read_file` — read a file or a range from it (bytes or lines).
- `fs.search_text` — search text across files (literal/regex, gitignore-aware).
- `fs.stat` — get basic metadata for a single file or directory.
- `fs.path_info` — inspect how a path is resolved and which git repo (if any) it belongs to.
- `fs.create_file` — create or overwrite a file with optional content.
- `fs.overwrite_file` — overwrite an existing file's entire content.
- `fs.delete_path` — delete a file or directory path (with optional recursion).
- `fs.copy_path` — copy a file from one path to another.
- `fs.move_path` — move (rename) a file from one path to another.

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

---

## fs.stat

Quickly check whether a single path exists and basic filesystem metadata for it.

### Arguments

- `path: string` — file or directory path.
  - If relative, it is resolved against the server root.
  - If absolute, it is used as-is.

### Result

```jsonc
{
  "path": "src/backend.rs",
  "exists": true,
  "is_file": true,
  "is_dir": false,
  "size": 1234,
  "modified": 1730500000
}
```

Notes:

- When the path does **not** exist, `exists` is `false` and `size`/`modified` are omitted.
- For relative paths, `path` in the result is relative to the server root when possible.

### Usage Examples

**Check if a source file exists:**

```json
{
  "path": "src/backend.rs"
}
```

**Check an absolute path outside the current project (when allowed):**

```json
{
  "path": "D:/RDM-Media-Player/Devolutions.MultiMediaPlayer.sln"
}
```

---

## fs.path_info

Inspect how a path is resolved by the server, including whether it exists and which git repository it belongs to.

### Arguments

- `path?: string` — optional path to inspect.
  - If omitted or empty, `"."` (the server root) is used.
  - If relative, it is resolved against the server root.
  - If absolute, it is used as-is.

### Result

```jsonc
{
  "input_path": "D:/RDM-Media-Player",
  "resolved_path": "D:/RDM-Media-Player",
  "exists": true,
  "is_file": false,
  "is_dir": true,
  "is_absolute": true,
  "canonical_path": "D:/RDM-Media-Player",
  "repo_root": "D:/RDM-Media-Player"
}
```

### Usage Examples

**Inspect the current server root:**

```json
{
  "path": "."
}
```

**Inspect an absolute directory and its git repo:**

```json
{
  "path": "D:/RDM-Media-Player"
}
```

---

## fs.overwrite_file

Replace the entire content of an existing file.

> This tool requires the target path to already exist and be a regular file.

### Arguments

- `path: string` — file path to overwrite.
  - If relative, it is resolved against the server root.
  - If absolute, it is used as-is but must be inside some git repository.
- `content: string` — new content for the file. The previous content is fully replaced.

### Result

```jsonc
{
  "path": "src/backend.rs"
}
```

### Usage Examples

**Overwrite a config file with new JSON:**

```json
{
  "path": "config/local.json",
  "content": "{ \"debug\": false }\n"
}
```

**Format or normalize a source file after edits:**

```json
{
  "path": "src/backend.rs",
  "content": "<full formatted Rust source here>"
}
```

---

## fs.create_file

Create a new file or overwrite an existing one, optionally writing initial content.

### Arguments

- `path: string` — file path to create.
  - If relative, it is resolved against the server root.
  - If absolute, it is used as-is but must be inside some git repository.
- `content?: string` — optional initial content. Default: empty file.
- `overwrite?: boolean` — overwrite existing file when `true`. Default: `false`.
- `create_parents?: boolean` — create missing parent directories when `true`. Default: `false`.

### Result

```jsonc
{
  "path": "src/new_file.rs",
  "created": true,
  "overwritten": false
}
```

### Usage Examples

**Create a new file under `src/`:**

```json
{
  "path": "src/new_file.rs",
  "content": "fn main() {}\n",
  "overwrite": false,
  "create_parents": true
}
```

**Overwrite an existing config file:**

```json
{
  "path": "config/local.json",
  "content": "{ \"debug\": true }\n",
  "overwrite": true
}
```

---

## fs.delete_path

Delete a file or directory path.

### Arguments

- `path: string` — file or directory path to delete.
  - If relative, it is resolved against the server root.
  - If absolute, it is used as-is but must be inside some git repository.
- `recursive?: boolean` — allow recursive delete for directories. Default: `false`.
- `force?: boolean` — treat non-existent path as success when `true`. Default: `false`.

### Result

```jsonc
{
  "path": "src/old_file.rs",
  "existed": true,
  "is_dir": false,
  "removed": true,
  "recursive": false
}
```

### Usage Examples

**Delete a single file:**

```json
{
  "path": "src/old_file.rs"
}
```

**Recursively delete a directory:**

```json
{
  "path": "target/tmp",
  "recursive": true
}
```

---

## fs.copy_path

Copy a file from one path to another.

> Note: currently only regular files are supported as the source.

### Arguments

- `from: string` — source file path.
- `to: string` — destination file path.
- `overwrite?: boolean` — overwrite destination if it exists. Default: `false`.
- `recursive?: boolean` — reserved for future directory support (currently ignored).
- `create_parents?: boolean` — create missing parent directories for destination. Default: `true`.

### Result

```jsonc
{
  "from": "src/backend.rs",
  "to": "backup/backend.rs",
  "bytes_copied": 4096,
  "overwritten": false
}
```

### Usage Examples

**Backup a source file:**

```json
{
  "from": "src/backend.rs",
  "to": "backup/backend.rs",
  "overwrite": true,
  "create_parents": true
}
```

---

## fs.move_path

Move (rename) a file from one path to another.

> Note: currently only regular files are supported as the source.

### Arguments

- `from: string` — source file path.
- `to: string` — destination file path.
- `overwrite?: boolean` — overwrite destination if it exists. Default: `false`.
- `recursive?: boolean` — reserved for future directory support (currently ignored).
- `create_parents?: boolean` — create missing parent directories for destination. Default: `true`.

### Result

```jsonc
{
  "from": "src/tmp.rs",
  "to": "src/main.rs",
  "existed": true,
  "overwritten": true,
  "recursive": false
}
```

### Usage Examples

**Rename a file in place:**

```json
{
  "from": "src/tmp.rs",
  "to": "src/main.rs",
  "overwrite": true
}
```

# MCP FS Server Test Cases

This document lists suggested manual/automated test cases for the `fs` MCP server
(`list_files`, `read_file`, `search_text`). All examples assume the server root
is the current project directory.

## Conventions

- Tools:
  - `fs.list_files`
  - `fs.read_file`
  - `fs.search_text`
- All arguments are JSON objects (MCP tool params).
- Expected results describe *behavior*, not exact ordering unless specified.

---

## 1. list_files Tests

### 1.1 Basic listing from root

- Tool: `list_files`
- Args:
  ```json
  {
    "root": ".",
    "recursive": true,
    "max_results": 50
  }
  ```
- Expectations:
  - `entries` contains at least:
    - `Cargo.toml`
    - `src/backend.rs`
    - `rmcp-sdk/Cargo.toml`
  - `is_dir` is `false` for files.
  - `has_more` is `true` when more than 50 files exist.

### 1.2 Non-recursive listing

- Tool: `list_files`
- Args:
  ```json
  {
    "root": ".",
    "recursive": false,
    "max_results": 100
  }
  ```
- Expectations:
  - Only files directly under project root are returned.
  - Paths do *not* include nested paths like `rmcp-sdk/...`.

### 1.3 Include globs filter

- Tool: `list_files`
- Args:
  ```json
  {
    "root": ".",
    "recursive": true,
    "include_globs": ["**/*.rs"],
    "max_results": 100
  }
  ```
- Expectations:
  - All returned entries have `.rs` extension.
  - No non-`.rs` files appear in `entries`.

### 1.4 Exclude globs filter

- Tool: `list_files`
- Args:
  ```json
  {
    "root": ".",
    "recursive": true,
    "exclude_globs": ["rmcp-sdk/**"],
    "max_results": 200
  }
  ```
- Expectations:
  - No entry path starts with `rmcp-sdk/`.

### 1.5 Include directories

- Tool: `list_files`
- Args:
  ```json
  {
    "root": ".",
    "recursive": false,
    "include_dirs": true,
    "max_results": 50
  }
  ```
- Expectations:
  - `entries` contains both files and directories.
  - Directories have `is_dir: true`.

### 1.6 Invalid root path

- Tool: `list_files`
- Args:
  ```json
  {
    "root": "this/path/does/not/exist",
    "recursive": true
  }
  ```
- Expectations:
  - Tool call fails with an MCP error.
  - Error message clearly mentions failure to traverse or missing path.

### 1.7 Paging with skip

- Tool: `list_files`
- Step 1 Args:
  ```json
  {
    "root": ".",
    "recursive": true,
    "max_results": 10,
    "skip": 0
  }
  ```
- Step 2 Args:
  ```json
  {
    "root": ".",
    "recursive": true,
    "max_results": 10,
    "skip": 10
  }
  ```
- Expectations:
  - Step 1 and Step 2 `entries` contain disjoint slices of the same overall file list.
  - If the project has more than 20 files, both calls may have `has_more: true`.

---

## 2. read_file Tests

### 2.1 Read small text file (bytes)

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "Cargo.toml",
    "range_type": "bytes",
    "offset_bytes": 0,
    "max_bytes": 4096
  }
  ```
- Expectations:
  - `content` matches full `Cargo.toml`.
  - `is_truncated` is `false`.
  - `range.range_type` is `"bytes"`, `offset_bytes` is `0`, `max_bytes` is `4096`.

### 2.2 Read first lines (lines mode)

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "src/backend.rs",
    "range_type": "lines",
    "start_line": 1,
    "max_lines": 40
  }
  ```
- Expectations:
  - `content` starts with Rust imports from `backend.rs`.
  - `is_truncated` is `true` (file has more than 40 lines).
  - `range.start_line` is `1`, `range.max_lines` is `40`.

### 2.3 Read later lines (pagination by lines)

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "src/backend.rs",
    "range_type": "lines",
    "start_line": 80,
    "max_lines": 40
  }
  ```
- Expectations:
  - `content` contains a later chunk of the file.
  - If end of file is reached, `is_truncated` may be `false`.

### 2.4 Read with byte offset (pagination by bytes)

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "src/backend.rs",
    "range_type": "bytes",
    "offset_bytes": 1024,
    "max_bytes": 4096
  }
  ```
- Expectations:
  - `content` starts from some mid-file location.
  - `is_truncated` indicates whether more data remains after this window.

### 2.5 Non-existent file

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "does/not/exist.rs",
    "range_type": "bytes",
    "offset_bytes": 0,
    "max_bytes": 1024
  }
  ```
- Expectations:
  - Tool call fails with a clear MCP error.
  - Error message mentions open/read failure.

### 2.6 Directory instead of file

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "src",
    "range_type": "bytes",
    "offset_bytes": 0,
    "max_bytes": 1024
  }
  ```
- Expectations:
  - Tool call fails.
  - Error mentions trying to read a directory or invalid file type.

### 2.7 Non-UTF8 file (if available)

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "<some-known-binary-file>",
    "range_type": "bytes",
    "offset_bytes": 0,
    "max_bytes": 1024
  }
  ```
- Expectations:
  - Tool fails with an error indicating non UTF-8 / binary file not supported.

### 2.8 Implicit lines mode when range_type omitted

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "src/backend.rs",
    "start_line": 1,
    "max_lines": 40
  }
  ```
- Expectations:
  - `range.range_type` is `"lines"` even though `range_type` was not provided.
  - `range.start_line` is `1`, `range.max_lines` is `40`.

### 2.9 Implicit bytes mode when only byte range is set

- Tool: `read_file`
- Args:
  ```json
  {
    "path": "src/backend.rs",
    "offset_bytes": 0,
    "max_bytes": 4096
  }
  ```
- Expectations:
  - `range.range_type` is `"bytes"` even though `range_type` was not provided.
  - `range.offset_bytes` is `0`, `range.max_bytes` is `4096`.

### 2.10 Invalid mixed arguments (bytes + lines)

- Tool: `read_file`
- Args example A:
  ```json
  {
    "path": "src/backend.rs",
    "range_type": "bytes",
    "start_line": 1,
    "max_lines": 10
  }
  ```
- Args example B:
  ```json
  {
    "path": "src/backend.rs",
    "range_type": "lines",
    "offset_bytes": 0,
    "max_bytes": 1024
  }
  ```
- Expectations:
  - Both calls fail with clear MCP errors indicating that line-based and byte-based parameters cannot be mixed for the given `range_type`.

---

## 3. search_text Tests

### 3.1 Simple literal search in Rust files

- Tool: `search_text`
- Args:
  ```json
  {
    "query": "serve_server",
    "mode": "literal",
    "case_sensitive": false,
    "root": "rmcp-sdk",
    "include_globs": ["**/*.rs"],
    "max_results": 5,
    "context_lines": 1
  }
  ```
- Expectations:
  - At least one hit in `rmcp-sdk/crates/rmcp/src/service/server.rs`.
  - Each hit has:
    - `path`, `line`, `column`, `line_text`,
    - `context_before` / `context_after` with 0–1 lines each.
  - `has_more` should be `true` if more than 5 usages exist.

### 3.2 Case-insensitive literal search

- Tool: `search_text`
- Args:
  ```json
  {
    "query": "ROOT DIRECTORY IS NOT A DIRECTORY",
    "mode": "literal",
    "case_sensitive": false,
    "root": "src",
    "include_globs": ["**/*.rs"],
    "max_results": 5,
    "context_lines": 1
  }
  ```
- Expectations:
  - Hit in `src/backend.rs` on the error string.
  - Column roughly matches the quote start position.

### 3.3 Regex search

- Tool: `search_text`
- Args:
  ```json
  {
    "query": \"const DEFAULT_MAX_[A-Z_]+: \\\\w+ = \\\\d+\",
    "mode": "regex",
    "case_sensitive": false,
    "root": "src",
    "include_globs": ["**/*.rs"],
    "max_results": 10,
    "context_lines": 0
  }
  ```
- Expectations:
  - Hits include constant definitions in `src/backend.rs`.
  - `column` corresponds to the start of the constant name.

### 3.4 Exclude globs

- Tool: `search_text`
- Args:
  ```json
  {
    "query": "Cargo.toml",
    "mode": "literal",
    "case_sensitive": false,
    "root": ".",
    "include_globs": ["**/*"],
    "exclude_globs": ["rmcp-sdk/**"],
    "max_results": 50,
    "context_lines": 0
  }
  ```
- Expectations:
  - Hits occur only in files outside `rmcp-sdk/`.

### 3.5 No matches

- Tool: `search_text`
- Args:
  ```json
  {
    "query": "THIS_STRING_SHOULD_NOT_EXIST",
    "mode": "literal",
    "case_sensitive": true,
    "root": ".",
    "include_globs": ["**/*"],
    "max_results": 10,
    "context_lines": 1
  }
  ```
- Expectations:
  - `hits` is an empty array.
  - `has_more` is `false`.

### 3.6 Paging with skip

- Tool: `search_text`
- Step 1 Args:
  ```json
  {
    "query": "serve_server",
    "mode": "literal",
    "case_sensitive": false,
    "root": "rmcp-sdk",
    "include_globs": ["**/*.rs"],
    "max_results": 5,
    "context_lines": 1,
    "skip": 0
  }
  ```
- Step 2 Args:
  ```json
  {
    "query": "serve_server",
    "mode": "literal",
    "case_sensitive": false,
    "root": "rmcp-sdk",
    "include_globs": ["**/*.rs"],
    "max_results": 5,
    "context_lines": 1,
    "skip": 5
  }
  ```
- Expectations:
  - Step 2 `hits` start from a later subset of all matches compared to Step 1 (no overlap in `line` for the same `path`).
  - If total matches exceed 10, both calls may have `has_more: true`.

---

## 4. Integration / Sanity Tests

### 4.1 End-to-end navigation

1. Use `list_files` to find a Rust file under `rmcp-sdk/crates/rmcp/src`.
2. Use `read_file` (lines mode) to read its first 60 lines.
3. Use `search_text` constrained to that file path to find a specific symbol.

Expectations:
- All three tools compose cleanly:
  - Paths from `list_files` are valid inputs to `read_file` and `search_text`.
- No unexpected errors when chaining operations.

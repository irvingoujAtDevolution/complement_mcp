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

### 1.1 Basic listing from project root (relative)

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
    - `llm/test/test_cases.md`
  - All `path` values are **relative to the server root** (project directory), e.g. `src/backend.rs`.
  - `is_dir` is `false` for files.
  - `has_more` is `true` when more than 50 files exist.

### 1.2 Non-recursive listing (relative root)

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

### 1.6 Invalid root path (relative)

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

### 1.7 Root escaping repository (relative)

- Tool: `list_files`
- Args:
  ```json
  {
    "root": "../outside",
    "recursive": true
  }
  ```
- Expectations:
  - Tool call fails with an MCP error.
  - Error message indicates that the root escapes the repository root.

### 1.8 Paging with skip (relative root)

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

### 1.9 Absolute root inside a git repository

- Tool: `list_files`
- Args (example for Windows):
  ```json
  {
    "root": "D:/RDM",
    "recursive": false,
    "include_dirs": true,
    "max_results": 100
  }
  ```
- Preconditions:
  - `D:/RDM` exists, is a directory, and is inside some git repository (has a `.git` ancestor).
- Expectations:
  - Tool call succeeds.
  - `entries` lists files/directories directly under `D:/RDM`.
  - Each `path` is **relative to the absolute root** `D:/RDM` (e.g. `subdir/file.txt`), not relative to the server project root.

### 1.10 Absolute root outside any git repository

- Tool: `list_files`
- Args (example):
  ```json
  {
    "root": "C:/Windows",
    "recursive": false
  }
  ```
- Preconditions:
  - `C:/Windows` (or chosen path) is not inside any git repository (no `.git` ancestor).
- Expectations:
  - Tool call fails with an MCP error.
  - Error message indicates that the root is not inside a git repository.

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
    "query": "LocalGitAwareFs",
    "mode": "literal",
    "case_sensitive": false,
    "root": "src",
    "include_globs": ["**/*.rs"],
    "max_results": 5,
    "context_lines": 1
  }
  ```
- Expectations:
  - At least one hit in `src/backend.rs`.
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
    "query": "LocalGitAwareFs",
    "mode": "literal",
    "case_sensitive": false,
    "root": "src",
    "include_globs": ["**/*.rs"],
    "max_results": 5,
    "context_lines": 1,
    "skip": 0
  }
  ```
- Step 2 Args:
  ```json
  {
    "query": "LocalGitAwareFs",
    "mode": "literal",
    "case_sensitive": false,
    "root": "src",
    "include_globs": ["**/*.rs"],
    "max_results": 5,
    "context_lines": 1,
    "skip": 5
  }
  ```
- Expectations:
  - Step 2 `hits` start from a later subset of all matches compared to Step 1 (no overlap in `line` for the same `path`).
  - If total matches exceed 10, both calls may have `has_more: true`.

### 3.7 Absolute root inside a git repository

- Tool: `search_text`
- Args (example for Windows):
  ```json
  {
    "query": "some_symbol_name",
    "mode": "literal",
    "case_sensitive": false,
    "root": "D:/RDM",
    "include_globs": ["**/*"],
    "max_results": 10,
    "context_lines": 1
  }
  ```
- Preconditions:
  - `D:/RDM` exists, is a directory, and is inside some git repository (has a `.git` ancestor).
- Expectations:
  - Tool call succeeds.
  - `hits` (if any) only come from files under `D:/RDM`.
  - `path` values in hits all point to files under `D:/RDM` (either as relative paths under that directory or as absolute paths within it, depending on how the server root is configured).

---

## 4. Integration / Sanity Tests

### 4.1 End-to-end navigation

1. Use `list_files` to find a Rust file under `src` (for example `src/backend.rs`).
2. Use `read_file` (lines mode) to read its first 60 lines.
3. Use `search_text` constrained to that file path (e.g. via `include_globs` using the same `path` returned by `list_files`) to find a specific symbol, such as `\"LocalGitAwareFs\"`.

Expectations:
- All three tools compose cleanly:
  - Paths from `list_files` are valid inputs to `read_file` (via `path`) and to `search_text` (via `include_globs`).
- No unexpected errors when chaining operations.

---

## 5. find_files Tests

### 5.1 Basic name search from project root (relative)

- Tool: `find_files`
- Args:
  ```json
  {
    "query": "backend.rs",
    "root": ".",
    "recursive": true,
    "match_mode": "name",
    "max_results": 10
  }
  ```
- Expectations:
  - `matches` contains at least one entry with `path` equal to `src/backend.rs`.
  - `is_dir` is `false` for that entry.

### 5.2 Path search under a subdirectory (relative)

- Tool: `find_files`
- Args:
  ```json
  {
    "query": "src/backend.rs",
    "root": ".",
    "recursive": true,
    "match_mode": "path",
    "max_results": 50
  }
  ```
- Expectations:
  - All returned `path` values are relative to the project root.
  - At least one match has `path` equal to `src/backend.rs`.

### 5.3 Case-insensitive name search

- Tool: `find_files`
- Args:
  ```json
  {
    "query": "CARGO.TOML",
    "root": ".",
    "recursive": false,
    "match_mode": "name",
    "case_sensitive": false,
    "max_results": 10
  }
  ```
- Expectations:
  - At least one match has `path` equal to `Cargo.toml`.

### 5.4 Exclude and include globs

- Tool: `find_files`
- Args:
  ```json
  {
    "query": "Cargo.toml",
    "root": ".",
    "recursive": true,
    "match_mode": "name",
    "include_globs": ["**/Cargo.toml"],
    "exclude_globs": ["rmcp-sdk/**"],
    "max_results": 20
  }
  ```
- Expectations:
  - All matches have file name `Cargo.toml`.
  - No match has a `path` starting with `rmcp-sdk/`.

### 5.5 Invalid root (relative)

- Tool: `find_files`
- Args:
  ```json
  {
    "query": "anything",
    "root": "this/path/does/not/exist",
    "recursive": true
  }
  ```
- Expectations:
  - Tool call fails with an MCP error.
  - Error message clearly mentions that the root path does not exist or is not a directory.

### 5.6 Root escaping repository (relative)

- Tool: `find_files`
- Args:
  ```json
  {
    "query": "anything",
    "root": "../outside",
    "recursive": true
  }
  ```
- Expectations:
  - Tool call fails with an MCP error.
  - Error message indicates that the root escapes the repository root.

### 5.7 Absolute root inside a git repository

- Tool: `find_files`
- Args (example for Windows):
  ```json
  {
    "query": ".sln",
    "root": "D:/RDM",
    "recursive": false,
    "match_mode": "name",
    "include_dirs": false,
    "max_results": 50
  }
  ```
- Preconditions:
  - `D:/RDM` exists, is a directory, and is inside some git repository (has a `.git` ancestor).
- Expectations:
  - Tool call succeeds.
  - `matches` contain entries like `Devolutions.Server.sln` (relative to `D:/RDM`).
  - All `path` values are relative to `D:/RDM` when possible.

### 5.8 Absolute root outside any git repository

- Tool: `find_files`
- Args (example):
  ```json
  {
    "query": "anything",
    "root": "C:/Windows",
    "recursive": false
  }
  ```
- Preconditions:
  - `C:/Windows` (or chosen path) is not inside any git repository (no `.git` ancestor).
- Expectations:
  - Tool call fails with an MCP error.
  - Error message indicates that the root is not inside a git repository.

---

## 6. stat Tests

### 6.1 Existing file (relative path)

- Tool: `stat`
- Args:
  ```json
  {
    "path": "src/backend.rs"
  }
  ```
- Expectations:
  - `exists` is `true`.
  - `is_file` is `true`, `is_dir` is `false`.
  - `path` is `src/backend.rs` (relative to server root).
  - `size` and `modified` are present and non-zero.

### 6.2 Existing directory (relative path)

- Tool: `stat`
- Args:
  ```json
  {
    "path": "src"
  }
  ```
- Expectations:
  - `exists` is `true`.
  - `is_dir` is `true`, `is_file` is `false`.
  - `size` is omitted or `null`.

### 6.3 Non-existent path (relative)

- Tool: `stat`
- Args:
  ```json
  {
    "path": "this/path/does/not/exist"
  }
  ```
- Expectations:
  - Call succeeds without MCP error.
  - `exists` is `false`.
  - `is_file` and `is_dir` are `false`.
  - `size` and `modified` are omitted.

### 6.4 Absolute file path

- Tool: `stat`
- Args (example for Windows):
  ```json
  {
    "path": "D:/RDM-Media-Player/Devolutions.MultiMediaPlayer.sln"
  }
  ```
- Preconditions:
  - The given `.sln` file exists on disk.
- Expectations:
  - `exists` is `true`.
  - `is_file` is `true`.
  - `path` is an absolute or canonical path to the solution file.

---

## 7. path_info Tests

### 7.1 Current server root

- Tool: `path_info`
- Args:
  ```json
  {
    "path": "."
  }
  ```
- Expectations:
  - `input_path` is `"."`.
  - `resolved_path` points to the project root directory.
  - `exists` is `true`, `is_dir` is `true`.
  - `is_absolute` reflects whether `resolved_path` is absolute.
  - `canonical_path` is present and points to the same directory.
  - `repo_root` is the same as `canonical_path` (project git root).

### 7.2 Existing absolute directory with git repo

- Tool: `path_info`
- Args (example for Windows):
  ```json
  {
    "path": "D:/RDM-Media-Player"
  }
  ```
- Preconditions:
  - `D:/RDM-Media-Player` exists and has a `.git` ancestor.
- Expectations:
  - `exists` is `true`, `is_dir` is `true`.
  - `is_absolute` is `true`.
  - `canonical_path` is present and matches the canonical form of `D:/RDM-Media-Player`.
  - `repo_root` is the git repository root containing that directory.

### 7.3 Non-existent absolute path

- Tool: `path_info`
- Args (example):
  ```json
  {
    "path": "C:/definitely/not/here"
  }
  ```
- Expectations:
  - `exists` is `false`.
  - `canonical_path` is `null` / omitted.
  - `repo_root` is `null` / omitted.

### 7.4 Relative path under server root

- Tool: `path_info`
- Args:
  ```json
  {
    "path": "src/backend.rs"
  }
  ```
- Expectations:
  - `resolved_path` is under the project root.
  - `exists` is `true`, `is_file` is `true`.
  - `canonical_path` is present and points to the same file location.
  - `repo_root` matches the project git root.

---

## 8. overwrite_file Tests

### 8.1 Overwrite existing file

- Tool: `overwrite_file`
- Args:
  ```json
  {
    "path": "src/backend.rs",
    "content": "// overwritten by test\n"
  }
  ```
- Expectations:
  - Tool call succeeds.
  - `path` in the result is `src/backend.rs`.
  - A subsequent `read_file` on `src/backend.rs` (lines mode) returns content starting with `"// overwritten by test"`.

> Note: in automated tests, this should be done on a temporary copy of `backend.rs`
> (e.g., copy the file to a temp path, overwrite that temp path, and then delete it).

### 8.2 Non-existent file

- Tool: `overwrite_file`
- Args:
  ```json
  {
    "path": "this/file/does/not/exist.rs",
    "content": "hello\n"
  }
  ```
- Expectations:
  - Tool call fails with an MCP error.
  - Error message clearly indicates a metadata/open failure for the target path.

### 8.3 Directory instead of file

- Tool: `overwrite_file`
- Args:
  ```json
  {
    "path": "src",
    "content": "hello\n"
  }
  ```
- Expectations:
  - Tool call fails with an MCP error.
  - Error message indicates that `overwrite_file` only supports regular files (not directories).

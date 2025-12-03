use rmcp::schemars;
use serde::{Deserialize, Serialize};

/// Search mode for `search_text` tool.
///
/// - `literal` (default): simple substring search.
/// - `regex`: line-by-line regular expression search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    Literal,
    Regex,
}

/// Range type for `read_file` tool.
///
/// - `bytes`: use `offset_bytes` / `max_bytes`.
/// - `lines`: use `start_line` / `max_lines`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RangeType {
    Bytes,
    Lines,
}

/// Match mode for `find_files` tool.
///
/// - `name` (default): match only against the file or directory name.
/// - `path`: match against the full relative path (e.g. "src/main.rs").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FindMatchMode {
    Name,
    Path,
}

/// Arguments for `search_text`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchTextArgs {
    /// Search query string. Literal or regex depending on `mode`.
    pub query: String,

    /// Optional. `"literal"` (default) or `"regex"`.
    #[serde(default)]
    pub mode: Option<SearchMode>,

    /// Optional. Case sensitivity for literal/regex search. Default: false.
    #[serde(default)]
    pub case_sensitive: Option<bool>,

    /// Optional. Root directory.
    ///
    /// - If relative, it is resolved against the server root (e.g. "src").
    /// - If absolute, it must be inside some git repository and is used as the search base.
    ///   In this case, result paths are relative to that absolute root when possible.
    #[serde(default)]
    pub root: Option<String>,

    /// Optional. Only include files matching any of these glob patterns.
    #[serde(default)]
    pub include_globs: Option<Vec<String>>,

    /// Optional. Exclude files matching any of these glob patterns.
    #[serde(default)]
    pub exclude_globs: Option<Vec<String>>,

    /// Optional. Maximum number of hits to return. Default: 200.
    #[serde(default)]
    pub max_results: Option<u32>,

    /// Optional. Number of context lines before/after each hit. Default: 2.
    #[serde(default)]
    pub context_lines: Option<u32>,

    /// Optional. Number of initial matches to skip (for simple paging). Default: 0.
    #[serde(default)]
    pub skip: Option<u32>,
}

/// Arguments for `read_file`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadFileArgs {
    /// File path relative to server root (e.g. "src/main.rs").
    pub path: String,

    /// Optional. `"bytes"` or `"lines"`.
    /// If omitted, `"lines"` is used when any of `start_line`/`max_lines` is set,
    /// otherwise `"bytes"` is used.
    #[serde(default)]
    pub range_type: Option<RangeType>,

    /// Optional. Byte offset for `"bytes"` mode. Default: 0.
    #[serde(default)]
    pub offset_bytes: Option<u64>,

    /// Optional. Maximum bytes to read in `"bytes"` mode. Default: 64 KiB.
    #[serde(default)]
    pub max_bytes: Option<u64>,

    /// Optional. 1-based start line for `"lines"` mode. Default: 1.
    #[serde(default)]
    pub start_line: Option<u64>,

    /// Optional. Maximum lines to read in `"lines"` mode. Default: 200.
    #[serde(default)]
    pub max_lines: Option<u64>,
}

/// Arguments for `list_files`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListFilesArgs {
    /// Optional. Root directory.
    ///
    /// - If relative, it is resolved against the server root. Returned paths are
    ///   relative to the server root.
    /// - If absolute, it must be inside some git repository and is used as the
    ///   listing base. Returned paths are relative to that absolute root when possible.
    #[serde(default)]
    pub root: Option<String>,

    /// Optional. Whether to recurse into subdirectories. Default: true.
    #[serde(default)]
    pub recursive: Option<bool>,

    /// Optional. Only include files/directories matching any of these globs.
    #[serde(default)]
    pub include_globs: Option<Vec<String>>,

    /// Optional. Exclude any paths matching these globs.
    #[serde(default)]
    pub exclude_globs: Option<Vec<String>>,

    /// Optional. Maximum number of entries to return. Default: 500.
    #[serde(default)]
    pub max_results: Option<u32>,

    /// Optional. Include directories in results. Default: false (files only).
    #[serde(default)]
    pub include_dirs: Option<bool>,

    /// Optional. Include basic metadata (size, modified) in entries. Default: false.
    #[serde(default)]
    pub include_metadata: Option<bool>,

    /// Optional. Number of matching entries to skip before collecting results. Default: 0.
    #[serde(default)]
    pub skip: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchHit {
    pub path: String,
    pub line: u64,
    pub column: u64,
    pub line_text: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchTextResult {
    pub hits: Vec<SearchHit>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileRangeInfo {
    pub range_type: RangeType,
    pub offset_bytes: Option<u64>,
    pub max_bytes: Option<u64>,
    pub start_line: Option<u64>,
    pub max_lines: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileChunkResult {
    pub path: String,
    pub content: String,
    pub is_truncated: bool,
    pub range: FileRangeInfo,
}

/// A single entry in `list_files` result.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FileEntry {
    /// File or directory path.
    ///
    /// - For relative roots, this is relative to the server root.
    /// - For absolute roots, this is relative to the absolute root when possible.
    pub path: String,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Optional. File size in bytes (only when `include_metadata` is enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Optional. Last modified time as UNIX timestamp seconds (only when `include_metadata` is enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ListFilesResult {
    pub entries: Vec<FileEntry>,
    pub has_more: bool,
}

/// Arguments for `find_files`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FindFilesArgs {
    /// Pattern to search for in file/directory names or paths.
    pub query: String,

    /// Optional. Root directory.
    ///
    /// - If relative, it is resolved against the server root.
    /// - If absolute, it must be inside some git repository and is used as the
    ///   search base. Returned paths are relative to that absolute root when possible.
    #[serde(default)]
    pub root: Option<String>,

    /// Optional. Whether to recurse into subdirectories. Default: true.
    #[serde(default)]
    pub recursive: Option<bool>,

    /// Optional. Only include files/directories matching any of these globs.
    #[serde(default)]
    pub include_globs: Option<Vec<String>>,

    /// Optional. Exclude any paths matching these globs.
    #[serde(default)]
    pub exclude_globs: Option<Vec<String>>,

    /// Optional. Match mode: `\"name\"` (default) or `\"path\"`.
    #[serde(default)]
    pub match_mode: Option<FindMatchMode>,

    /// Optional. Case sensitivity for query matching. Default: false.
    #[serde(default)]
    pub case_sensitive: Option<bool>,

    /// Optional. Include directories in results. Default: true.
    #[serde(default)]
    pub include_dirs: Option<bool>,

    /// Optional. Maximum number of matches to return. Default: 200.
    #[serde(default)]
    pub max_results: Option<u32>,

    /// Optional. Number of matching entries to skip before collecting results. Default: 0.
    #[serde(default)]
    pub skip: Option<u32>,
}

/// A single match in `find_files` result.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FindFileMatch {
    /// File or directory path.
    ///
    /// - For relative roots, this is relative to the server root.
    /// - For absolute roots, this is relative to the absolute root when possible.
    pub path: String,
    /// Whether this match is a directory.
    pub is_dir: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FindFilesResult {
    pub matches: Vec<FindFileMatch>,
    pub has_more: bool,
}

/// Arguments for `stat`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct StatArgs {
    /// File or directory path.
    ///
    /// - If relative, it is resolved against the server root.
    /// - If absolute, it is used as-is.
    pub path: String,
}

/// Result for `stat`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct StatResult {
    /// Resolved path.
    ///
    /// - For relative paths, this is relative to the server root when possible.
    /// - For absolute paths, this is the canonical absolute path, or the
    ///   original path when canonicalization is not available.
    pub path: String,
    /// Whether the path exists.
    pub exists: bool,
    /// Whether the path is a regular file (when it exists).
    pub is_file: bool,
    /// Whether the path is a directory (when it exists).
    pub is_dir: bool,
    /// Optional file size in bytes (only for existing regular files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Optional last modified time as UNIX timestamp seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
}

/// Arguments for `path_info`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PathInfoArgs {
    /// Optional path to inspect.
    ///
    /// - If omitted or empty, `"."` (the server root) is used.
    /// - If relative, it is resolved against the server root.
    /// - If absolute, it is used as-is.
    #[serde(default)]
    pub path: Option<String>,
}

/// Result for `path_info`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PathInfoResult {
    /// The path argument as given (or `"."` when omitted).
    pub input_path: String,
    /// The resolved path before canonicalization.
    pub resolved_path: String,
    /// Whether the path exists according to the filesystem.
    pub exists: bool,
    /// Whether the path is a regular file (when it exists).
    pub is_file: bool,
    /// Whether the path is a directory (when it exists).
    pub is_dir: bool,
    /// Whether the resolved path is absolute.
    pub is_absolute: bool,
    /// Canonicalized path, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    /// Git repository root containing this path (directory with `.git`), when any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_root: Option<String>,
}

/// Arguments for `create_file`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateFileArgs {
    /// File path to create.
    ///
    /// - If relative, it is resolved against the server root.
    /// - If absolute, it is used as-is (subject to server-side safety checks).
    pub path: String,

    /// Optional initial content for the file. Default: empty file.
    #[serde(default)]
    pub content: Option<String>,

    /// Optional. Overwrite existing file if it already exists. Default: false.
    #[serde(default)]
    pub overwrite: Option<bool>,

    /// Optional. Create parent directories as needed. Default: false.
    #[serde(default)]
    pub create_parents: Option<bool>,
}

/// Result for `create_file`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CreateFileResult {
    /// Final path of the file (relative to server root when possible).
    pub path: String,
    /// Whether a new file was created.
    pub created: bool,
    /// Whether an existing file was overwritten.
    pub overwritten: bool,
}

/// Arguments for `delete_path`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DeletePathArgs {
    /// File or directory path to delete.
    ///
    /// - If relative, it is resolved against the server root.
    /// - If absolute, it is used as-is (subject to server-side safety checks).
    pub path: String,

    /// Optional. Allow recursive delete for directories. Default: false.
    #[serde(default)]
    pub recursive: Option<bool>,

    /// Optional. If true, a non-existent path is treated as success. Default: false.
    #[serde(default)]
    pub force: Option<bool>,
}

/// Result for `delete_path`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DeletePathResult {
    /// The resolved path that was targeted for deletion.
    pub path: String,
    /// Whether the path existed before the delete.
    pub existed: bool,
    /// Whether the path was a directory (when it existed).
    pub is_dir: bool,
    /// Whether anything was actually removed.
    pub removed: bool,
    /// Whether deletion was performed recursively.
    pub recursive: bool,
}

/// Arguments for `copy_path`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CopyPathArgs {
    /// Source file or directory.
    pub from: String,
    /// Destination file or directory.
    pub to: String,

    /// Optional. Overwrite destination if it exists. Default: false.
    #[serde(default)]
    pub overwrite: Option<bool>,

    /// Optional. Allow copying directories recursively. Default: false.
    #[serde(default)]
    pub recursive: Option<bool>,

    /// Optional. Create parent directories for the destination. Default: true.
    #[serde(default)]
    pub create_parents: Option<bool>,
}

/// Result for `copy_path`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CopyPathResult {
    /// Resolved source path.
    pub from: String,
    /// Resolved destination path.
    pub to: String,
    /// Optional total bytes copied (for single-file copies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_copied: Option<u64>,
    /// Whether the destination overwrote an existing file.
    pub overwritten: bool,
}

/// Arguments for `move_path`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MovePathArgs {
    /// Source file or directory.
    pub from: String,
    /// Destination file or directory.
    pub to: String,

    /// Optional. Overwrite destination if it exists. Default: false.
    #[serde(default)]
    pub overwrite: Option<bool>,

    /// Optional. Allow moving directories recursively. Default: true.
    #[serde(default)]
    pub recursive: Option<bool>,

    /// Optional. Create parent directories for the destination. Default: true.
    #[serde(default)]
    pub create_parents: Option<bool>,
}

/// Result for `move_path`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MovePathResult {
    /// Resolved source path before the move.
    pub from: String,
    /// Resolved destination path after the move.
    pub to: String,
    /// Whether the source existed before the move.
    pub existed: bool,
    /// Whether an existing destination was overwritten.
    pub overwritten: bool,
    /// Whether the move was performed recursively (for directories).
    pub recursive: bool,
}

/// Arguments for `overwrite_file`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct OverwriteFileArgs {
    /// File path whose entire content will be replaced.
    ///
    /// - If relative, it is resolved against the server root.
    /// - If absolute, it is used as-is (subject to server-side safety checks).
    pub path: String,

    /// New content for the file. Existing content is fully replaced.
    pub content: String,
}

/// Result for `overwrite_file`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct OverwriteFileResult {
    /// Final path of the overwritten file (relative to server root when possible).
    pub path: String,
}

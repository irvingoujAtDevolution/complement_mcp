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

    /// Optional. Root directory relative to server root (e.g. "src"). Default: ".".
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
    /// Optional. Root directory relative to server root. Default: ".".
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
    /// File or directory path relative to server root.
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

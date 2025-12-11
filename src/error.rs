use std::io;
use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, FsError>;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("failed to canonicalize root directory {root}: {source}")]
    CanonicalizeRoot {
        root: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to canonicalize path {path}: {source}")]
    CanonicalizePath {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("path escapes repository root: {path}")]
    PathEscapesRepo { path: PathBuf },

    #[error("list_files root path does not exist: {path}")]
    ListRootNotExist { path: PathBuf },

    #[error("list_files root path is not a directory: {path}")]
    ListRootNotDirectory { path: PathBuf },

    #[error("list_files root path is not inside a git repository: {path}")]
    ListRootNotInGit { path: PathBuf },

    #[error("find_files root path does not exist: {path}")]
    FindRootNotExist { path: PathBuf },

    #[error("find_files root path is not a directory: {path}")]
    FindRootNotDirectory { path: PathBuf },

    #[error("find_files root path is not inside a git repository: {path}")]
    FindRootNotInGit { path: PathBuf },

    #[error("search_text root path is not inside a git repository: {path}")]
    SearchRootNotInGit { path: PathBuf },

    #[error("search_text root escapes repository root: {path}")]
    SearchRootEscapesRepo { path: PathBuf },

    #[error("read_file path does not exist: {path}")]
    ReadFilePathNotExist { path: PathBuf },

    #[error("read_file permission denied for path: {path}")]
    ReadFilePermissionDenied { path: PathBuf },

    #[error("read_file target is not a regular file: {path}")]
    ReadFileNotFile { path: PathBuf },

    #[error("write path is not inside a git repository: {path}")]
    WritePathNotInGit { path: PathBuf },

    #[error("invalid glob pattern: {pattern}")]
    InvalidGlobPattern {
        pattern: String,
        #[source]
        source: globset::Error,
    },

    #[error("invalid find_files query regex {query}: {source}")]
    InvalidFindFilesRegex {
        query: String,
        #[source]
        source: regex::Error,
    },

    #[error("invalid search_text regex {query}: {source}")]
    InvalidSearchRegex {
        query: String,
        #[source]
        source: regex::Error,
    },

    #[error("root directory is not a directory: {path}")]
    RootNotDirectory { path: PathBuf },

    #[error(
        "invalid read_file arguments: start_line/max_lines cannot be used with bytes range_type"
    )]
    ReadFileLinesWithBytes,

    #[error(
        "invalid read_file arguments: offset_bytes/max_bytes cannot be used with lines range_type"
    )]
    ReadFileBytesWithLines,

    #[error("start_line must be >= 1")]
    StartLineMustBePositive,

    #[error("failed to open file {path}: {source}")]
    OpenFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to seek in file {path}: {source}")]
    SeekFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to read file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to write file {path}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("file is not valid UTF-8, binary files are not supported: {path}")]
    FileNotUtf8 { path: PathBuf },

    #[error("failed to get metadata for {path}: {source}")]
    FileMetadata {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to read line from {path}: {source}")]
    ReadLine {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to delete path {path}: {source}")]
    DeletePath {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("cannot delete directory without recursive=true: {path}")]
    DeleteDirNonRecursive { path: PathBuf },

    #[error("destination already exists (use overwrite=true to replace): {path}")]
    DestinationExists { path: PathBuf },

    #[error("failed to create parent directories for {path}: {source}")]
    CreateParents {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to copy from {from} to {to}: {source}")]
    CopyPath {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to move from {from} to {to}: {source}")]
    MovePath {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("copy across different git repositories is not allowed: {from} -> {to}")]
    CopyAcrossRepos { from: PathBuf, to: PathBuf },

    #[error("move across different git repositories is not allowed: {from} -> {to}")]
    MoveAcrossRepos { from: PathBuf, to: PathBuf },
}

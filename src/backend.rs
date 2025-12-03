use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use memmap2::Mmap;
use regex::bytes::RegexBuilder as ByteRegexBuilder;

use crate::error::{FsError, Result};
use crate::types::{
    CopyPathArgs, CopyPathResult, CreateFileArgs, CreateFileResult, DeletePathArgs,
    DeletePathResult, FileChunkResult, FileEntry, FileRangeInfo, FindFileMatch, FindFilesArgs,
    FindFilesResult, FindMatchMode, ListFilesArgs, ListFilesResult, MovePathArgs, MovePathResult,
    OverwriteFileArgs, OverwriteFileResult, PathInfoArgs, PathInfoResult, RangeType, ReadFileArgs,
    SearchHit, SearchMode, SearchTextArgs, SearchTextResult, StatArgs, StatResult,
};

const DEFAULT_MAX_SEARCH_RESULTS: u32 = 200;
const DEFAULT_SEARCH_CONTEXT_LINES: u32 = 2;
const DEFAULT_MAX_READ_BYTES: u64 = 64 * 1024;
const DEFAULT_MAX_READ_LINES: u64 = 200;
const DEFAULT_LIST_MAX_RESULTS: u32 = 500;

#[derive(Clone)]
pub struct LocalGitAwareFs {
    root: PathBuf,
}

impl LocalGitAwareFs {
    pub fn new(root: PathBuf) -> Result<Self> {
        let root = if root.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            root
        };
        let root = root
            .canonicalize()
            .map_err(|source| FsError::CanonicalizeRoot {
                root: root.clone(),
                source,
            })?;

        if !root.is_dir() {
            return Err(FsError::RootNotDirectory { path: root });
        }

        Ok(Self { root })
    }

    fn resolve_path(&self, rel: &str) -> Result<PathBuf> {
        let rel_path = Path::new(rel);
        let is_absolute = rel_path.is_absolute();

        let joined = if is_absolute {
            rel_path.to_path_buf()
        } else {
            self.root.join(rel_path)
        };

        let canonical = joined
            .canonicalize()
            .map_err(|source| FsError::CanonicalizePath {
                path: joined.clone(),
                source,
            })?;

        if !is_absolute && !canonical.starts_with(&self.root) {
            return Err(FsError::PathEscapesRepo { path: canonical });
        }

        Ok(canonical)
    }

    fn build_globset(patterns: &Option<Vec<String>>) -> Result<Option<GlobSet>> {
        let mut builder = GlobSetBuilder::new();
        let mut any = false;

        if let Some(pats) = patterns {
            for pat in pats {
                let glob = Glob::new(pat).map_err(|source| FsError::InvalidGlobPattern {
                    pattern: pat.clone(),
                    source,
                })?;
                builder.add(glob);
                any = true;
            }
        }

        if any {
            let built = builder
                .build()
                .map_err(|source| FsError::InvalidGlobPattern {
                    pattern: "<compiled globset>".to_string(),
                    source,
                })?;
            Ok(Some(built))
        } else {
            Ok(None)
        }
    }

    fn find_git_root(start: &Path) -> Option<PathBuf> {
        let mut current = Some(start.to_path_buf());
        while let Some(dir) = current {
            let git_dir = dir.join(".git");
            if git_dir.exists() {
                return Some(dir);
            }
            current = dir.parent().map(Path::to_path_buf);
        }
        None
    }

    pub fn list_files(&self, args: ListFilesArgs) -> Result<ListFilesResult> {
        let root_arg = args.root.as_deref().unwrap_or(".");
        let root_path = Path::new(root_arg);
        let is_absolute = root_path.is_absolute();

        let start_path = if is_absolute {
            root_path
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: root_path.to_path_buf(),
                    source,
                })?
        } else {
            let joined = self.root.join(root_path);
            let canonical = joined
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: joined.clone(),
                    source,
                })?;

            if !canonical.starts_with(&self.root) {
                return Err(FsError::PathEscapesRepo { path: canonical });
            }

            canonical
        };

        let recursive = args.recursive.unwrap_or(true);
        let include_dirs = args.include_dirs.unwrap_or(false);
        let max_results = args.max_results.unwrap_or(DEFAULT_LIST_MAX_RESULTS);
        let skip = args.skip.unwrap_or(0);
        let include_metadata = args.include_metadata.unwrap_or(false);

        if !start_path.exists() {
            return Err(FsError::ListRootNotExist {
                path: start_path.clone(),
            });
        }
        if !start_path.is_dir() {
            return Err(FsError::ListRootNotDirectory {
                path: start_path.clone(),
            });
        }

        if is_absolute && Self::find_git_root(&start_path).is_none() {
            return Err(FsError::ListRootNotInGit {
                path: start_path.clone(),
            });
        }

        let include_globs: Option<GlobSet> = Self::build_globset(&args.include_globs)?;
        let exclude_globs: Option<GlobSet> = Self::build_globset(&args.exclude_globs)?;

        let mut builder = WalkBuilder::new(&start_path);
        builder.standard_filters(true);
        if !recursive {
            builder.max_depth(Some(1));
        }

        let mut entries = Vec::new();
        let mut seen: u32 = 0;
        let mut hit_limit = false;
        for result in builder.build() {
            let entry = match result {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("list_files: skip entry error: {err}");
                    continue;
                }
            };

            let path = entry.path();
            if path == start_path {
                continue;
            }
            let rel = match path.strip_prefix(&start_path) {
                Ok(r) => r,
                Err(_) => path.strip_prefix(&self.root).unwrap_or(path),
            };

            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir && !include_dirs {
                continue;
            }

            let rel_str = rel.to_string_lossy();

            if let Some(ref excludes) = exclude_globs
                && excludes.is_match(rel_str.as_ref())
            {
                continue;
            }

            if let Some(ref includes) = include_globs
                && !includes.is_match(rel_str.as_ref())
            {
                continue;
            }

            seen = seen.saturating_add(1);

            if seen <= skip {
                continue;
            }

            let mut size = None;
            let mut modified = None;

            if include_metadata && let Ok(meta) = std::fs::metadata(path) {
                size = Some(meta.len());
                if let Ok(time) = meta.modified()
                    && let Ok(dur) = time.duration_since(SystemTime::UNIX_EPOCH)
                {
                    modified = Some(dur.as_secs());
                }
            }

            entries.push(FileEntry {
                path: rel_str.into_owned(),
                is_dir,
                size,
                modified,
            });

            if entries.len() as u32 >= max_results {
                hit_limit = true;
                break;
            }
        }

        Ok(ListFilesResult {
            entries,
            has_more: hit_limit,
        })
    }

    pub fn find_files(&self, args: FindFilesArgs) -> Result<FindFilesResult> {
        let root_arg = args.root.as_deref().unwrap_or(".");
        let root_path = Path::new(root_arg);
        let is_absolute = root_path.is_absolute();

        let start_path = if is_absolute {
            root_path
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: root_path.to_path_buf(),
                    source,
                })?
        } else {
            let joined = self.root.join(root_path);
            let canonical = joined
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: joined.clone(),
                    source,
                })?;

            if !canonical.starts_with(&self.root) {
                return Err(FsError::PathEscapesRepo { path: canonical });
            }

            canonical
        };

        let recursive = args.recursive.unwrap_or(true);
        let include_dirs = args.include_dirs.unwrap_or(true);
        let max_results = args.max_results.unwrap_or(DEFAULT_MAX_SEARCH_RESULTS);
        let skip = args.skip.unwrap_or(0);
        let match_mode = args.match_mode.unwrap_or(FindMatchMode::Name);
        let case_sensitive = args.case_sensitive.unwrap_or(false);

        if !start_path.exists() {
            return Err(FsError::FindRootNotExist {
                path: start_path.clone(),
            });
        }
        if !start_path.is_dir() {
            return Err(FsError::FindRootNotDirectory {
                path: start_path.clone(),
            });
        }

        if is_absolute && Self::find_git_root(&start_path).is_none() {
            return Err(FsError::FindRootNotInGit {
                path: start_path.clone(),
            });
        }

        let include_globs: Option<GlobSet> = Self::build_globset(&args.include_globs)?;
        let exclude_globs: Option<GlobSet> = Self::build_globset(&args.exclude_globs)?;

        // How many matches we need to collect in total (for paging).
        let total_needed = skip.saturating_add(max_results);

        if total_needed == 0 {
            return Ok(FindFilesResult {
                matches: Vec::new(),
                has_more: false,
            });
        }

        // Shared collection of matches; accessed from multiple threads.
        let matches: Arc<Mutex<Vec<FindFileMatch>>> = Arc::new(Mutex::new(Vec::new()));

        // Global counters across all threads: how many matches have been
        // seen (for `skip`) and whether we hit the `total_needed` cap.
        let seen_matches = Arc::new(AtomicU32::new(0));
        let hit_limit = Arc::new(AtomicBool::new(false));

        let repo_root = self.root.clone();

        // Use a regex matcher for literal substring search, to avoid
        // allocating a lowercased string per entry in the hot loop.
        let query = args.query;
        let escaped = regex::escape(&query);
        let matcher = regex::RegexBuilder::new(&escaped)
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|source| FsError::InvalidFindFilesRegex {
                query: query.clone(),
                source,
            })?;
        let matcher = Arc::new(matcher);

        let mut builder = WalkBuilder::new(&start_path);
        builder.standard_filters(true);
        if !recursive {
            builder.max_depth(Some(1));
        }

        builder.build_parallel().run(|| {
            let matches = matches.clone();
            let seen_matches = seen_matches.clone();
            let hit_limit = hit_limit.clone();
            let include_globs = include_globs.clone();
            let exclude_globs = exclude_globs.clone();
            let start_path = start_path.clone();
            let repo_root = repo_root.clone();
            let matcher = matcher.clone();

            Box::new(move |entry_res| {
                if hit_limit.load(Ordering::Relaxed) {
                    return ignore::WalkState::Quit;
                }

                let entry = match entry_res {
                    Ok(e) => e,
                    Err(err) => {
                        eprintln!("find_files: skip entry error: {err}");
                        return ignore::WalkState::Continue;
                    }
                };

                let path = entry.path();
                if path == start_path {
                    return ignore::WalkState::Continue;
                }

                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir && !include_dirs {
                    return ignore::WalkState::Continue;
                }

                let rel = match path.strip_prefix(&start_path) {
                    Ok(r) => r,
                    Err(_) => path.strip_prefix(&repo_root).unwrap_or(path),
                };

                let rel_str = rel.to_string_lossy();

                if let Some(ref excludes) = exclude_globs
                    && excludes.is_match(rel_str.as_ref())
                {
                    return ignore::WalkState::Continue;
                }

                if let Some(ref includes) = include_globs
                    && !includes.is_match(rel_str.as_ref())
                {
                    return ignore::WalkState::Continue;
                }

                let haystack = match match_mode {
                    FindMatchMode::Name => match path.file_name().and_then(|n| n.to_str()) {
                        Some(name) => name,
                        None => return ignore::WalkState::Continue,
                    },
                    FindMatchMode::Path => rel_str.as_ref(),
                };

                if !matcher.is_match(haystack) {
                    return ignore::WalkState::Continue;
                }

                let seen_after = seen_matches.fetch_add(1, Ordering::Relaxed) + 1;
                if seen_after > total_needed {
                    hit_limit.store(true, Ordering::Relaxed);
                    return ignore::WalkState::Quit;
                }

                let rel_owned = rel_str.into_owned();

                let mut guard = matches.lock().expect("find_files: matches mutex poisoned");
                guard.push(FindFileMatch {
                    path: rel_owned,
                    is_dir,
                });

                ignore::WalkState::Continue
            })
        });

        let mut matches = {
            let mut guard = matches
                .lock()
                .expect("find_files: matches mutex poisoned at final collection");
            std::mem::take(&mut *guard)
        };

        // Parallel walking does not guarantee order; sort by path to make
        // results deterministic before applying skip/limit.
        matches.sort_by(|a, b| a.path.cmp(&b.path));

        let skip_usize = skip as usize;
        let max_usize = max_results as usize;
        let total = matches.len();

        let sliced = if skip_usize >= total {
            Vec::new()
        } else {
            matches
                .into_iter()
                .skip(skip_usize)
                .take(max_usize)
                .collect()
        };

        let has_more = hit_limit.load(Ordering::Relaxed)
            || (seen_matches.load(Ordering::Relaxed) > total_needed);

        Ok(FindFilesResult {
            matches: sliced,
            has_more,
        })
    }

    pub fn stat(&self, args: StatArgs) -> Result<StatResult> {
        let raw_path = args.path;
        let path = Path::new(&raw_path);
        let is_absolute = path.is_absolute();

        let resolved = if is_absolute {
            PathBuf::from(&raw_path)
        } else {
            self.root.join(path)
        };

        match std::fs::metadata(&resolved) {
            Ok(meta) => {
                // Only enforce repo-root containment for relative paths.
                let canonical =
                    resolved
                        .canonicalize()
                        .map_err(|source| FsError::CanonicalizePath {
                            path: resolved.clone(),
                            source,
                        })?;

                if !is_absolute && !canonical.starts_with(&self.root) {
                    return Err(FsError::PathEscapesRepo { path: canonical });
                }

                let size = if meta.is_file() {
                    Some(meta.len())
                } else {
                    None
                };
                let modified = meta.modified().ok().and_then(|time| {
                    time.duration_since(SystemTime::UNIX_EPOCH)
                        .ok()
                        .map(|dur| dur.as_secs())
                });

                let display_path = self
                    .strip_root(&canonical)
                    .unwrap_or_else(|| canonical.display().to_string());

                Ok(StatResult {
                    path: display_path,
                    exists: true,
                    is_file: meta.is_file(),
                    is_dir: meta.is_dir(),
                    size,
                    modified,
                })
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                // Path does not exist: return a structured, non-error result.
                let display_path = if is_absolute {
                    resolved.display().to_string()
                } else {
                    resolved
                        .strip_prefix(&self.root)
                        .unwrap_or(&resolved)
                        .to_string_lossy()
                        .into_owned()
                };

                Ok(StatResult {
                    path: display_path,
                    exists: false,
                    is_file: false,
                    is_dir: false,
                    size: None,
                    modified: None,
                })
            }
            Err(source) => Err(FsError::FileMetadata {
                path: resolved,
                source,
            }),
        }
    }

    pub fn path_info(&self, args: PathInfoArgs) -> Result<PathInfoResult> {
        let input = args.path.unwrap_or_else(|| ".".to_string());
        let input_path = if input.is_empty() {
            ".".to_string()
        } else {
            input
        };

        let p = Path::new(&input_path);
        let is_absolute = p.is_absolute();

        let resolved = if is_absolute {
            PathBuf::from(&input_path)
        } else {
            self.root.join(p)
        };

        let resolved_str = resolved.display().to_string();

        match std::fs::metadata(&resolved) {
            Ok(meta) => {
                let canonical =
                    resolved
                        .canonicalize()
                        .map_err(|source| FsError::CanonicalizePath {
                            path: resolved.clone(),
                            source,
                        })?;

                let canonical_str = canonical.display().to_string();
                let repo_root = Self::find_git_root(&canonical).map(|p| p.display().to_string());

                Ok(PathInfoResult {
                    input_path,
                    resolved_path: resolved_str,
                    exists: true,
                    is_file: meta.is_file(),
                    is_dir: meta.is_dir(),
                    is_absolute,
                    canonical_path: Some(canonical_str),
                    repo_root,
                })
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(PathInfoResult {
                input_path,
                resolved_path: resolved_str,
                exists: false,
                is_file: false,
                is_dir: false,
                is_absolute,
                canonical_path: None,
                repo_root: None,
            }),
            Err(source) => Err(FsError::FileMetadata {
                path: resolved,
                source,
            }),
        }
    }

    pub fn create_file(&self, args: CreateFileArgs) -> Result<CreateFileResult> {
        let overwrite = args.overwrite.unwrap_or(false);
        let create_parents = args.create_parents.unwrap_or(false);

        let raw_path = args.path;
        let path = Path::new(&raw_path);
        let is_absolute = path.is_absolute();

        let resolved = if is_absolute {
            PathBuf::from(&raw_path)
        } else {
            self.root.join(path)
        };

        // Create parent directories if requested.
        if create_parents
            && let Some(parent) = resolved.parent()
            && let Err(source) = std::fs::create_dir_all(parent)
        {
            return Err(FsError::CreateParents {
                path: parent.to_path_buf(),
                source,
            });
        }

        // Perform safety checks based on the parent directory.
        if let Some(parent) = resolved.parent() {
            let canonical_parent =
                parent
                    .canonicalize()
                    .map_err(|source| FsError::CanonicalizePath {
                        path: parent.to_path_buf(),
                        source,
                    })?;

            if !is_absolute {
                if !canonical_parent.starts_with(&self.root) {
                    return Err(FsError::PathEscapesRepo {
                        path: canonical_parent,
                    });
                }
            } else if Self::find_git_root(&canonical_parent).is_none() {
                return Err(FsError::WritePathNotInGit {
                    path: canonical_parent,
                });
            }
        }

        // Check existing target.
        let existed_meta = std::fs::metadata(&resolved).ok();
        if let Some(meta) = &existed_meta {
            if meta.is_dir() {
                return Err(FsError::DestinationExists {
                    path: resolved.clone(),
                });
            }
            if !overwrite {
                return Err(FsError::DestinationExists {
                    path: resolved.clone(),
                });
            }
        }

        // Open and write content.
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&resolved)
            .map_err(|source| FsError::OpenFile {
                path: resolved.clone(),
                source,
            })?;

        if let Some(content) = args.content {
            use std::io::Write;
            file.write_all(content.as_bytes())
                .map_err(|source| FsError::WriteFile {
                    path: resolved.clone(),
                    source,
                })?;
        }

        let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());

        let display_path = self
            .strip_root(&canonical)
            .unwrap_or_else(|| canonical.display().to_string());

        Ok(CreateFileResult {
            path: display_path,
            created: existed_meta.is_none(),
            overwritten: existed_meta.is_some(),
        })
    }

    pub fn delete_path(&self, args: DeletePathArgs) -> Result<DeletePathResult> {
        let recursive = args.recursive.unwrap_or(false);
        let force = args.force.unwrap_or(false);

        let raw_path = args.path;
        let path = Path::new(&raw_path);
        let is_absolute = path.is_absolute();

        let resolved = if is_absolute {
            PathBuf::from(&raw_path)
        } else {
            self.root.join(path)
        };

        match std::fs::metadata(&resolved) {
            Ok(meta) => {
                let canonical =
                    resolved
                        .canonicalize()
                        .map_err(|source| FsError::CanonicalizePath {
                            path: resolved.clone(),
                            source,
                        })?;

                if !is_absolute && !canonical.starts_with(&self.root) {
                    return Err(FsError::PathEscapesRepo { path: canonical });
                }
                if is_absolute && Self::find_git_root(&canonical).is_none() {
                    return Err(FsError::WritePathNotInGit { path: canonical });
                }

                let is_dir = meta.is_dir();
                if is_dir {
                    if !recursive {
                        return Err(FsError::DeleteDirNonRecursive { path: canonical });
                    }
                    std::fs::remove_dir_all(&canonical).map_err(|source| FsError::DeletePath {
                        path: canonical.clone(),
                        source,
                    })?;
                } else {
                    std::fs::remove_file(&canonical).map_err(|source| FsError::DeletePath {
                        path: canonical.clone(),
                        source,
                    })?;
                }

                let display_path = self
                    .strip_root(&canonical)
                    .unwrap_or_else(|| canonical.display().to_string());

                Ok(DeletePathResult {
                    path: display_path,
                    existed: true,
                    is_dir,
                    removed: true,
                    recursive: is_dir && recursive,
                })
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                if !force {
                    return Err(FsError::FileMetadata {
                        path: resolved.clone(),
                        source: err,
                    });
                }

                let display_path = if is_absolute {
                    resolved.display().to_string()
                } else {
                    resolved
                        .strip_prefix(&self.root)
                        .unwrap_or(&resolved)
                        .to_string_lossy()
                        .into_owned()
                };

                Ok(DeletePathResult {
                    path: display_path,
                    existed: false,
                    is_dir: false,
                    removed: false,
                    recursive: false,
                })
            }
            Err(source) => Err(FsError::FileMetadata {
                path: resolved,
                source,
            }),
        }
    }

    pub fn copy_path(&self, args: CopyPathArgs) -> Result<CopyPathResult> {
        let overwrite = args.overwrite.unwrap_or(false);
        let create_parents = args.create_parents.unwrap_or(true);

        let from_raw = args.from;
        let to_raw = args.to;

        let from_path = Path::new(&from_raw);
        let to_path = Path::new(&to_raw);

        let from_abs = from_path.is_absolute();
        let to_abs = to_path.is_absolute();

        let from_resolved = if from_abs {
            PathBuf::from(&from_raw)
        } else {
            self.root.join(from_path)
        };
        let to_resolved = if to_abs {
            PathBuf::from(&to_raw)
        } else {
            self.root.join(to_path)
        };

        let from_meta =
            std::fs::metadata(&from_resolved).map_err(|source| FsError::FileMetadata {
                path: from_resolved.clone(),
                source,
            })?;

        if !from_meta.is_file() {
            return Err(FsError::CopyPath {
                from: from_resolved,
                to: to_resolved,
                source: io::Error::other("copy_path currently only supports regular files"),
            });
        }

        // Security: both ends must be inside some git repo, and in the same repo.
        let from_canonical =
            from_resolved
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: from_resolved.clone(),
                    source,
                })?;

        let from_repo_root =
            Self::find_git_root(&from_canonical).ok_or_else(|| FsError::WritePathNotInGit {
                path: from_canonical.clone(),
            })?;

        // Ensure destination parent exists if requested.
        if create_parents
            && let Some(parent) = to_resolved.parent()
            && let Err(source) = std::fs::create_dir_all(parent)
        {
            return Err(FsError::CreateParents {
                path: parent.to_path_buf(),
                source,
            });
        }

        let to_parent = to_resolved
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let to_parent_canonical =
            to_parent
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: to_parent.clone(),
                    source,
                })?;

        let to_repo_root = Self::find_git_root(&to_parent_canonical).ok_or_else(|| {
            FsError::WritePathNotInGit {
                path: to_parent_canonical.clone(),
            }
        })?;

        if from_repo_root != to_repo_root {
            return Err(FsError::CopyAcrossRepos {
                from: from_canonical,
                to: to_resolved.clone(),
            });
        }

        // Overwrite handling.
        let existing_to = std::fs::metadata(&to_resolved).ok();
        if existing_to.is_some() && !overwrite {
            return Err(FsError::DestinationExists {
                path: to_resolved.clone(),
            });
        }

        let bytes_copied =
            std::fs::copy(&from_resolved, &to_resolved).map_err(|source| FsError::CopyPath {
                from: from_resolved.clone(),
                to: to_resolved.clone(),
                source,
            })?;

        let from_display = self
            .strip_root(&from_canonical)
            .unwrap_or_else(|| from_canonical.display().to_string());
        let to_display = self
            .strip_root(&to_resolved)
            .unwrap_or_else(|| to_resolved.display().to_string());

        Ok(CopyPathResult {
            from: from_display,
            to: to_display,
            bytes_copied: Some(bytes_copied),
            overwritten: existing_to.is_some(),
        })
    }

    pub fn move_path(&self, args: MovePathArgs) -> Result<MovePathResult> {
        let overwrite = args.overwrite.unwrap_or(false);
        let create_parents = args.create_parents.unwrap_or(true);

        let from_raw = args.from;
        let to_raw = args.to;

        let from_path = Path::new(&from_raw);
        let to_path = Path::new(&to_raw);

        let from_abs = from_path.is_absolute();
        let to_abs = to_path.is_absolute();

        let from_resolved = if from_abs {
            PathBuf::from(&from_raw)
        } else {
            self.root.join(from_path)
        };
        let to_resolved = if to_abs {
            PathBuf::from(&to_raw)
        } else {
            self.root.join(to_path)
        };

        let from_meta = match std::fs::metadata(&from_resolved) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                let from_display = from_resolved.display().to_string();
                return Ok(MovePathResult {
                    from: from_display,
                    to: to_resolved.display().to_string(),
                    existed: false,
                    overwritten: false,
                    recursive: false,
                });
            }
            Err(source) => {
                return Err(FsError::FileMetadata {
                    path: from_resolved.clone(),
                    source,
                });
            }
        };

        if !from_meta.is_file() {
            return Err(FsError::MovePath {
                from: from_resolved,
                to: to_resolved,
                source: io::Error::other("move_path currently only supports regular files"),
            });
        }

        // Security: both ends must be inside some git repo, and in the same repo.
        let from_canonical =
            from_resolved
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: from_resolved.clone(),
                    source,
                })?;

        let from_repo_root =
            Self::find_git_root(&from_canonical).ok_or_else(|| FsError::WritePathNotInGit {
                path: from_canonical.clone(),
            })?;

        if create_parents
            && let Some(parent) = to_resolved.parent()
            && let Err(source) = std::fs::create_dir_all(parent)
        {
            return Err(FsError::CreateParents {
                path: parent.to_path_buf(),
                source,
            });
        }

        let to_parent = to_resolved
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let to_parent_canonical =
            to_parent
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: to_parent.clone(),
                    source,
                })?;

        let to_repo_root = Self::find_git_root(&to_parent_canonical).ok_or_else(|| {
            FsError::WritePathNotInGit {
                path: to_parent_canonical.clone(),
            }
        })?;

        if from_repo_root != to_repo_root {
            return Err(FsError::MoveAcrossRepos {
                from: from_canonical,
                to: to_resolved.clone(),
            });
        }

        let existing_to = std::fs::metadata(&to_resolved).ok();
        if existing_to.is_some() && !overwrite {
            return Err(FsError::DestinationExists {
                path: to_resolved.clone(),
            });
        }

        std::fs::rename(&from_resolved, &to_resolved).map_err(|source| FsError::MovePath {
            from: from_resolved.clone(),
            to: to_resolved.clone(),
            source,
        })?;

        let from_display = self
            .strip_root(&from_canonical)
            .unwrap_or_else(|| from_canonical.display().to_string());
        let to_display = self
            .strip_root(&to_resolved)
            .unwrap_or_else(|| to_resolved.display().to_string());

        Ok(MovePathResult {
            from: from_display,
            to: to_display,
            existed: true,
            overwritten: existing_to.is_some(),
            recursive: false,
        })
    }

    pub fn overwrite_file(&self, args: OverwriteFileArgs) -> Result<OverwriteFileResult> {
        let raw_path = args.path;
        let path = Path::new(&raw_path);
        let is_absolute = path.is_absolute();

        let resolved = if is_absolute {
            PathBuf::from(&raw_path)
        } else {
            self.root.join(path)
        };

        let meta = std::fs::metadata(&resolved).map_err(|source| FsError::FileMetadata {
            path: resolved.clone(),
            source,
        })?;

        if !meta.is_file() {
            return Err(FsError::WriteFile {
                path: resolved,
                source: io::Error::other("overwrite_file only supports regular files"),
            });
        }

        // Safety: similar to create_file, but require an existing file.
        if let Some(parent) = resolved.parent() {
            let canonical_parent =
                parent
                    .canonicalize()
                    .map_err(|source| FsError::CanonicalizePath {
                        path: parent.to_path_buf(),
                        source,
                    })?;

            if !is_absolute && !canonical_parent.starts_with(&self.root) {
                return Err(FsError::PathEscapesRepo {
                    path: canonical_parent,
                });
            }

            if is_absolute && Self::find_git_root(&canonical_parent).is_none() {
                return Err(FsError::WritePathNotInGit {
                    path: canonical_parent,
                });
            }
        }

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&resolved)
            .map_err(|source| FsError::OpenFile {
                path: resolved.clone(),
                source,
            })?;

        use std::io::Write;
        file.write_all(args.content.as_bytes())
            .map_err(|source| FsError::WriteFile {
                path: resolved.clone(),
                source,
            })?;

        let canonical = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());

        let display_path = self
            .strip_root(&canonical)
            .unwrap_or_else(|| canonical.display().to_string());

        Ok(OverwriteFileResult { path: display_path })
    }

    pub fn read_file(&self, args: ReadFileArgs) -> Result<FileChunkResult> {
        let abs_path = self.resolve_path(&args.path)?;
        let has_byte_params = args.offset_bytes.is_some() || args.max_bytes.is_some();
        let has_line_params = args.start_line.is_some() || args.max_lines.is_some();

        let range_type = match args.range_type {
            Some(rt) => rt,
            None if has_line_params => RangeType::Lines,
            None => RangeType::Bytes,
        };

        match range_type {
            RangeType::Bytes if has_line_params => {
                return Err(FsError::ReadFileLinesWithBytes);
            }
            RangeType::Lines if has_byte_params => {
                return Err(FsError::ReadFileBytesWithLines);
            }
            _ => {}
        }

        match range_type {
            RangeType::Bytes => self.read_file_bytes(&abs_path, &args),
            RangeType::Lines => self.read_file_lines(&abs_path, &args),
        }
    }

    fn read_file_bytes(&self, abs_path: &Path, args: &ReadFileArgs) -> Result<FileChunkResult> {
        let offset = args.offset_bytes.unwrap_or(0);
        let max_bytes = args.max_bytes.unwrap_or(DEFAULT_MAX_READ_BYTES);

        let mut file = File::open(abs_path).map_err(|source| FsError::OpenFile {
            path: abs_path.to_path_buf(),
            source,
        })?;

        if offset > 0 {
            file.seek(SeekFrom::Start(offset))
                .map_err(|source| FsError::SeekFile {
                    path: abs_path.to_path_buf(),
                    source,
                })?;
        }

        let mut buf = Vec::new();
        let mut limited = file.take(max_bytes);
        limited
            .read_to_end(&mut buf)
            .map_err(|source| FsError::ReadFile {
                path: abs_path.to_path_buf(),
                source,
            })?;

        let content = String::from_utf8(buf.clone()).map_err(|_| FsError::FileNotUtf8 {
            path: abs_path.to_path_buf(),
        })?;

        let metadata = std::fs::metadata(abs_path).map_err(|source| FsError::FileMetadata {
            path: abs_path.to_path_buf(),
            source,
        })?;
        let file_len = metadata.len();
        let is_truncated = offset + max_bytes < file_len;

        Ok(FileChunkResult {
            path: self
                .strip_root(abs_path)
                .unwrap_or_else(|| abs_path.display().to_string()),
            content,
            is_truncated,
            range: FileRangeInfo {
                range_type: RangeType::Bytes,
                offset_bytes: Some(offset),
                max_bytes: Some(max_bytes),
                start_line: None,
                max_lines: None,
            },
        })
    }

    fn read_file_lines(&self, abs_path: &Path, args: &ReadFileArgs) -> Result<FileChunkResult> {
        let start_line = args.start_line.unwrap_or(1);
        let max_lines = args.max_lines.unwrap_or(DEFAULT_MAX_READ_LINES);

        if start_line == 0 {
            return Err(FsError::StartLineMustBePositive);
        }

        let file = File::open(abs_path).map_err(|source| FsError::OpenFile {
            path: abs_path.to_path_buf(),
            source,
        })?;
        let reader = BufReader::new(file);

        let mut content = String::new();
        let mut current_line: u64 = 0;
        let mut collected: u64 = 0;
        let mut is_truncated = false;

        for line_res in reader.lines() {
            let line = line_res.map_err(|source| FsError::ReadLine {
                path: abs_path.to_path_buf(),
                source,
            })?;
            current_line += 1;

            if current_line < start_line {
                continue;
            }

            content.push_str(&line);
            content.push('\n');
            collected += 1;

            if collected >= max_lines {
                is_truncated = true;
                break;
            }
        }

        Ok(FileChunkResult {
            path: self
                .strip_root(abs_path)
                .unwrap_or_else(|| abs_path.display().to_string()),
            content,
            is_truncated,
            range: FileRangeInfo {
                range_type: RangeType::Lines,
                offset_bytes: None,
                max_bytes: None,
                start_line: Some(start_line),
                max_lines: Some(max_lines),
            },
        })
    }

    pub fn search_text(&self, args: SearchTextArgs) -> Result<SearchTextResult> {
        let mode = args.mode.unwrap_or(SearchMode::Literal);
        let case_sensitive = args.case_sensitive.unwrap_or(false);
        let max_results = args.max_results.unwrap_or(DEFAULT_MAX_SEARCH_RESULTS);
        let context_lines = args.context_lines.unwrap_or(DEFAULT_SEARCH_CONTEXT_LINES);
        let skip = args.skip.unwrap_or(0);

        let root_arg = args.root.as_deref().unwrap_or(".");
        let root_path = Path::new(root_arg);
        let is_absolute = root_path.is_absolute();

        let start_path = if is_absolute {
            let canonical =
                root_path
                    .canonicalize()
                    .map_err(|source| FsError::CanonicalizePath {
                        path: root_path.to_path_buf(),
                        source,
                    })?;

            if Self::find_git_root(&canonical).is_none() {
                return Err(FsError::SearchRootNotInGit { path: canonical });
            }

            canonical
        } else {
            let joined = self.root.join(root_path);
            let canonical = joined
                .canonicalize()
                .map_err(|source| FsError::CanonicalizePath {
                    path: joined.clone(),
                    source,
                })?;

            if !canonical.starts_with(&self.root) {
                return Err(FsError::SearchRootEscapesRepo { path: canonical });
            }

            canonical
        };

        let include_globs = Self::build_globset(&args.include_globs)?.map(Arc::new);
        let exclude_globs = Self::build_globset(&args.exclude_globs)?.map(Arc::new);

        // Build a bytes-based regex matcher. Literal mode is implemented
        // by escaping the query string.
        let pattern_str = match mode {
            SearchMode::Literal => regex::escape(&args.query),
            SearchMode::Regex => args.query.clone(),
        };

        let matcher = ByteRegexBuilder::new(&pattern_str)
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|source| FsError::InvalidSearchRegex {
                query: args.query.clone(),
                source,
            })?;

        let matcher = Arc::new(matcher);
        let hits: Arc<Mutex<Vec<SearchHit>>> = Arc::new(Mutex::new(Vec::new()));

        // Global counters across all threads: how many matches have been
        // seen (for `skip`) and whether we hit the `max_results` cap.
        let seen_matches = Arc::new(AtomicU32::new(0));
        let hit_limit = Arc::new(AtomicBool::new(false));

        let repo_root = self.root.clone();
        let mut builder = WalkBuilder::new(&start_path);
        builder.standard_filters(true);

        builder.build_parallel().run(|| {
            let matcher = matcher.clone();
            let hits = hits.clone();
            let seen_matches = seen_matches.clone();
            let hit_limit = hit_limit.clone();
            let include_globs = include_globs.clone();
            let exclude_globs = exclude_globs.clone();
            let start_path = start_path.clone();
            let repo_root = repo_root.clone();

            Box::new(move |entry_res| {
                if hit_limit.load(Ordering::Relaxed) {
                    return ignore::WalkState::Quit;
                }

                let entry = match entry_res {
                    Ok(e) => e,
                    Err(err) => {
                        eprintln!("search_text: skip entry error: {err}");
                        return ignore::WalkState::Continue;
                    }
                };

                let path = entry.path();
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    return ignore::WalkState::Continue;
                }

                let rel = match path.strip_prefix(&start_path) {
                    Ok(r) => r,
                    Err(_) => path.strip_prefix(&repo_root).unwrap_or(path),
                };

                let rel_str = rel.to_string_lossy();

                if let Some(ref excludes) = exclude_globs
                    && excludes.is_match(rel_str.as_ref())
                {
                    return ignore::WalkState::Continue;
                }

                if let Some(ref includes) = include_globs
                    && !includes.is_match(rel_str.as_ref())
                {
                    return ignore::WalkState::Continue;
                }

                let file = match File::open(path) {
                    Ok(f) => f,
                    Err(err) => {
                        eprintln!(
                            "search_text: skip file open error {}: {err}",
                            path.display()
                        );
                        return ignore::WalkState::Continue;
                    }
                };

                let mmap = match unsafe { Mmap::map(&file) } {
                    Ok(m) => m,
                    Err(err) => {
                        eprintln!("search_text: skip mmap error {}: {err}", path.display());
                        return ignore::WalkState::Continue;
                    }
                };

                if mmap.is_empty() {
                    return ignore::WalkState::Continue;
                }

                // Precompute line start offsets (0-based byte indices).
                let mut line_starts: Vec<usize> = Vec::new();
                line_starts.push(0);
                for (i, &b) in mmap.iter().enumerate() {
                    if b == b'\n' && i + 1 < mmap.len() {
                        line_starts.push(i + 1);
                    }
                }

                let line_count = line_starts.len();

                for (idx, &line_start) in line_starts.iter().enumerate() {
                    if hit_limit.load(Ordering::Relaxed) {
                        return ignore::WalkState::Quit;
                    }

                    let line_end = if idx + 1 < line_count {
                        line_starts[idx + 1].saturating_sub(1)
                    } else {
                        mmap.len()
                    };

                    if line_start >= line_end || line_end > mmap.len() {
                        continue;
                    }

                    let line_slice = &mmap[line_start..line_end];

                    // Find the first match in this line (keep behavior
                    // of at most one hit per line).
                    let mat = match matcher.find(line_slice) {
                        Some(m) => m,
                        None => continue,
                    };

                    let col_idx = mat.start();

                    let seen_before = seen_matches.fetch_add(1, Ordering::Relaxed);
                    let seen_after = seen_before + 1;

                    if seen_after <= skip {
                        continue;
                    }

                    // Check and push into the shared hits vector.
                    let mut guard = hits.lock().expect("search_text: hits mutex poisoned");
                    if guard.len() as u32 >= max_results {
                        hit_limit.store(true, Ordering::Relaxed);
                        return ignore::WalkState::Quit;
                    }

                    let line_num = idx as u64 + 1;
                    let col = col_idx as u64;

                    let start_ctx = idx.saturating_sub(context_lines as usize);
                    let end_ctx = usize::min(line_count, idx + 1 + context_lines as usize);

                    let mut context_before = Vec::new();
                    let mut context_after = Vec::new();

                    for ctx_idx in start_ctx..end_ctx {
                        if ctx_idx == idx {
                            continue;
                        }

                        let ctx_start = line_starts[ctx_idx];
                        let ctx_end = if ctx_idx + 1 < line_count {
                            line_starts[ctx_idx + 1].saturating_sub(1)
                        } else {
                            mmap.len()
                        };

                        if ctx_start >= ctx_end || ctx_end > mmap.len() {
                            continue;
                        }

                        let ctx_slice = &mmap[ctx_start..ctx_end];
                        let ctx_text = String::from_utf8_lossy(ctx_slice).to_string();

                        if ctx_idx < idx {
                            context_before.push(ctx_text);
                        } else {
                            context_after.push(ctx_text);
                        }
                    }

                    let rel = match path.strip_prefix(&repo_root) {
                        Ok(r) => r.to_string_lossy().into_owned(),
                        Err(_) => path.display().to_string(),
                    };

                    let line_text = String::from_utf8_lossy(line_slice).to_string();

                    guard.push(SearchHit {
                        path: rel,
                        line: line_num,
                        column: col,
                        line_text,
                        context_before,
                        context_after,
                    });

                    if guard.len() as u32 >= max_results {
                        hit_limit.store(true, Ordering::Relaxed);
                        return ignore::WalkState::Quit;
                    }
                }

                ignore::WalkState::Continue
            })
        });

        let hits = {
            let mut guard = hits
                .lock()
                .expect("search_text: hits mutex poisoned at final collection");
            std::mem::take(&mut *guard)
        };

        Ok(SearchTextResult {
            hits,
            has_more: hit_limit.load(Ordering::Relaxed),
        })
    }

    fn strip_root(&self, path: &Path) -> Option<String> {
        path.strip_prefix(&self.root)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    }
}

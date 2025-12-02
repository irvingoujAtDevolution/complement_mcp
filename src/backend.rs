use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use regex::RegexBuilder;

use crate::types::{
    FileChunkResult, FileEntry, FileRangeInfo, ListFilesArgs, ListFilesResult, RangeType,
    ReadFileArgs, SearchHit, SearchMode, SearchTextArgs, SearchTextResult,
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
            .context("failed to canonicalize root directory")?;

        if !root.is_dir() {
            return Err(anyhow!(
                "root directory is not a directory: {}",
                root.display()
            ));
        }

        Ok(Self { root })
    }

    fn resolve_path(&self, rel: &str) -> Result<PathBuf> {
        let joined = self.root.join(rel);
        let canonical = joined
            .canonicalize()
            .with_context(|| format!("failed to canonicalize path: {}", joined.display()))?;

        let root = &self.root;
        if !canonical.starts_with(root) {
            return Err(anyhow!(
                "path escapes repository root: {}",
                canonical.display()
            ));
        }

        Ok(canonical)
    }

    fn build_globset(patterns: &Option<Vec<String>>) -> Result<Option<GlobSet>> {
        let mut builder = GlobSetBuilder::new();
        let mut any = false;

        if let Some(pats) = patterns {
            for pat in pats {
                let glob =
                    Glob::new(pat).with_context(|| format!("invalid glob pattern: {pat}"))?;
                builder.add(glob);
                any = true;
            }
        }

        if any {
            Ok(Some(builder.build()?))
        } else {
            Ok(None)
        }
    }

    pub fn list_files(&self, args: ListFilesArgs) -> Result<ListFilesResult> {
        let root_rel = args.root.as_deref().unwrap_or(".");
        let start_path = self.root.join(root_rel);
        let recursive = args.recursive.unwrap_or(true);
        let include_dirs = args.include_dirs.unwrap_or(false);
        let max_results = args.max_results.unwrap_or(DEFAULT_LIST_MAX_RESULTS);
        let skip = args.skip.unwrap_or(0);
        let include_metadata = args.include_metadata.unwrap_or(false);

        let include_globs = Self::build_globset(&args.include_globs)?;
        let exclude_globs = Self::build_globset(&args.exclude_globs)?;

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

            let rel = match path.strip_prefix(&self.root) {
                Ok(r) => r,
                Err(_) => continue,
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
                return Err(anyhow!(
                    "invalid read_file arguments: start_line/max_lines cannot be used with bytes range_type"
                ));
            }
            RangeType::Lines if has_byte_params => {
                return Err(anyhow!(
                    "invalid read_file arguments: offset_bytes/max_bytes cannot be used with lines range_type"
                ));
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

        let mut file = File::open(abs_path)
            .with_context(|| format!("failed to open file: {}", abs_path.display()))?;

        if offset > 0 {
            file.seek(SeekFrom::Start(offset))
                .with_context(|| format!("failed to seek in file: {}", abs_path.display()))?;
        }

        let mut buf = Vec::new();
        let mut limited = file.take(max_bytes);
        limited
            .read_to_end(&mut buf)
            .with_context(|| format!("failed to read file: {}", abs_path.display()))?;

        let content = String::from_utf8(buf.clone()).map_err(|_| {
            anyhow!(
                "file is not valid UTF-8, binary files are not supported: {}",
                abs_path.display()
            )
        })?;

        let metadata = std::fs::metadata(abs_path)
            .with_context(|| format!("failed to get metadata: {}", abs_path.display()))?;
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
            return Err(anyhow!("start_line must be >= 1"));
        }

        let file = File::open(abs_path)
            .with_context(|| format!("failed to open file: {}", abs_path.display()))?;
        let reader = BufReader::new(file);

        let mut content = String::new();
        let mut current_line: u64 = 0;
        let mut collected: u64 = 0;
        let mut is_truncated = false;

        for line_res in reader.lines() {
            let line = line_res
                .with_context(|| format!("failed to read line from {}", abs_path.display()))?;
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

        let root_rel = args.root.as_deref().unwrap_or(".");
        let start_path = self.root.join(root_rel);

        let include_globs = Self::build_globset(&args.include_globs)?;
        let exclude_globs = Self::build_globset(&args.exclude_globs)?;

        let mut builder = WalkBuilder::new(&start_path);
        builder.standard_filters(true);

        let mut hits = Vec::new();
        let mut matched: u32 = 0;

        let pattern = match mode {
            SearchMode::Literal => None,
            SearchMode::Regex => {
                let mut builder = RegexBuilder::new(&args.query);
                builder.case_insensitive(!case_sensitive);
                let pat = builder
                    .build()
                    .with_context(|| format!("invalid regex: {}", args.query))?;
                Some(pat)
            }
        };

        for result in builder.build() {
            let entry = match result {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("search_text: skip entry error: {err}");
                    continue;
                }
            };

            let path = entry.path();
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }

            let rel = match path.strip_prefix(&self.root) {
                Ok(r) => r,
                Err(_) => continue,
            };

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

            let file = match File::open(path) {
                Ok(f) => f,
                Err(err) => {
                    eprintln!(
                        "search_text: skip file open error {}: {err}",
                        path.display()
                    );
                    continue;
                }
            };
            let reader = BufReader::new(file);

            let mut lines: Vec<String> = Vec::new();
            for line_res in reader.lines() {
                match line_res {
                    Ok(line) => lines.push(line),
                    Err(err) => {
                        eprintln!(
                            "search_text: skip file read error {}: {err}",
                            path.display()
                        );
                        lines.clear();
                        break;
                    }
                }
            }
            if lines.is_empty() {
                continue;
            }

            for (idx, line) in lines.iter().enumerate() {
                let match_start: Option<usize> = match mode {
                    SearchMode::Regex => {
                        let re = pattern
                            .as_ref()
                            .expect("regex mode requires compiled pattern");
                        re.find(line).map(|m| m.start())
                    }
                    SearchMode::Literal => {
                        if case_sensitive {
                            line.find(&args.query)
                        } else {
                            let line_lower = line.to_lowercase();
                            let query_lower = args.query.to_lowercase();
                            line_lower.find(&query_lower)
                        }
                    }
                };

                let Some(col_idx) = match_start else {
                    continue;
                };

                matched = matched.saturating_add(1);
                if matched <= skip {
                    continue;
                }

                let line_num = idx as u64 + 1;
                let col = col_idx as u64;
                let start = idx.saturating_sub(context_lines as usize);
                let end = usize::min(lines.len(), idx + 1 + context_lines as usize);

                let mut context_before = Vec::new();
                let mut context_after = Vec::new();

                for (i, ctx_line) in lines[start..end].iter().enumerate() {
                    let real_idx = start + i;
                    if real_idx < idx {
                        context_before.push(ctx_line.clone());
                    } else if real_idx > idx {
                        context_after.push(ctx_line.clone());
                    }
                }

                let rel = match path.strip_prefix(&self.root) {
                    Ok(r) => r.to_string_lossy().into_owned(),
                    Err(_) => path.display().to_string(),
                };

                hits.push(SearchHit {
                    path: rel,
                    line: line_num,
                    column: col,
                    line_text: line.clone(),
                    context_before,
                    context_after,
                });

                if hits.len() as u32 >= max_results {
                    return Ok(SearchTextResult {
                        hits,
                        has_more: true,
                    });
                }
            }
        }

        Ok(SearchTextResult {
            hits,
            has_more: false,
        })
    }

    fn strip_root(&self, path: &Path) -> Option<String> {
        path.strip_prefix(&self.root)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    }
}

mod backend;
mod error;
mod mcp_service;
mod types;

use std::{env, error::Error, path::PathBuf};

use backend::LocalGitAwareFs;
use mcp_service::FileServer;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = env::args().nth(1).unwrap_or_else(|| ".".into());
    let root = PathBuf::from(repo_root);

    let backend = LocalGitAwareFs::new(root).map_err(|e| {
        eprintln!("complement_mcp: failed to initialize file backend: {e}");
        e
    })?;

    eprintln!("complement_mcp: git-aware file server starting on stdio");

    // Use the same pattern as rmcp examples (Counter/memory stdio servers)
    let service = FileServer::new(backend)
        .serve(stdio())
        .await
        .inspect_err(|e| eprintln!("complement_mcp: serving error: {e:?}"))?;

    service.waiting().await?;

    Ok(())
}

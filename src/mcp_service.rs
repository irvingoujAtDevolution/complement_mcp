use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    tool, tool_handler, tool_router,
};

use crate::backend::LocalGitAwareFs;
use crate::types::{ListFilesArgs, ReadFileArgs, SearchTextArgs};

#[derive(Clone)]
pub struct FileServer {
    backend: Arc<LocalGitAwareFs>,
    tool_router: ToolRouter<FileServer>,
}

impl FileServer {
    fn internal_error(code: &str, message: impl Into<String>) -> McpError {
        let full = format!("{code}: {}", message.into());
        McpError::internal_error(full, None)
    }
}

#[tool_router]
impl FileServer {
    pub fn new(backend: LocalGitAwareFs) -> Self {
        Self {
            backend: Arc::new(backend),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Search text in repository (gitignore aware)")]
    pub async fn search_text(
        &self,
        Parameters(args): Parameters<SearchTextArgs>,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .backend
            .search_text(args)
            .map_err(|e| Self::internal_error("search_text_failed", e.to_string()))?;

        let json = serde_json::to_string(&result)
            .map_err(|e| Self::internal_error("serialize_failed", e.to_string()))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Read a file or a range from it")]
    pub async fn read_file(
        &self,
        Parameters(args): Parameters<ReadFileArgs>,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .backend
            .read_file(args)
            .map_err(|e| Self::internal_error("read_file_failed", e.to_string()))?;

        let json = serde_json::to_string(&result)
            .map_err(|e| Self::internal_error("serialize_failed", e.to_string()))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List files in repository (gitignore aware)")]
    pub async fn list_files(
        &self,
        Parameters(args): Parameters<ListFilesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .backend
            .list_files(args)
            .map_err(|e| Self::internal_error("list_files_failed", e.to_string()))?;

        let json = serde_json::to_string(&result)
            .map_err(|e| Self::internal_error("serialize_failed", e.to_string()))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for FileServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Fast git-aware file server with tools: search_text, read_file, list_files"
                    .to_string(),
            ),
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
        }
    }
}

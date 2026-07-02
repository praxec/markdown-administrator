use anyhow::Result;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::transport::stdio;
use rmcp::RoleServer;
use rmcp::{ServerHandler, ServiceExt};
use std::sync::Arc;

type McpError = rmcp::ErrorData;

use markdown_administrator::{outline, read_section};

// ── server struct ─────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MarkdownServer;

impl ServerHandler for MarkdownServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = InitializeResult::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }

    async fn list_tools(
        &self,
        _params: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let outline_schema = Arc::new(
            serde_json::json!({
                "type": "object",
                "properties": {
                    "doc": {
                        "type": "string",
                        "description": "Path to the markdown file."
                    }
                },
                "required": ["doc"]
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        let read_section_schema = Arc::new(
            serde_json::json!({
                "type": "object",
                "properties": {
                    "doc": {
                        "type": "string",
                        "description": "Path to the markdown file."
                    },
                    "heading_path": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Heading path components."
                    }
                },
                "required": ["doc", "heading_path"]
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        Ok(ListToolsResult {
            tools: vec![
                Tool::new(
                    "outline",
                    "Return the outline (headings with paths) of a markdown file.",
                    outline_schema,
                ),
                Tool::new(
                    "read_section",
                    "Read the content of a section identified by its heading path.",
                    read_section_schema,
                ),
            ],
            next_cursor: None,
            meta: Default::default(),
        })
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let args = req.arguments.unwrap_or_default();

        match req.name.as_ref() {
            "outline" => {
                let doc = args
                    .get("doc")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("missing 'doc' argument", None))?;
                let src = std::fs::read_to_string(doc).map_err(|e| {
                    McpError::invalid_params(format!("cannot read {doc}: {e}"), None)
                })?;
                let headings = outline(&src);
                let value: Vec<serde_json::Value> = headings
                    .iter()
                    .map(|h| {
                        serde_json::json!({
                            "level": h.level,
                            "text": h.text,
                            "heading_path": h.heading_path,
                        })
                    })
                    .collect();
                let text = serde_json::to_string_pretty(&value)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            "read_section" => {
                let doc = args
                    .get("doc")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("missing 'doc' argument", None))?;
                let heading_path: Vec<String> = args
                    .get("heading_path")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        McpError::invalid_params("missing 'heading_path' argument", None)
                    })?
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                let src = std::fs::read_to_string(doc).map_err(|e| {
                    McpError::invalid_params(format!("cannot read {doc}: {e}"), None)
                })?;
                match read_section(&src, &heading_path) {
                    Ok(content) => Ok(CallToolResult::success(vec![Content::text(content)])),
                    Err(e) => Ok(CallToolResult::success(vec![Content::text(format!(
                        "Error: {e}"
                    ))])),
                }
            }
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

// ── entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let service = MarkdownServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_can_be_constructed() {
        let _s = MarkdownServer;
    }

    #[tokio::test]
    async fn server_info_has_tools_capability() {
        let s = MarkdownServer;
        let info = s.get_info();
        assert!(info.capabilities.tools.is_some());
    }

    #[tokio::test]
    async fn server_is_clone() {
        let s = MarkdownServer;
        let _cloned = s.clone();
    }
}

use crate::error::{Result, SlyError};
use crate::mcp::local::LocalMcp;
use async_trait::async_trait;
use serde_json::Value;

pub struct FetchMcp;

#[async_trait]
impl LocalMcp for FetchMcp {
    fn name(&self) -> &str {
        "fetch"
    }

    fn tool_definitions(&self) -> String {
        r#"
<tool_def>
    <name>fetch_url</name>
    <description>Fetches the content of a URL using HTTP GET.</description>
    <parameters>
        <parameter>
            <name>url</name>
            <type>string</type>
            <description>The URL to fetch</description>
            <required>true</required>
        </parameter>
    </parameters>
</tool_def>
"#.trim().to_string()
    }

    async fn execute(&self, tool_name: &str, args: &Value) -> Result<Value> {
        match tool_name {
            "fetch_url" => self.fetch_url(args).await.map(Value::String),
            _ => Err(SlyError::Task(format!("Unknown fetch tool: {}", tool_name))),
        }
    }
}

impl FetchMcp {
    async fn fetch_url(&self, args: &Value) -> Result<String> {
        let url = args["url"].as_str().ok_or_else(|| SlyError::Task("Missing 'url' argument".to_string()))?;
        println!("   📥 Fetching: {}", url);

        let client = reqwest::Client::new();
        let resp = client.get(url).send().await.map_err(|e| SlyError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        let text = resp.text().await.map_err(|e| SlyError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        Ok(text)
    }
}

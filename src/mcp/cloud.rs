use crate::error::{Result, SlyError};
use crate::mcp::local::LocalMcp;
use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

pub struct CloudMcp;

#[async_trait]
impl LocalMcp for CloudMcp {
    fn name(&self) -> &str {
        "cloud"
    }

    fn tool_definitions(&self) -> String {
        r#"
<tool_def>
    <name>cloud_deploy_pages</name>
    <description>Deploys a static site to Cloudflare Pages using Wrangler.</description>
    <parameters>
        <parameter>
            <name>project_dir</name>
            <type>string</type>
            <description>Directory containing static assets</description>
            <required>true</required>
        </parameter>
        <parameter>
            <name>project_name</name>
            <type>string</type>
            <description>Cloudflare project name</description>
            <required>false</required>
        </parameter>
    </parameters>
</tool_def>
<tool_def>
    <name>cloud_s3_sync</name>
    <description>Syncs a local directory to an AWS S3 bucket.</description>
    <parameters>
        <parameter>
            <name>local_dir</name>
            <type>string</type>
            <description>Local path to sync</description>
            <required>true</required>
        </parameter>
        <parameter>
            <name>bucket</name>
            <type>string</type>
            <description>Target S3 bucket name</description>
            <required>true</required>
        </parameter>
    </parameters>
</tool_def>
"#.trim().to_string()
    }

    async fn execute(&self, tool_name: &str, args: &Value) -> Result<Value> {
        match tool_name {
            "cloud_deploy_pages" => self.deploy_pages(args).await.map(Value::String),
            "cloud_s3_sync" => self.s3_sync(args).await.map(Value::String),
            _ => Err(SlyError::Task(format!("Unknown cloud tool: {}", tool_name))),
        }
    }
}

impl CloudMcp {
    async fn deploy_pages(&self, args: &Value) -> Result<String> {
        let project_dir = args["project_dir"].as_str().unwrap_or(".");
        let project_name = args["project_name"].as_str().unwrap_or("sly-deployment");

        println!("   ☁️ Cloudflare Pages Deploy: {} ({})", project_name, project_dir);

        let output = Command::new("wrangler")
            .arg("pages")
            .arg("deploy")
            .arg(project_dir)
            .arg("--project-name")
            .arg(project_name)
            .output()
            .await
            .map_err(|e| SlyError::Task(format!("Failed to execute wrangler: {}", e)))?;

        if output.status.success() {
             Ok(format!("Deployment Success!\n{}", String::from_utf8_lossy(&output.stdout)))
        } else {
             Err(SlyError::Task(format!("Deployment Failed:\n{}", String::from_utf8_lossy(&output.stderr))))
        }
    }

    async fn s3_sync(&self, args: &Value) -> Result<String> {
        let local_dir = args["local_dir"].as_str().ok_or_else(|| SlyError::Task("Missing 'local_dir'".to_string()))?;
        let bucket = args["bucket"].as_str().ok_or_else(|| SlyError::Task("Missing 'bucket'".to_string()))?;

        println!("   ☁️ AWS S3 Sync: {} -> s3://{}", local_dir, bucket);

        let output = Command::new("aws")
            .arg("s3")
            .arg("sync")
            .arg(local_dir)
            .arg(format!("s3://{}", bucket))
            .output()
            .await
            .map_err(|e| SlyError::Task(format!("Failed to execute aws cli: {}", e)))?;
        
        Ok(format!("Sync output: {}", String::from_utf8_lossy(&output.stdout)))
    }
}

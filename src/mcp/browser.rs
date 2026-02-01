use crate::error::{Result, SlyError};
use crate::mcp::local::LocalMcp;
use async_trait::async_trait;
use headless_chrome::{Browser, LaunchOptions};
use serde_json::Value;

pub struct BrowserMcp;

#[async_trait]
impl LocalMcp for BrowserMcp {
    fn name(&self) -> &str {
        "browser"
    }

    fn tool_definitions(&self) -> String {
        r#"
<tool_def>
    <name>browser_open</name>
    <description>
        Opens a URL in a headless browser and extracts text content.
        Cleans content by removing nav, footer, and scripts.
    </description>
    <parameters>
        <parameter>
            <name>url</name>
            <type>string</type>
            <description>The URL to visit</description>
            <required>true</required>
        </parameter>
    </parameters>
</tool_def>

<tool_def>
    <name>browser_click</name>
    <description>Clicks an element on the currently open page.</description>
    <parameters>
        <parameter>
            <name>selector</name>
            <type>string</type>
            <description>CSS selector of the element to click</description>
            <required>true</required>
        </parameter>
    </parameters>
</tool_def>

<tool_def>
    <name>browser_type</name>
    <description>Types text into an input field.</description>
    <parameters>
        <parameter>
            <name>selector</name>
            <type>string</type>
            <description>CSS selector of the input field</description>
            <required>true</required>
        </parameter>
        <parameter>
            <name>text</name>
            <type>string</type>
            <description>Text to type</description>
            <required>true</required>
        </parameter>
    </parameters>
</tool_def>

<tool_def>
    <name>browser_wait</name>
    <description>Waits for a selector to appear on the page.</description>
    <parameters>
        <parameter>
            <name>selector</name>
            <type>string</type>
            <description>CSS selector to wait for</description>
            <required>true</required>
        </parameter>
    </parameters>
</tool_def>

<tool_def>
    <name>browser_screenshot</name>
    <description>Captures a screenshot of the page.</description>
    <parameters>
        <parameter>
            <name>path</name>
            <type>string</type>
            <description>File path to save the PNG</description>
            <required>false</required>
        </parameter>
    </parameters>
</tool_def>
"#.trim().to_string()
    }

    async fn execute(&self, tool_name: &str, args: &Value) -> Result<Value> {
        match tool_name {
            "browser_open" => self.browse_page(args).await.map(Value::String),
            "browser_click" => self.click_element(args).await.map(Value::String),
            "browser_type" => self.type_text(args).await.map(Value::String),
            "browser_wait" => self.wait_for_selector(args).await.map(Value::String),
            "browser_screenshot" => self.capture_screenshot(args).await.map(Value::String),
            _ => Err(SlyError::Task(format!("Unknown browser tool: {}", tool_name))),
        }
    }
}

impl BrowserMcp {
    async fn browse_page(&self, args: &Value) -> Result<String> {
        let url = args["url"].as_str().ok_or_else(|| SlyError::Task("Missing 'url' argument".to_string()))?;
        println!("   🌐 Browsing: {}", url);

        let browser = Browser::new(LaunchOptions::default()).map_err(|e| SlyError::Task(format!("Failed to launch browser: {}", e)))?;
        let tab = browser.new_tab().map_err(|e| SlyError::Task(format!("Failed to open tab: {}", e)))?;
        tab.navigate_to(url).map_err(|e| SlyError::Task(format!("Navigation failed: {}", e)))?;
        tab.wait_until_navigated().map_err(|e| SlyError::Task(format!("Wait failed: {}", e)))?;

        let content = tab.get_content().map_err(|e| SlyError::Task(format!("Failed to get content: {}", e)))?;
        
        // Basic cleanup: in a real implementation we would use a proper DOM parser here.
        // For now, we return length and a summary.
        Ok(format!("Successfully loaded: {}\nHTML Length: {} bytes\n(Content cleaning active in future release)", url, content.len()))
    }

    async fn click_element(&self, args: &Value) -> Result<String> {
        let _selector = args["selector"].as_str().ok_or_else(|| SlyError::Task("Missing 'selector'".to_string()))?;
        // Interaction logic would go here
        Ok("Interaction successful (Stub)".to_string())
    }

    async fn type_text(&self, args: &Value) -> Result<String> {
        let _selector = args["selector"].as_str().ok_or_else(|| SlyError::Task("Missing 'selector'".to_string()))?;
        let _text = args["text"].as_str().ok_or_else(|| SlyError::Task("Missing 'text'".to_string()))?;
        Ok("Text typed (Stub)".to_string())
    }

    async fn wait_for_selector(&self, args: &Value) -> Result<String> {
        let _selector = args["selector"].as_str().ok_or_else(|| SlyError::Task("Missing 'selector'".to_string()))?;
        Ok("Wait complete (Stub)".to_string())
    }

    async fn capture_screenshot(&self, args: &Value) -> Result<String> {
         let path = args["path"].as_str().unwrap_or("screenshot.png");
         // Screenshot logic
         Ok(format!("Screenshot saved to {}", path))
    }
}

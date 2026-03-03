use crate::error::{Result, SlyError};
use std::sync::Arc;
use crate::io::events::Impulse;
use std::path::Path;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::io::interface::{AgentIO, InputMessage, IoModality};
use std::collections::VecDeque;

#[derive(Debug, Serialize, Deserialize)]
pub struct Chat {
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub message_id: i64,
    pub chat: Chat,
    pub text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub message: Option<Message>,
    pub data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InlineKeyboardButton {
    pub text: String,
    pub callback_data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InlineKeyboardMarkup {
    pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
    pub callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TelegramResponse<T> {
    pub ok: bool,
    pub result: T,
}

pub struct TelegramClient {
    token: String,
    client: Client,
    chat_id: Option<i64>,
    // Polling state
    offset: i64,
    buffer: VecDeque<Update>,
}

impl TelegramClient {
    pub fn new(token: String) -> Self {
        Self {
            token,
            client: Client::new(),
            chat_id: None,
            offset: 0,
            buffer: VecDeque::new(),
        }
    }

    pub fn set_chat_id(&mut self, chat_id: i64) {
        self.chat_id = Some(chat_id);
    }

    pub fn chat_id_is_set(&self) -> bool {
        self.chat_id.is_some()
    }

    pub async fn get_me(&self) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/getMe", self.token);
        println!("🔍 Checking Identity: {}", url.replace(&self.token, "TOKEN"));
        let res = self.client.get(&url).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        let status = res.status();
        println!("📩 getMe Status: {}", status);
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Telegram verification failed: {} - {}", status, err_text)));
        }
        Ok(())
    }

    pub async fn send_message(&self, text: &str) -> Result<i64> {
        let chat_id = self.chat_id.ok_or_else(|| SlyError::Task("Telegram Chat ID not set".to_string()))?;
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let payload = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML"
        });
        println!("📤 Outgoing Message: {} (to {})", text, chat_id);

        let res = self.client.post(&url).json(&payload).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Telegram API Error: {} - {}", status, err_text)));
        }

        let resp = res.json::<TelegramResponse<Message>>().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        if !resp.ok {
             return Err(SlyError::Cortex("Telegram API returned ok: false".to_string()));
        }
        Ok(resp.result.message_id)
    }

    pub async fn send_message_with_markup(&self, text: &str, markup: InlineKeyboardMarkup) -> Result<i64> {
        let chat_id = self.chat_id.ok_or_else(|| SlyError::Task("Telegram Chat ID not set".to_string()))?;
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let payload = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML",
            "reply_markup": markup
        });
        println!("📤 Outgoing Markup Message: {} (to {})", text, chat_id);

        let res = self.client.post(&url).json(&payload).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Telegram API Error: {}", err_text)));
        }

        let resp = res.json::<TelegramResponse<Message>>().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
         if !resp.ok {
             return Err(SlyError::Cortex("Telegram API returned ok: false".to_string()));
        }
        Ok(resp.result.message_id)
    }
    
    pub async fn edit_message_text(&self, message_id: i64, text: &str) -> Result<()> {
        let chat_id = self.chat_id.ok_or_else(|| SlyError::Task("Telegram Chat ID not set".to_string()))?;
        let url = format!("https://api.telegram.org/bot{}/editMessageText", self.token);
        
        let payload = json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
            "parse_mode": "HTML"
        });

        let res = self.client.post(&url).json(&payload).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            // Ignore "message is not modified" errors to prevent spamming logs
            if !err_text.contains("message is not modified") {
                return Err(SlyError::Cortex(format!("Telegram API Error: {}", err_text)));
            }
        }
        Ok(())
    }

    pub async fn answer_callback_query(&self, callback_query_id: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/answerCallbackQuery", self.token);
        let payload = json!({
            "callback_query_id": callback_query_id
        });

        let res = self.client.post(&url).json(&payload).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Telegram API Error: {}", err_text)));
        }

        Ok(())
    }

    pub async fn send_photo(&self, photo_path: &Path, caption: Option<&str>) -> Result<()> {
        let chat_id = self.chat_id.ok_or_else(|| SlyError::Task("Telegram Chat ID not set".to_string()))?;
        let url = format!("https://api.telegram.org/bot{}/sendPhoto", self.token);

        let file_name = photo_path.file_name()
            .and_then(|n: &std::ffi::OsStr| n.to_str())
            .unwrap_or("photo.png")
            .to_string();

        let file_bytes = std::fs::read(photo_path).map_err(|e: std::io::Error| SlyError::Io(e))?;
        let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

        let mut form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .part("photo", part);

        if let Some(c) = caption {
            form = form.text("caption", c.to_string()).text("parse_mode", "HTML");
        }

        let res = self.client.post(&url).multipart(form).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Telegram API Error: {} - {}", status, err_text)));
        }

        Ok(())
    }

    pub async fn send_document(&self, doc_path: &Path, caption: Option<&str>, markup: Option<InlineKeyboardMarkup>) -> Result<()> {
        let chat_id = self.chat_id.ok_or_else(|| SlyError::Task("Telegram Chat ID not set".to_string()))?;
        let url = format!("https://api.telegram.org/bot{}/sendDocument", self.token);

        let file_name = doc_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.md")
            .to_string();

        let file_bytes = std::fs::read(doc_path).map_err(|e: std::io::Error| SlyError::Io(e))?;
        let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

        let mut form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .part("document", part);

        if let Some(c) = caption {
            form = form.text("caption", c.to_string()).text("parse_mode", "HTML");
        }
        
        if let Some(m) = markup {
            form = form.text("reply_markup", serde_json::to_string(&m).unwrap_or_default());
        }

        let res = self.client.post(&url).multipart(form).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Telegram API Error: {}", err_text)));
        }

        Ok(())
    }

    pub async fn edit_message_reply_markup(&self, message_id: i64, markup: Option<InlineKeyboardMarkup>) -> Result<()> {
        let chat_id = self.chat_id.ok_or_else(|| SlyError::Task("Telegram Chat ID not set".to_string()))?;
        let url = format!("https://api.telegram.org/bot{}/editMessageReplyMarkup", self.token);
        
        let payload = json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reply_markup": markup
        });

        let res = self.client.post(&url).json(&payload).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(SlyError::Cortex(format!("Telegram API Error: {}", err_text)));
        }

        Ok(())
    }

    pub async fn get_updates(&self, offset: i64) -> Result<Vec<Update>> {
        let url = format!("https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=5", self.token, offset);
        
        // Use GET instead of POST to simplify
        let res = self.client.get(&url).send().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
             return Err(SlyError::Cortex(format!("Telegram API Error: {} - {}", status, err_text)));
        }

        let resp = res.json::<TelegramResponse<Vec<Update>>>().await.map_err(|e: reqwest::Error| SlyError::Cortex(e.to_string()))?;
        if !resp.ok {
            return Err(SlyError::Cortex("Telegram API returned ok: false".to_string()));
        }

        Ok(resp.result)
    }
}

#[async_trait::async_trait]
impl AgentIO for TelegramClient {
    fn modality(&self) -> IoModality {
        IoModality::Telegram
    }

    async fn next_message(&mut self) -> Result<Option<InputMessage>> {
        // 1. Drain buffer first
        if let Some(update) = self.buffer.pop_front() {
             return Ok(self.process_update(update));
        }

        // 2. Fetch new updates
        match self.get_updates(self.offset).await {
            Ok(updates) => {
                if updates.is_empty() {
                    return Ok(None);
                }
                for update in updates {
                    self.offset = update.update_id + 1;
                    self.buffer.push_back(update);
                }
                // Return the first one we just fetched
                if let Some(first) = self.buffer.pop_front() {
                    Ok(self.process_update(first))
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                eprintln!("Telegram Polling Error: {}", e);
                // Return None (or error?) to retry
                // For now, let's sleep a bit to verify we don't spam loop on error, 
                // but traits are async so caller handles loop.
                Ok(None)
            }
        }
    }

    async fn send_message(&mut self, content: &str) -> Result<()> {
        self.send_message(content).await.map(|_| ())
    }
}

impl TelegramClient {
    fn process_update(&self, update: Update) -> Option<InputMessage> {
        let mut sender = "unknown".to_string();
        let content = if let Some(msg) = update.message {
             sender = format!("telegram_user_{}", msg.chat.id);
             msg.text?
        } else if let Some(cb) = update.callback_query {
             // Handle callbacks as user inputs?
             // Simplification: Return data as text
             cb.data?
        } else {
            return None;
        };

        Some(InputMessage {
            content,
            sender,
            session_id: "default_telegram_session".to_string(),
            metadata: None,
        })
    }
}

pub struct TelegramAdapter(pub Arc<tokio::sync::Mutex<TelegramClient>>);

#[async_trait::async_trait]
impl crate::io::adapter::SlyAdapter for TelegramAdapter {
    fn name(&self) -> &str { "telegram" }
    async fn handle(&self, event: crate::core::bus::SlyEvent) -> Result<()> {
        self.0.lock().await.handle(event).await
    }
}

#[async_trait::async_trait]
impl crate::io::adapter::SlyAdapter for TelegramClient {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn handle(&self, event: crate::core::bus::SlyEvent) -> Result<()> {
        use crate::core::bus::SlyEvent;
        match event {
            SlyEvent::Thought(_, ref content) | SlyEvent::ThoughtStream(_, ref content) => {
                // If it's a stream, we should probably debounce or use edit_message
                // For now, let's just handle complete Thoughts to keep it simple
                if matches!(event, SlyEvent::Thought(_, _)) {
                    let _ = self.send_message(content).await;
                }
            }
            SlyEvent::Error(_, msg) => {
                let _ = self.send_message(&format!("❌ <b>Error</b>\n\n{}", html_escape(&msg))).await;
            }
            SlyEvent::Action(_, msg) => {
                let _ = self.send_message(&format!("⚡ <b>Action</b>: {}", html_escape(&msg))).await;
            }
            SlyEvent::SystemStatus(status) => {
                let _ = self.send_message(&format!("ℹ️ <b>System</b>: {}", html_escape(&status))).await;
            }
            _ => {} // Ignore other events for now
        }
        Ok(())
    }
}

impl TelegramClient {
    pub async fn start_polling(this: Arc<tokio::sync::Mutex<Self>>, bus: crate::core::bus::ArcBus) {
        let mut offset = 0;
        println!("📡 Telegram Polling Started.");
        
        loop {
            let updates = {
                let tg = this.lock().await;
                tg.get_updates(offset).await
            };

            match updates {
                Ok(u) => {
                    for update in u {
                        offset = update.update_id + 1;
                        if let Some(msg) = update.message {
                            if let Some(text) = msg.text {
                                if text.starts_with('/') {
                                    let cmd = text.trim_start_matches('/');
                                    let parts: Vec<&str> = cmd.split_whitespace().collect();
                                    let name = parts[0];
                                    
                                    match name {
                                        "run" => {
                                            let task = parts[1..].join(" ");
                                            let _ = bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::InitiateSession(task))).await;
                                        }
                                        "stop" => {
                                            let _ = bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::SystemInterrupt)).await;
                                        }
                                        _ => {
                                            // Assume it's a workflow
                                            let _ = bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::ExecuteWorkflow(name.to_string()))).await;
                                        }
                                    }
                                } else {
                                    // Treat as a new session request
                                    let _ = bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::InitiateSession(text))).await;
                                }
                            }
                        }
                        if let Some(cb) = update.callback_query {
                            if let Some(data) = cb.data {
                                if data.starts_with("think:") {
                                    let session_id = data.replace("think:", "");
                                    let _ = bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::ThinkStep(session_id))).await;
                                } else if data.starts_with("undo:") {
                                    let session_id = data.replace("undo:", "");
                                    let _ = bus.publish(crate::core::bus::SlyEvent::Impulse(Impulse::Undo(session_id))).await;
                                }
                                // Answer callback to stop the spinning circle
                                let _ = this.lock().await.answer_callback_query(&cb.id).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Telegram Polling Error: {}", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
    }
}

pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn test_client_init() {
        let client = TelegramClient::new("fake_token".to_string());
        assert_eq!(client.token, "fake_token");
        assert!(client.chat_id.is_none());
    }
}

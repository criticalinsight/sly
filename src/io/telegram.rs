use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

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
struct GetUpdatesResponse {
    pub ok: bool,
    pub result: Vec<Update>,
}

pub struct TelegramClient {
    token: String,
    client: Client,
    chat_id: Option<i64>,
}

impl TelegramClient {
    pub fn new(token: String) -> Self {
        Self {
            token,
            client: Client::new(),
            chat_id: None,
        }
    }

    pub fn set_chat_id(&mut self, chat_id: i64) {
        self.chat_id = Some(chat_id);
    }

    pub async fn send_message(&self, text: &str) -> Result<()> {
        let chat_id = self.chat_id.ok_or_else(|| anyhow!("Chat ID not set"))?;
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let payload = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "MarkdownV2"
        });

        let res = self.client.post(&url).json(&payload).send().await?;
        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(anyhow!("Telegram API Error: {} - {}", status, err_text));
        }

        Ok(())
    }

    pub async fn send_message_with_markup(&self, text: &str, markup: InlineKeyboardMarkup) -> Result<()> {
        let chat_id = self.chat_id.ok_or_else(|| anyhow!("Chat ID not set"))?;
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let payload = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "MarkdownV2",
            "reply_markup": markup
        });

        let res = self.client.post(&url).json(&payload).send().await?;
        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(anyhow!("Telegram API Error: {}", err_text));
        }

        Ok(())
    }

    pub async fn answer_callback_query(&self, callback_query_id: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/answerCallbackQuery", self.token);
        let payload = json!({
            "callback_query_id": callback_query_id
        });

        let res = self.client.post(&url).json(&payload).send().await?;
        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(anyhow!("Telegram API Error: {}", err_text));
        }

        Ok(())
    }

    pub async fn get_updates(&self, offset: i64) -> Result<Vec<Update>> {
        let url = format!("https://api.telegram.org/bot{}/getUpdates", self.token);
        let payload = json!({
            "offset": offset,
            "timeout": 30
        });

        let res = self.client.post(&url).json(&payload).send().await?;
        let status = res.status();
        if !status.is_success() {
            let err_text = res.text().await.unwrap_or_default();
            return Err(anyhow!("Telegram API Error: {} - {}", status, err_text));
        }

        let resp: GetUpdatesResponse = res.json().await?;
        if !resp.ok {
            return Err(anyhow!("Telegram API returned ok: false"));
        }

        Ok(resp.result)
    }
}

pub fn escape_markdown_v2(text: &str) -> String {
    // Basic MarkdownV2 escaping for Telegram
    let escape_chars = ['_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!'];
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if escape_chars.contains(&c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

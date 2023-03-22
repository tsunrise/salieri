use async_stream::try_stream;
use futures_util::TryStream;
use futures_util::{StreamExt, TryStreamExt};

use std::str;
use worker::{console_log, ByteStream};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum FinishReason {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "content_filter")]
    ContentFilter,
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Serialize)]
pub enum StreamItem {
    RoleMsg,
    #[serde(rename = "start")]
    Start(u32), // token limit
    #[serde(rename = "delta")]
    Delta(String),
    #[serde(rename = "finish")]
    Finish(FinishReason),
}

impl StreamItem {
    pub fn from_json_str(s: &str) -> Option<Self> {
        if let Ok(s) = serde_json::from_str::<serde_json::Value>(s) {
            let choice = s.get("choices")?.get(0)?;
            let finish_reason = choice.get("finish_reason")?;
            if finish_reason != &serde_json::Value::Null {
                let finish_reason =
                    serde_json::from_value::<FinishReason>(finish_reason.clone()).ok()?;
                Some(StreamItem::Finish(finish_reason))
            } else {
                let delta = choice.get("delta")?;
                if let Some(_) = delta.get("role") {
                    Some(StreamItem::RoleMsg)
                } else {
                    let content = delta.get("content")?;
                    let content = serde_json::from_value::<String>(content.clone()).ok()?;
                    Some(StreamItem::Delta(content))
                }
            }
        } else {
            None
        }
    }
}

pub struct ChatStreamParser {
    buffer: String,
}

impl ChatStreamParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn add_chunk(&mut self, chunk: &[u8]) {
        if let Ok(chunk_str) = str::from_utf8(chunk) {
            self.buffer.push_str(chunk_str);
        }
    }

    pub fn next(&mut self) -> Option<StreamItem> {
        if let Some(start) = self.buffer.find("data:") {
            let json_start = start + "data:".len();
            let json_end = self.buffer[json_start..].find('\n').map(|i| json_start + i);
            if let Some(end) = json_end {
                let json_str = self.buffer[json_start..end].trim().to_string();
                self.buffer.drain(..end);
                return Some(StreamItem::from_json_str(&json_str)?);
            }
        }
        None
    }

    pub fn parse_byte_stream(
        stream: ByteStream,
    ) -> impl TryStream<Item = worker::Result<StreamItem>> + Unpin {
        Box::pin(try_stream! {
            let mut parser = ChatStreamParser::new();
            let mut stream = stream.into_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                parser.add_chunk(&chunk);
                while let Some(json) = parser.next() {
                    yield json;
                }
            }
        })
    }
}

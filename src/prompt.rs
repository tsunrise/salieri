use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub prompt: Prompt,
    pub questions: Vec<String>,
    pub welcome: String,
    pub announcement: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct RequestToOpenAI {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub stream: bool, // true
}

pub fn get_local_datetime(timezone: impl chrono::TimeZone) -> String {
    use chrono::prelude::*;
    let js_date = js_sys::Date::new_0();
    let now_timestamp = js_date.get_time() / 1000.; // convert milliseconds to seconds
    let naive_datetime =
        chrono::NaiveDateTime::from_timestamp_opt(now_timestamp as i64, 0).unwrap();
    let local_datetime = timezone.from_utc_datetime(&naive_datetime);
    format!(
        "{}-{:02}-{:02} {}:{}:{}",
        local_datetime.year(),
        local_datetime.month(),
        local_datetime.day(),
        local_datetime.hour(),
        local_datetime.minute(),
        local_datetime.second()
    )
}

impl RequestToOpenAI {
    pub fn new(
        mut prompt: Prompt,
        user_question: String,
        timezone: impl chrono::TimeZone,
    ) -> Result<Self> {
        // length check
        const MAX_LENGTH: usize = 300; // TODO: make this configurable
        if user_question.len() > MAX_LENGTH {
            return Err(Error::InvalidRequest(format!(
                "question is too long ({} > {})",
                user_question.len(),
                MAX_LENGTH
            )));
        }

        prompt.messages.push(Message {
            role: Role::User,
            content: user_question,
        });

        if prompt.messages[0].role != Role::System {
            return Err(Error::InternalError(
                "first message must be a system message".to_string(),
            ));
        }

        // replace [CURRENT_DATE] in first message with current date
        let local_date = get_local_datetime(timezone);

        prompt.messages[0].content = prompt.messages[0]
            .content
            .replace("[CURRENT_TIME]", &local_date);

        Ok(Self {
            model: prompt.model,
            messages: prompt.messages,
            max_tokens: prompt.max_tokens.unwrap_or(128),
            stream: true,
        })
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UserRequest {
    pub question: String,
    pub captcha_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_to_prompt() {
        let expected = Prompt {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![
                Message {
                    role: Role::System,
                    content: "You are a helpful chatbot.".to_string(),
                },
                Message {
                    role: Role::User,
                    content: "Hello!".to_string(),
                },
            ],
            max_tokens: None,
        };

        let json = r#"{
            "model": "gpt-3.5-turbo",
            "messages": [{"role": "system", "content": "You are a helpful chatbot."}, {"role": "user", "content": "Hello!"}],
            "extra": "this field is ignored"
          }
          "#;

        let actual: Prompt = serde_json::from_str(json).unwrap();
        assert_eq!(expected, actual);

        let toml = r#"
            model = "gpt-3.5-turbo"
            messages = [{role = "system", content = "You are a helpful chatbot."}, {role = "user", content = "Hello!"}]
        "#;

        let actual: Prompt = toml::from_str(toml).unwrap();
        assert_eq!(expected, actual);

        let expected = Prompt {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![
                Message {
                    role: Role::System,
                    content: "You are a helpful chatbot.".to_string(),
                },
                Message {
                    role: Role::User,
                    content: "Hello!".to_string(),
                },
            ],
            max_tokens: Some(126),
        };

        let toml = r#"
        model = "gpt-3.5-turbo"
        messages = [{role = "system", content = "You are a helpful chatbot."}, {role = "user", content = "Hello!"}]
        max_tokens = 126
    "#;

        let actual: Prompt = toml::from_str(toml).unwrap();
        assert_eq!(expected, actual);
    }
}

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

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: Option<u32>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct RequestToOpenAI {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub stream: bool, // true
}

impl RequestToOpenAI {
    pub fn new(mut prompt: Prompt, user_question: String) -> Self {
        prompt.messages.push(Message {
            role: Role::User,
            content: user_question,
        });
        Self {
            model: prompt.model,
            messages: prompt.messages,
            max_tokens: prompt.max_tokens.unwrap_or(128),
            stream: true,
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UserRequest {
    pub question: String,
    // TODO:
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
            "messages": [{"role": "system", "content": "You are a helpful chatbot."}, {"role": "user", "content": "Hello!"}]
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

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Prompt{
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub stream: bool,
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn json_to_prompt(){
        let expected = Prompt{
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![Message{
                role: Role::User,
                content: "Hello!".to_string()
            }],
            stream: false
        };

        let json = r#"{
            "model": "gpt-3.5-turbo",
            "messages": [{"role": "user", "content": "Hello!"}]
          }
          "#;

        let actual: Prompt = serde_json::from_str(json).unwrap();
        assert_eq!(expected, actual);

        let toml = r#"
            model = "gpt-3.5-turbo"
            messages = [{role = "user", content = "Hello!"}]
        "#;

        let actual: Prompt = toml::from_str(toml).unwrap();
        assert_eq!(expected, actual);
    }
}
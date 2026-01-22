use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Directive {
    pub type_name: String,
    pub payload: Value,
}

impl Directive {
    pub fn new(type_name: &str, payload: Value) -> Self {
        Self {
            type_name: type_name.to_string(),
            payload,
        }
    }
}

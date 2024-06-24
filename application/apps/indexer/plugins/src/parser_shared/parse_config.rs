#[derive(Debug, Clone)]
pub struct ParserConfig {
    pub placeholder: String,
}

impl ParserConfig {
    pub fn new(placeholder: String) -> Self {
        Self { placeholder }
    }
}

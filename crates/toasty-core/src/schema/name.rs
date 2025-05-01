use std_util::str;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Name {
    pub parts: Vec<String>,
}

impl Name {
    pub fn new(src: &str) -> Self {
        // TODO: make better
        let snake = str::snake_case(src);
        let parts = snake.split("_").map(String::from).collect();
        Self { parts }
    }

    pub fn camel_case(&self) -> String {
        str::camel_case(&self.snake_case())
    }

    pub fn upper_camel_case(&self) -> String {
        str::upper_camel_case(&self.snake_case())
    }

    pub fn snake_case(&self) -> String {
        self.parts.join("_")
    }

    pub fn upper_snake_case(&self) -> String {
        str::upper_snake_case(&self.snake_case())
    }
}

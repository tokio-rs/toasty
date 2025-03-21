pub fn snake_case(string: &str) -> String {
    use heck::ToSnakeCase;
    string.to_snake_case()
}

pub fn upper_snake_case(string: &str) -> String {
    use heck::ToShoutySnakeCase;
    string.to_shouty_snake_case()
}

pub fn camel_case(string: &str) -> String {
    use heck::ToLowerCamelCase;
    string.to_lower_camel_case()
}

pub fn upper_camel_case(string: &str) -> String {
    use heck::ToUpperCamelCase;
    string.to_upper_camel_case()
}

pub fn pluralize(word: &str) -> String {
    pluralizer::pluralize(word, 2, false)
}

pub fn singularize(word: &str) -> String {
    pluralizer::pluralize(word, 1, false)
}

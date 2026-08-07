use heck::{ToSnakeCase, ToUpperCamelCase};

/// A multi-part identifier that can be rendered in snake case or upper camel case.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::Name;
///
/// let name = Name::new("UserProfile");
/// assert_eq!(name.snake_case(), "user_profile");
/// assert_eq!(name.upper_camel_case(), "UserProfile");
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Name {
    /// The individual lowercase word parts of this name.
    pub parts: Vec<String>,
}

impl Name {
    /// Creates a new `Name` by splitting `src` into word parts.
    ///
    /// The input is first converted to snake_case, then split on underscores.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::Name;
    ///
    /// let name = Name::new("myField");
    /// assert_eq!(name.parts, vec!["my", "field"]);
    /// ```
    pub fn new(src: &str) -> Self {
        // TODO: make better
        let snake = src.to_snake_case();
        let parts = snake.split('_').map(String::from).collect();
        Self { parts }
    }

    /// Returns this name in `UpperCamelCase` (PascalCase).
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::Name;
    ///
    /// assert_eq!(Name::new("user_id").upper_camel_case(), "UserId");
    /// ```
    pub fn upper_camel_case(&self) -> String {
        self.snake_case().to_upper_camel_case()
    }

    /// Returns this name in `snake_case`.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::Name;
    ///
    /// assert_eq!(Name::new("UserProfile").snake_case(), "user_profile");
    /// ```
    pub fn snake_case(&self) -> String {
        self.parts.join("_")
    }
}

impl core::fmt::Display for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.upper_camel_case())
    }
}

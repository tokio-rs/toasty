use std_util::str;

/// A multi-part identifier that can be rendered in various casing conventions.
///
/// `Name` stores the identifier as individual lowercase words (parts). It is
/// created from a string in any common casing style (snake_case, camelCase,
/// PascalCase) and can be converted back to any of those forms.
///
/// # Examples
///
/// ```
/// use toasty_core::schema::Name;
///
/// let name = Name::new("UserProfile");
/// assert_eq!(name.snake_case(), "user_profile");
/// assert_eq!(name.upper_camel_case(), "UserProfile");
/// assert_eq!(name.camel_case(), "userProfile");
/// assert_eq!(name.upper_snake_case(), "USER_PROFILE");
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
        let snake = str::snake_case(src);
        let parts = snake.split("_").map(String::from).collect();
        Self { parts }
    }

    /// Returns this name in `camelCase`.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::Name;
    ///
    /// assert_eq!(Name::new("user_id").camel_case(), "userId");
    /// ```
    pub fn camel_case(&self) -> String {
        str::camel_case(&self.snake_case())
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
        str::upper_camel_case(&self.snake_case())
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

    /// Returns this name in `UPPER_SNAKE_CASE`.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::Name;
    ///
    /// assert_eq!(Name::new("user_id").upper_snake_case(), "USER_ID");
    /// ```
    pub fn upper_snake_case(&self) -> String {
        str::upper_snake_case(&self.snake_case())
    }
}

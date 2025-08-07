use nadi_plugin::nadi_internal_plugin;

/// This plugin provides the functions for the regex operations in
/// string. Refer to the
/// [regex](https://docs.rs/regex/latest/regex/index.html) crate for
/// more details on the regex patterns.
#[nadi_internal_plugin]
mod regex {
    use nadi_plugin::env_func;
    use regex::Regex;

    /// Filter from the string list with only the values matching pattern
    ///
    /// ```task
    /// env assert_eq(str_filter(["abc", "and", "xyz"], "^a"), ["abc", "and"])
    /// ```
    #[env_func]
    fn str_filter(
        /// attribute to check for pattern
        #[relaxed]
        attrs: Vec<String>,
        /// Regex pattern to match
        pattern: Regex,
    ) -> Vec<String> {
        attrs.into_iter().filter(|a| pattern.is_match(a)).collect()
    }

    /// Check if the given pattern matches the value or not
    ///
    /// You can also use match operator for this
    ///
    /// ```task
    /// env assert_eq(str_match("abc", "^a"), true)
    /// env assert_eq(str_match("abc", "^a"), "abc" match "^a")
    /// ```
    #[env_func]
    fn str_match(
        /// attribute to check for pattern
        #[relaxed]
        attr: &str,
        /// Regex pattern to match
        pattern: Regex,
    ) -> bool {
        pattern.is_match(attr)
    }

    /// Replace the occurances of the given match
    ///
    /// ```task
    /// env assert_eq(str_replace("abc", "^a", 2), "2bc")
    /// env assert_eq(str_replace("abc", "[abc]", 2), "222")
    /// ```
    #[env_func]
    fn str_replace(
        /// original string
        #[relaxed]
        attr: &str,
        /// Regex pattern to match
        pattern: Regex,
        /// replacement string
        #[relaxed]
        rep: &str,
    ) -> String {
        pattern.replace_all(attr, rep).to_string()
    }

    /// Find the given pattern in the value
    ///
    /// ```task
    /// env assert_eq(str_find("abc", "^[ab]"), "a")
    /// ```
    #[env_func]
    fn str_find(
        /// attribute to check for pattern
        #[relaxed]
        attr: &str,
        /// Regex pattern to match
        pattern: Regex,
    ) -> Option<String> {
        pattern.find(attr).map(|m| m.as_str().to_string())
    }

    /// Find all the matches of the given pattern in the value
    ///
    /// ```task
    /// env assert_eq(str_find_all("abc", "[ab]"), ["a", "b"])
    /// ```
    #[env_func]
    fn str_find_all(
        /// attribute to check for pattern
        #[relaxed]
        attr: &str,
        /// Regex pattern to match
        pattern: Regex,
    ) -> Vec<String> {
        pattern
            .captures_iter(attr)
            .map(|c| c[0].to_string())
            .collect()
    }

    /// Count the number of matches of given pattern in the string
    ///
    /// ```task
    /// env assert_eq(str_count("abc", "[ab]"), 2)
    /// ```
    #[env_func]
    fn str_count(
        /// attribute to check for pattern
        #[relaxed]
        attr: &str,
        /// Regex pattern to match
        pattern: Regex,
    ) -> usize {
        pattern.captures_iter(attr).count()
    }

    /// Split the string with the given pattern
    ///
    /// ```task
    /// env assert_eq(str_split("abc", "^[ab]"), ["", "bc"])
    /// env assert_eq(str_split("abc", "[ab]"), ["", "", "c"])
    /// env assert_eq(str_split("abc", "[ab]", limit=2), ["", "bc"])
    /// ```
    #[env_func]
    fn str_split(
        /// String to split
        attr: &str,
        /// Regex pattern to split with
        pattern: Regex,
        /// Limit the substrings to this number
        limit: Option<usize>,
    ) -> Vec<String> {
        if let Some(l) = limit {
            pattern.splitn(attr, l).map(String::from).collect()
        } else {
            pattern.split(attr).map(String::from).collect()
        }
    }
}

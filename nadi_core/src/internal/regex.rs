use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod regex {
    use nadi_plugin::env_func;
    use regex::Regex;

    /// Check if the given pattern matches the value or not
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

    /// Count the number of matches of given pattern in the value
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
}

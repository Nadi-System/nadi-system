use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod regex {
    use crate::convert_impls;
    use crate::prelude::*;
    use anyhow::Result;
    use nadi_plugin::nadi_func;
    use regex::Regex;

    convert_impls!(String => Regex);

    /// Check if the given pattern matches the value or not
    #[nadi_func]
    fn str_match(
        /// Regex pattern to match
        pattern: Regex,
        /// attribute to check for pattern
        #[relaxed]
        attr: &str,
    ) -> bool {
        pattern.is_match(attr)
    }

    /// Replace the occurances of the given match
    #[nadi_func]
    fn str_replace(
        /// Regex pattern to match
        pattern: Regex,
        /// attribute to replace
        #[relaxed]
        attr: &str,
        /// replacement string
        #[relaxed]
        rep: &str,
    ) -> String {
        pattern.replace_all(attr, rep).to_string()
    }

    /// Find the given pattern in the value
    #[nadi_func]
    fn str_find(
        /// Regex pattern to match
        pattern: Regex,
        /// attribute to check for pattern
        #[relaxed]
        attr: &str,
    ) -> Option<String> {
        pattern.find(attr).map(|m| m.as_str().to_string())
    }

    /// Find all the matches of the given pattern in the value
    #[nadi_func]
    fn str_find_all(
        /// Regex pattern to match
        pattern: Regex,
        /// attribute to check for pattern
        #[relaxed]
        attr: &str,
    ) -> Vec<String> {
        pattern
            .captures_iter(attr)
            .map(|c| c[0].to_string())
            .collect()
    }

    /// Count the number of matches of given pattern in the value
    #[nadi_func]
    fn str_count(
        /// Regex pattern to match
        pattern: Regex,
        /// attribute to check for pattern
        #[relaxed]
        attr: &str,
    ) -> usize {
        pattern.captures_iter(attr).count()
    }
}

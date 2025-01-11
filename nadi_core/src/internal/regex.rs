use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod regex {
    use crate::convert_impls;
    use crate::prelude::*;
    use anyhow::Result;
    use nadi_plugin::nadi_func;
    use regex::Regex;

    convert_impls!(String => Regex);

    /// Set the node size of the nodes based on the attribute value
    #[nadi_func]
    fn str_match(pattern: Regex, #[relaxed] attr: &str) -> bool {
        pattern.is_match(attr)
    }

    /// Set the node size of the nodes based on the attribute value
    #[nadi_func]
    fn str_replace(pattern: Regex, #[relaxed] attr: &str, #[relaxed] rep: &str) -> String {
        pattern.replace_all(attr, rep).to_string()
    }

    /// Set the node size of the nodes based on the attribute value
    #[nadi_func]
    fn str_find(pattern: Regex, #[relaxed] attr: &str) -> Option<String> {
        pattern.find(attr).map(|m| m.as_str().to_string())
    }

    /// Set the node size of the nodes based on the attribute value
    #[nadi_func]
    fn str_find_all(pattern: Regex, #[relaxed] attr: &str) -> Vec<String> {
        pattern
            .captures_iter(attr)
            .map(|c| c[0].to_string())
            .collect()
    }

    /// Set the node size of the nodes based on the attribute value
    #[nadi_func]
    fn str_count(pattern: Regex, #[relaxed] attr: &str) -> usize {
        pattern.captures_iter(attr).count()
    }
}

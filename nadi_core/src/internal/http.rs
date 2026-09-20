use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod http {
    use nadi_plugin::env_func;

    /// Request text response from a url
    #[env_func]
    fn request(
        /// url to request
        url: String,
    ) -> anyhow::Result<String> {
        Ok(reqwest::blocking::get(url)?.text()?)
    }
}

use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod connections {
    use crate::prelude::*;
    use nadi_plugin::network_func;
    use std::path::PathBuf;

    /// Load the given file into the network
    #[network_func]
    fn load_file(net: &mut Network, file: PathBuf) -> anyhow::Result<()> {
        *net = Network::from_file(file)?;
        Ok(())
    }
}

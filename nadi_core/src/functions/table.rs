use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod table {
    use crate::network::Network;
    use crate::table::Table;

    use nadi_plugin::network_func;
    use std::io::Write;
    use std::path::PathBuf;
    use std::str::FromStr;

    /// Render the Table as a rendered markdown
    #[network_func]
    fn table_to_markdown(
        net: &mut Network,
        table: Option<PathBuf>,
        template: Option<String>,
        outfile: Option<PathBuf>,
        connections: Option<String>,
    ) -> anyhow::Result<()> {
        let tab = match (table, template) {
            (Some(t), None) => Table::from_file(t)?,
            (None, Some(t)) => Table::from_str(&t)?,
            (Some(_), Some(_)) => return Err(anyhow::Error::msg("table and template both given")),
            (None, None) => return Err(anyhow::Error::msg("neither table nor template given")),
        };
        let md = tab.render_markdown(net, connections)?;
        if let Some(out) = outfile {
            let mut output = std::fs::File::create(out)?;
            write!(output, "{md}")?;
        } else {
            println!("{md}");
        }
        Ok(())
    }
}

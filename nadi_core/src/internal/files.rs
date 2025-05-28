use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod files {
    use crate::prelude::*;
    use crate::string_template::Template;
    use nadi_core::nadi_plugin::{env_func, node_func};
    use std::io::Write;
    use std::io::{BufRead, BufReader};
    use std::path::{Path, PathBuf};

    fn file_exists(path: &Path, min_lines: Option<usize>) -> bool {
        if let Some(ml) = min_lines {
            match std::fs::File::open(path) {
                Ok(f) => BufReader::new(f).lines().count() > ml,
                _ => false,
            }
        } else {
            path.exists()
        }
    }

    /// Checks if the given path exists
    #[env_func]
    fn exists(
        /// Path to check
        path: PathBuf,
        /// Minimum number of lines the file should have
        min_lines: Option<usize>,
    ) -> bool {
        file_exists(&path, min_lines)
    }

    /// Checks if the given path exists when rendering the template
    #[node_func]
    fn exists(
        node: &NodeInner,
        /// Path to check
        path: Template,
        /// Minimum number of lines the file should have
        min_lines: Option<usize>,
    ) -> anyhow::Result<bool> {
        let p = node.render(&path)?;
        Ok(file_exists(p.as_ref(), min_lines))
    }

    /// Reads the file contents as string
    #[env_func]
    fn from_file(
        /// File Path to load the contents from
        path: PathBuf,
        /// default value
        default: Option<String>,
    ) -> anyhow::Result<String> {
        let contents = if let Some(val) = default {
            std::fs::read_to_string(path).unwrap_or(val)
        } else {
            std::fs::read_to_string(path)?
        };
        Ok(contents)
    }

    /// Writes the string to the file
    #[env_func(append = false, end = "\n")]
    fn to_file(
        /// Contents to write
        contents: String,
        /// Path to write the file
        path: PathBuf,
        /// Append to the file
        append: bool,
        /// End the write with this
        end: String,
    ) -> anyhow::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(path)?;
        write!(file, "{}{}", contents, end)?;
        Ok(())
    }
}

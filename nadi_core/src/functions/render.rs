use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod render {
    use crate::{Network, NodeInner};
    use nadi_plugin::{network_func, node_func};
    use std::path::PathBuf;
    use string_template_plus::Template;

    /// Render the template based on the node attributes
    ///
    /// # Arguments
    /// - `template` - String template to render
    /// - `safe` - if render fails keep it as it is instead of exiting
    #[node_func(safe = false)]
    fn render(node: &mut NodeInner, template: Template, safe: bool) -> Result<String, String> {
        let text = if safe {
            node.render(&template)
                .unwrap_or_else(|_| template.original().to_string())
        } else {
            node.render(&template).map_err(|e| e.to_string())?
        };
        println!("{text}");
        Ok(text)
    }

    /// Render a File template for the nodes in the whole network
    ///
    /// Write the file with templates for input variables in the same
    /// way you write string templates. It's useful for markdown
    /// files, as the curly braces syntax won't be used for anything
    /// else that way. Do be careful about that. And the program will
    /// replace those templates with their values when you run it with
    /// inputs.
    ///
    /// It'll repeat the same template for each node and render them.
    /// If you want only a portion of the file repeated for nodes
    /// inclose them with lines with `---8<---` on both start and the
    /// end. The lines containing the clip syntax will be ignored,
    /// ideally you can put them in comments.
    ///
    /// You can also use `---include:<filename>[::line_range]` syntax to
    /// include a file, the line_range syntax, if present, should be
    /// in the form of `start[:increment]:end`, you can exclude start
    /// or end to denote the line 1 or last line (e.g. `:5` is 1:5,
    /// and `3:` is from line 3 to the end)
    ///
    /// # Arguments
    /// - `template`: Path to the template file
    /// - `outfile` [Optional]: Path to save the template file, if none it'll be printed in stdout
    #[network_func]
    fn render(
        network: &mut Network,
        template: PathBuf,
        outfile: Option<PathBuf>,
    ) -> anyhow::Result<()> {
        let template = super::render_utils::RenderFileContents::read_file(&template)?;
        template.print_render(network, outfile)
    }
}

mod render_utils {
    use crate::{functions::Propagation, Network};
    use anyhow::{Context, Error};
    use number_range::NumberRangeOptions;
    
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::io::{BufWriter, Write};
    use std::path::{Path, PathBuf};
    use string_template_plus::{Render, Template};

    pub enum RenderFileContentsType {
        Include(PathBuf, String),
        Literal(String),
        Snippet(Template, Propagation),
    }

    pub struct RenderFileContents {
        contents: Vec<RenderFileContentsType>,
    }

    fn insert_till_now(
        lines: &mut String,
        batch: Option<Propagation>,
        filecontents: &mut RenderFileContents,
    ) -> Result<(), Error> {
        let p = if let Some(batch) = batch {
            RenderFileContentsType::Snippet(Template::parse_template(lines)?, batch)
        } else {
            RenderFileContentsType::Literal(lines.clone())
        };
        filecontents.contents.push(p);
        lines.clear();
        Ok(())
    }

    impl RenderFileContents {
        pub fn read_file(filename: &Path) -> Result<Self, Error> {
            let file = match File::open(filename) {
                Ok(f) => f,
                Err(e) => {
                    return Err(Error::msg(format!(
                        "Couldn't open input file: {:?}\n{:?}",
                        filename.to_string_lossy(),
                        e
                    )))
                }
            };
            let reader_lines = BufReader::new(file).lines();

            let mut snippet = false;
            let mut filecontents = RenderFileContents {
                contents: Vec::new(),
            };
            let mut lines = String::new();
            let mut batch: Option<Propagation> = None;
            for line in reader_lines {
                let l = line.unwrap();
                if l.contains("---8<---") {
                    insert_till_now(&mut lines, batch.clone(), &mut filecontents)?;
                    batch = if snippet {
                        // if in a snippet already, we're exiting
                        None
                    } else if let Some((_, s)) = l.split_once(':') {
                        let (_, prop) = crate::parser::functions::node_list_or_path(s)
                            .map_err(|e| anyhow::Error::msg(e.to_string()))
                            .context("Invalid Nodes Selection Method, use path or list of nodes")?;
                        Some(prop)
                    } else {
                        Some(Propagation::default())
                    };
                    snippet = !snippet;
                } else if l.contains("---include:") {
                    if snippet {
                        // todo let it include files globally, as well as inside snippets
                        return Err(Error::msg("Cannot have file in render snippet"));
                    }
                    insert_till_now(&mut lines, None, &mut filecontents)?;
                    let (_, fname) = l.split_once(':').unwrap();
                    let (fname, lines) = fname.split_once("::").unwrap_or((fname, ":"));
                    filecontents.contents.push(RenderFileContentsType::Include(
                        PathBuf::from(filename).parent().unwrap().join(fname.trim()),
                        lines.to_string(),
                    ))
                } else {
                    lines.push_str(&l);
                    lines.push('\n');
                }
            }
            if filecontents.contents.is_empty() {
                // if there is no ---8<--- in file, consider the whole
                // file as snippet
                batch = Some(Propagation::default());
            }
            if !lines.is_empty() {
                insert_till_now(&mut lines, batch, &mut filecontents)?;
            }
            Ok(filecontents)
        }

        fn snippet(templ: &str, batch: Propagation) -> Result<Self, Error> {
            Ok(Self {
                contents: vec![RenderFileContentsType::Snippet(
                    Template::parse_template(templ)?,
                    batch,
                )],
            })
        }

        pub fn print_render(
            &self,
            net: &mut Network,
            output: Option<PathBuf>,
        ) -> Result<(), Error> {
            let file = output.map(|f| File::create(f).unwrap());

            let mut writer: Box<dyn Write> = match file {
                Some(f) => Box::new(BufWriter::new(f)),
                None => Box::new(std::io::stdout()),
            };

            for part in &self.contents {
                match part {
                    RenderFileContentsType::Include(filename, lines) => {
                        let file = File::open(filename)
                            .with_context(|| format!("File {filename:?} not found"))?;
                        let reader_lines: Vec<String> = BufReader::new(file)
                            .lines()
                            .collect::<Result<Vec<String>, std::io::Error>>()?;
                        let lines = NumberRangeOptions::default()
                            .with_default_start(1)
                            .with_default_end(reader_lines.len())
                            .parse(lines)?;
                        for l in lines {
                            writeln!(writer, "{}", reader_lines[l - 1])?;
                        }
                    }
                    RenderFileContentsType::Literal(s) => write!(writer, "{}", s)?,
                    RenderFileContentsType::Snippet(templ, prop) => {
                        for node in net.nodes_propagation(&prop) {
                            write!(writer, "{}", node.lock().render(&templ)?)?;
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

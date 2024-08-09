use crate::node::Node;
use anyhow::Context;
use itertools::Itertools;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::str::FromStr;
use string_template_plus::Template;

/// Formats to export the table into
pub enum TableFormat {
    LaTeX,
    Delimited(char),
    JSON,
}

/// Tables are data types with headers and the value template. Tables
/// can be rendered/exported into CSV, JSON, and LaTeX format. Other
/// formats can be added later. Although tables are not exposed to the
/// plugin system, functions to export different table formats can be
/// written as a network function.
///
/// A Table file contains the table definition with each line with
/// [`Column`] definition for one column.
///
/// A sample Table file showing two columns, left aligned name for
/// station in title case, and right aligned 7Q10 column with float
/// value of 2 digits after decimal:
///
///     <Name: {_stn:case(title)}
///     >7Q10: {nat_7q10:f(2)}
///
/// The template system that tables use is the same as the one used
/// throughout the program (refer [`string_template_plus::Template`]).
pub struct Table {
    columns: Vec<Column>,
}

impl Table {
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file = File::open(path.as_ref())?;
        let reader_lines = BufReader::new(file).lines();
        Ok(Self {
            columns: reader_lines
                .map(|line| line.unwrap().trim().to_string())
                .filter(|line| !line.starts_with('#') && !line.is_empty())
                .map(|l| Column::from_str(&l))
                .collect::<Result<Vec<Column>, anyhow::Error>>()?,
        })
    }

    pub fn to_file<P: AsRef<Path>>(
        &self,
        nodes: Vec<&Node>,
        path: P,
        format: TableFormat,
    ) -> anyhow::Result<()> {
        match format {
            TableFormat::Delimited(sep) => self.to_delimited(nodes, path, sep),
            TableFormat::JSON => self.to_json(nodes, path),
            TableFormat::LaTeX => self.to_latex(nodes, path),
        }
    }

    pub fn to_delimited<P: AsRef<Path>>(
        &self,
        nodes: Vec<&Node>,
        path: P,
        sep: char,
    ) -> anyhow::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writeln!(writer, "{}", self.headers().join(&sep.to_string()))?;
        for node in nodes {
            writeln!(
                writer,
                "{}",
                self.templates()
                    .map(|t| node.borrow().render(t))
                    .collect::<anyhow::Result<Vec<String>>>()?
                    .join(&sep.to_string())
            )?;
        }
        Ok(())
    }
    pub fn to_latex<P: AsRef<Path>>(&self, nodes: Vec<&Node>, path: P) -> anyhow::Result<()> {
        todo!()
    }
    pub fn to_json<P: AsRef<Path>>(&self, nodes: Vec<&Node>, path: P) -> anyhow::Result<()> {
        todo!()
    }

    pub fn headers(&self) -> impl Iterator<Item = &str> {
        self.columns.iter().map(|c| c.header.as_str())
    }
    pub fn templates(&self) -> impl Iterator<Item = &Template> {
        self.columns.iter().map(|c| &c.template)
    }
}

pub struct Column {
    align: ColumnAlign,
    header: String,
    template: Template,
}

#[derive(Default)]
pub enum ColumnAlign {
    Left,
    #[default]
    Center,
    Right,
}

impl FromStr for Column {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (head, templ) = s
            .split_once(':')
            .context("Header should have a template followed")?;
        let (align, head) = match head.chars().next().context("Empty Template Not allowed")? {
            '<' => (ColumnAlign::Left, &head[1..]),
            '>' => (ColumnAlign::Right, &head[1..]),
            _ => (ColumnAlign::Center, head),
        };

        Ok(Self {
            align,
            header: head.to_string(),
            template: Template::parse_template(templ.trim())?,
        })
    }
}

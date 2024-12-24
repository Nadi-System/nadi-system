use abi_stable::{
    std_types::{RString, RVec},
    StableAbi,
};
use string_template_plus::Template;

use crate::network::Network;

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub enum ColumnAlign {
    Left,
    Right,
    #[default]
    Center,
}

impl std::fmt::Display for ColumnAlign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Left => '<',
                Self::Right => '>',
                Self::Center => '^',
            }
        )
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub struct Column {
    pub align: ColumnAlign,
    pub header: RString,
    pub template: RString,
}

impl Column {
    pub fn new(header: &str, template: &str, align: Option<ColumnAlign>) -> Self {
        Self {
            align: align.unwrap_or_default(),
            header: header.into(),
            template: template.into(),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub struct Table {
    pub columns: RVec<Column>,
}


impl Table {
    pub fn render_contents(
        &self,
        net: &Network,
        conn: bool,
    ) -> Result<Vec<Vec<String>>, anyhow::Error> {
        let templates = self
            .columns
            .iter()
            .map(|c| Template::parse_template(&c.template))
            .collect::<Result<Vec<Template>, anyhow::Error>>()?;

        if conn {
            net.nodes()
                .zip(net.connections_utf8())
                .map(|(n, c)| {
                    let n = n.lock();
                    let mut row = templates
                        .iter()
                        .map(|t| n.render(t))
                        .collect::<Result<Vec<String>, anyhow::Error>>()?;
                    row.insert(0, c);
                    Ok(row)
                })
                .collect()
        } else {
            net.nodes()
                .map(|n| {
                    let n = n.lock();
                    let row = templates
                        .iter()
                        .map(|t| n.render(t))
                        .collect::<Result<Vec<String>, anyhow::Error>>()?;
                    Ok(row)
                })
                .collect()
        }
    }

    pub fn render_markdown(&self, net: &Network, conn: Option<String>) -> anyhow::Result<String> {
        let mut headers: Vec<&str> = self.columns.iter().map(|c| c.header.as_str()).collect();
        if let Some(c) = &conn {
            headers.insert(0, c);
        }
        let mut alignments: Vec<&ColumnAlign> = self.columns.iter().map(|c| &c.align).collect();
        if conn.is_some() {
            // conn needs to be left align for the ascii diagram to work
            alignments.insert(0, &ColumnAlign::Left);
        }
        let contents = self.render_contents(net, conn.is_some())?;
        Ok(contents_2_md(&headers, &alignments, contents))
    }
}

pub fn contents_2_md(
    headers: &[&str],
    alignments: &[&ColumnAlign],
    contents: Vec<Vec<String>>,
) -> String {
    let col_widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            contents
                .iter()
                .map(|row| row[i].len())
                .chain([h.len()])
                .max()
                .unwrap_or(1)
        })
        .collect();
    let mut table = String::new();
    table.push('|');
    for ((c, w), a) in headers.iter().zip(&col_widths).zip(alignments) {
        table.push_str(&align_fmt_fn(c, a, w));
        table.push('|');
    }
    table.push('\n');
    table.push('|');
    for (w, a) in col_widths.iter().zip(alignments) {
        let (pre, post) = match a {
            ColumnAlign::Left => (':', '-'),
            ColumnAlign::Right => ('-', ':'),
            ColumnAlign::Center => (':', ':'),
        };
        table.push_str(&format!("{pre}{:->1$}{post}|", "", w));
    }
    table.push('\n');
    for row in contents {
        table.push('|');
        for ((c, w), a) in row.iter().zip(&col_widths).zip(alignments) {
            table.push_str(&align_fmt_fn(c, a, w));
            table.push('|');
        }
        table.push('\n');
    }
    table
}

fn align_fmt_fn(col: &str, align: &ColumnAlign, width: &usize) -> String {
    match align {
        ColumnAlign::Left => format!(" {:<1$} ", col, width),
        ColumnAlign::Right => format!(" {:>1$} ", col, width),
        ColumnAlign::Center => format!(" {:^1$} ", col, width),
    }
}

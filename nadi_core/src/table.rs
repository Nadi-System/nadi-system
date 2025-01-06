use crate::{
    attrs::{Attribute, FromAttribute},
    network::Network,
};
use abi_stable::{
    std_types::{RString, RVec},
    StableAbi,
};
use std::str::FromStr;
use string_template_plus::Template;

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

impl FromStr for ColumnAlign {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "left" | "<" => Ok(ColumnAlign::Left),
            "right" | ">" => Ok(ColumnAlign::Right),
            "center" | "^" => Ok(ColumnAlign::Center),
            a => Err(format!("Invalid Column Align: {a}")),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub struct Column {
    pub align: ColumnAlign,
    pub header: RString,
    pub template: RString,
}

#[cfg(feature = "parser")]
impl FromStr for Column {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match crate::parser::table::column(&s).map_err(|e| e.to_string())? {
            ("", c) => Ok(c),
            (r, _) => Err(format!("Remainder from parsing column definition: {r}")),
        }
    }
}

impl FromAttribute for Column {
    fn from_attr(value: &Attribute) -> Option<Self> {
        FromAttribute::try_from_attr(value).ok()
    }

    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
        match value {
            Attribute::Table(tab) => {
                let header = tab
                    .get("header")
                    .and_then(|h| String::from_attr(h))
                    .ok_or("Invalid or no header".to_string())?;
                let align = tab
                    .get("align")
                    .and_then(|h| String::from_attr(h))
                    .ok_or("Invalid or no align".to_string())?;
                let align = ColumnAlign::from_str(&align)?;
                let templ = tab
                    .get("template")
                    .and_then(|h| String::from_attr(h))
                    .ok_or("Invalid or no template".to_string())?;
                Ok(Column::new(&header, &templ, Some(align)))
            }
            Attribute::Array(cols) => match cols.len() {
                #[cfg(feature = "parser")]
                1 => {
                    let col = String::try_from_attr(&cols[0])?;
                    Column::from_str(&col)
                }
                2 => {
                    let header = String::try_from_attr(&cols[0])?;
                    let templ = String::try_from_attr(&cols[1])?;
                    Ok(Column::new(&header, &templ, None))
                }
                3 => {
                    let header = String::try_from_attr(&cols[0])?;
                    let align = ColumnAlign::from_str(String::try_from_attr(&cols[1])?.as_str())?;
                    let templ = String::try_from_attr(&cols[2])?;
                    Ok(Column::new(&header, &templ, Some(align)))
                }
                x => Err(format!("Column can be 1,2 or 3 string array not {x}")),
            },
            #[cfg(feature = "parser")]
            Attribute::String(s) => Column::from_str(&s),
            _ => Err(format!(
                "Incorrect Type: got {} instead of Table/Array or String",
                value.type_name()
            )),
        }
    }
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

impl FromAttribute for Table {
    fn from_attr(value: &Attribute) -> Option<Self> {
        FromAttribute::try_from_attr(value).ok()
    }

    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
        let mut cols = vec![];
        match value {
            Attribute::Table(tab) => {
                for kv in tab {
                    let (align, templ) = match String::from_attr(kv.1) {
                        Some(s) => (None, s),
                        None => match <(String, String)>::from_attr(kv.1) {
                            Some((a, s)) => (Some(ColumnAlign::from_str(&a)?), s),
                            None => {
                                return Err(format!(
                                    "Incorrect Type: got {} instead of String or [String, String]",
                                    value.type_name()
                                ));
                            }
                        },
                    };
                    cols.push(Column::new(&kv.0, &templ, align));
                }
            }
            Attribute::Array(ar) => {
                for c in ar {
                    cols.push(Column::try_from_attr(c)?);
                }
            }
            #[cfg(feature = "parser")]
            Attribute::String(s) => return Table::from_str(s).map_err(|e| e.to_string()),
            _ => {
                return Err(format!(
                    "Incorrect Type: got {} instead of Table/Array or String",
                    value.type_name()
                ))
            }
        }
        Ok(Table {
            columns: cols.into(),
        })
    }
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

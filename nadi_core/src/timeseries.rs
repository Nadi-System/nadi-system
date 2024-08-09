use anyhow::Context;
use chrono::{NaiveDate, NaiveDateTime};
use itertools::Itertools;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

macro_rules! reterr {
    ($msg: expr) => {
        return Err(anyhow::Error::msg($msg))
    };
}

/// Timeseries of values, at regular interval. Can support integers,
/// floats and strings as of now. More data types can be added. The
/// timeseries are lazy loaded, that is, they are not actually read from
/// the CSV files till they are accessed.
///
/// Once timeseries is tried to be accessed [`TimeSeriesDefinition`]
/// is converted into [`LoadedTimeSeries`]
///
/// For timeseries that are not in a simple CSV format. The path to the
/// timeseries can be provided as a node attribute and plugin functions
/// can be written to use that path to load the timeseries into the node
/// attribute.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TimeSeriesDefinition {
    path: PathBuf,
    column: Option<String>,
    timecol: Option<String>,
    dtfmt: Option<String>,
    start: Option<NaiveDateTime>,
    #[serde(default)]
    #[serde(with = "humantime_serde")]
    timestep: Option<Duration>,
    dtype: String,
}

impl TimeSeriesDefinition {
    pub fn print_csv(&self) -> anyhow::Result<()> {
        let mut file = csv::Reader::from_path(&self.path)?;
        let headerrow = file.headers()?.iter().map(|f| f.to_string()).join("\t");
        println!("{}", headerrow);
        for rec in file.records() {
            let row = rec?.iter().map(|f| f.to_string()).join("\t");
            println!("{}", row);
        }
        Ok(())
    }

    pub fn resolve_path(&mut self, parent_dir: &Path) {
        if self.path.is_relative() {
            self.path = parent_dir.join(&self.path);
        }
    }

    pub fn load(&self) -> anyhow::Result<LoadedTimeSeries> {
        let mut file = csv::Reader::from_path(&self.path)?;
        let headers: HashMap<String, usize> = file
            .headers()?
            .iter()
            .enumerate()
            .map(|(i, f)| (f.to_string(), i))
            .collect();
        let col_ind: usize = if let Some(name) = &self.column {
            *headers
                .get(name)
                .context(format!("No columns with given name {name}"))?
        } else if headers.is_empty() {
            reterr!("No columns in the csv file")
        } else {
            0
        };
        let timecol = match &self.timecol {
            Some(c) => Some(
                headers
                    .get(c)
                    .context(format!("No such columns timecol={c}"))?,
            ),
            None => None,
        };

        let mut timevals: [NaiveDateTime; 2] = [NaiveDateTime::default(), NaiveDateTime::default()];

        let values: Vec<String> = file
            .records()
            .enumerate()
            .map(|(i, r)| -> anyhow::Result<String> {
                r.map_err(|e| e.into()).and_then(|r| {
                    if i < 2 {
                        if let Some(tc) = timecol {
                            let tstr = r.get(*tc).unwrap();
                            timevals[i] = if let Some(fmt) = &self.dtfmt {
                                NaiveDateTime::parse_from_str(tstr, fmt).or_else(|_| {
                                    NaiveDate::parse_from_str(tstr, fmt)
                                        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                                })?
                            } else {
                                NaiveDateTime::from_str(tstr).or_else(|_| {
                                    NaiveDate::from_str(tstr)
                                        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                                })?
                            }
                        }
                    }
                    r.get(col_ind)
                        .context(format!("No column {col_ind}"))
                        .map(|s| s.to_string())
                })
            })
            .try_collect()?;

        let (start, timestep) = match &self.timecol {
            Some(_) => (
                timevals[0],
                Duration::from_millis((timevals[1] - timevals[0]).num_milliseconds() as u64),
            ),
            None => (
                self.start.context("No start and no timecolumn")?,
                self.timestep.context("No timestep and no timecolumn")?,
            ),
        };

        let values = match self.dtype.as_str() {
            "float" => TimeSeriesData::Floats(
                values
                    .iter()
                    .map(|v| v.parse::<f64>().unwrap_or(f64::NAN))
                    .collect(),
            ),
            "int" => {
                let data: Vec<Option<i64>> = values.iter().map(|v| v.parse::<i64>().ok()).collect();
                TimeSeriesData::Ints(
                    data.iter().map(|v| v.unwrap_or(0)).collect(),
                    data.iter().map(|v| v.is_some()).collect(),
                )
            }
            "str" => TimeSeriesData::Strings(values),
            _ => reterr!("Unknown Data Type"),
        };

        Ok(LoadedTimeSeries {
            start,
            timestep,
            values,
        })
    }
}

/// Enum for different types of timeseries data
#[derive(Clone)]
pub enum TimeSeriesData {
    Strings(Vec<String>),
    Floats(Vec<f64>),
    /// strings can be empty and floats can be f64::NAN, so ints have
    /// mask for valid data
    Ints(Vec<i64>, Vec<bool>),
}

impl TimeSeriesData {
    pub fn strings(s: Vec<String>) -> Self {
        TimeSeriesData::Strings(s)
    }
    pub fn floats(s: Vec<f64>) -> Self {
        TimeSeriesData::Floats(s)
    }
    pub fn ints(s: Vec<i64>, m: Vec<bool>) -> Self {
        TimeSeriesData::Ints(s, m)
    }
}

/// Timeseries values
#[derive(Clone)]
pub struct LoadedTimeSeries {
    start: NaiveDateTime,
    timestep: Duration,
    values: TimeSeriesData,
}

impl LoadedTimeSeries {
    pub fn new(start: NaiveDateTime, timestep: Duration, values: TimeSeriesData) -> Self {
        Self {
            start,
            timestep,
            values,
        }
    }

    pub fn like(other: &Self, values: TimeSeriesData) -> Self {
        Self {
            start: other.start,
            timestep: other.timestep,
            values,
        }
    }
}

pub trait TimeSeries {
    fn start(&self) -> &NaiveDateTime;
    fn timestep(&self) -> &Duration;
    fn length(&self) -> usize;
    fn values_str(&self) -> Option<Vec<&str>>;
    fn values_int(&self) -> Option<(Vec<&i64>, Vec<bool>)>;
    fn values_float(&self) -> Option<Vec<&f64>>;
}

impl TimeSeries for LoadedTimeSeries {
    fn start(&self) -> &NaiveDateTime {
        &self.start
    }
    fn timestep(&self) -> &Duration {
        &self.timestep
    }
    fn length(&self) -> usize {
        match &self.values {
            TimeSeriesData::Strings(v) => v.len(),
            TimeSeriesData::Floats(v) => v.len(),
            TimeSeriesData::Ints(v, _) => v.len(),
        }
    }
    fn values_str(&self) -> Option<Vec<&str>> {
        match &self.values {
            TimeSeriesData::Strings(s) => Some(s.iter().map(|s| s.as_str()).collect()),
            _ => None,
        }
    }
    fn values_int(&self) -> Option<(Vec<&i64>, Vec<bool>)> {
        match &self.values {
            TimeSeriesData::Ints(i, b) => Some((i.iter().collect(), b.clone())),
            _ => None,
        }
    }
    fn values_float(&self) -> Option<Vec<&f64>> {
        match &self.values {
            TimeSeriesData::Floats(f) => Some(f.iter().collect()),
            _ => None,
        }
    }
}

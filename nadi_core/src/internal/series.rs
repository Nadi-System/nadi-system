use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod series {
    use crate::prelude::*;
    use crate::timeseries::{CompleteSeries, Series};
    use nadi_plugin::node_func;

    /// Number of series in the node
    #[node_func]
    fn sr_count(node: &NodeInner) -> usize {
        node.series_map().len()
    }

    /// List all series in the node
    #[node_func]
    fn sr_list(node: &NodeInner) -> Vec<String> {
        node.series_map().keys().map(|s| s.to_string()).collect()
    }

    /// Type name of the series
    #[node_func(safe = false)]
    fn sr_dtype(
        node: &NodeInner,
        /// Name of the series
        name: &str,
        /// Do not error if series does't exist
        safe: bool,
    ) -> Result<Option<String>, String> {
        match node.try_series(name) {
            Ok(s) => Ok(Some(s.type_name().to_string())),
            Err(_) if safe => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Length of the series
    #[node_func(safe = false, countna = true)]
    fn sr_len(
        node: &NodeInner,
        /// Name of the series
        name: &str,
        /// Do not error if series does't exist
        safe: bool,
        /// count na values in the length
        countna: bool,
    ) -> Result<Option<usize>, String> {
        match node.try_series(name) {
            Ok(s) => Ok(Some(if countna { s.len() } else { s.len_valid() })),
            Err(_) if safe => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Mean of a series values
    #[node_func]
    fn sr_mean(
        node: &NodeInner,
        /// Name of the series
        name: &str,
    ) -> Result<f64, String> {
        let sr = node.try_series(name)?;
        match sr {
            Series::Complete(CompleteSeries::Floats(ref vals)) => {
                Ok(vals.iter().sum::<f64>() / vals.len() as f64)
            }
            Series::Complete(CompleteSeries::Integers(ref vals)) => {
                Ok(vals.iter().sum::<i64>() as f64 / vals.len() as f64)
            }
            Series::Complete(CompleteSeries::Booleans(ref vals)) => {
                Ok(vals.iter().filter(|v| **v).count() as f64 / vals.len() as f64)
            }
            s => Err(format!(
                "Incorrect Type: Mean cannot be calculated for series of type `{}`",
                s.type_name(),
            )),
        }
    }

    /// Sum of the series values
    #[node_func]
    fn sr_sum(
        node: &NodeInner,
        /// Name of the series
        name: &str,
    ) -> Result<Attribute, String> {
        let sr = node.try_series(name)?;
        match sr {
            Series::Complete(CompleteSeries::Floats(ref vals)) => {
                Ok(vals.iter().sum::<f64>().into())
            }
            Series::Complete(CompleteSeries::Integers(ref vals)) => {
                Ok(vals.iter().sum::<i64>().into())
            }
            Series::Complete(CompleteSeries::Booleans(ref vals)) => {
                Ok(vals.iter().filter(|v| **v).count().into())
            }
            s => Err(format!(
                "Incorrect Type: Mean cannot be calculated for series of type `{}`",
                s.type_name(),
            )),
        }
    }

    /// set the following series to the node
    #[node_func]
    fn set_series(
        node: &mut NodeInner,
        /// Name of the series to save as
        name: &str,
        /// Argument to convert to series
        value: Attribute,
        /// type
        dtype: &str,
    ) -> Result<(), String> {
        let val = Series::from_attr(&value, dtype)?;
        node.set_series(name, val);
        Ok(())
    }

    #[node_func]
    /// Delete the series with the given name
    fn sr_delete(
        node: &mut NodeInner,
        /// Name of the series to delete from this node
        name: &str,
    ) -> bool {
        node.del_series(name).is_some()
    }

    /// Sort an series
    #[node_func]
    fn sr_sort(
        node: &mut NodeInner,
        /// Name of the series
        name: &str,
    ) -> Result<(), String> {
        let sr = node.try_series_mut(name)?;
        match sr {
            Series::Complete(CompleteSeries::Floats(ref mut vals)) => {
                let _: () = vals.sort_by(f64::total_cmp);
                Ok(())
            }
            Series::Complete(CompleteSeries::Integers(ref mut vals)) => {
                let _: () = vals.sort();
                Ok(())
            }
            Series::Complete(CompleteSeries::Booleans(ref mut vals)) => {
                let _: () = vals.sort();
                Ok(())
            }
            Series::Complete(CompleteSeries::Strings(ref mut vals)) => {
                let _: () = vals.sort();
                Ok(())
            }
            Series::Complete(CompleteSeries::Dates(ref mut vals)) => {
                let _: () = vals.sort();
                Ok(())
            }
            Series::Complete(CompleteSeries::Times(ref mut vals)) => {
                let _: () = vals.sort();
                Ok(())
            }
            s => Err(format!(
                "Incorrect Type: Mean cannot be calculated for series of type `{}`",
                s.type_name(),
            )),
        }
    }

    /// get nth member of a series
    #[node_func]
    fn sr_get(
        node: &NodeInner,
        /// Name of the series
        name: &str,
        /// index to get
        ind: usize,
    ) -> Result<Option<Attribute>, String> {
        let sr = node.try_series(name)?;
        match sr {
            Series::Complete(CompleteSeries::Floats(ref vals)) => {
                Ok(vals.get(ind).cloned().map(Attribute::Float))
            }
            Series::Complete(CompleteSeries::Integers(ref vals)) => {
                Ok(vals.get(ind).cloned().map(Attribute::Integer))
            }
            Series::Complete(CompleteSeries::Strings(ref vals)) => {
                Ok(vals.get(ind).cloned().map(Attribute::String))
            }
            Series::Complete(CompleteSeries::Dates(ref vals)) => {
                Ok(vals.get(ind).cloned().map(Attribute::Date))
            }
            Series::Complete(CompleteSeries::Times(ref vals)) => {
                Ok(vals.get(ind).cloned().map(Attribute::Time))
            }
            Series::Complete(CompleteSeries::DateTimes(ref vals)) => {
                Ok(vals.get(ind).cloned().map(Attribute::DateTime))
            }
            Series::Complete(CompleteSeries::Attributes(ref vals)) => Ok(vals.get(ind).cloned()),
            _ => Err("Incorrect Type: Masked Series not supported for Nth".to_string()),
        }
    }

    /// Make an array from the series if it's complete
    #[node_func(safe = false)]
    fn sr_to_array(
        node: &NodeInner,
        /// Name of the series
        name: &str,
        /// Do not error if series does't exist or there are gaps
        safe: bool,
    ) -> Result<Option<Attribute>, String> {
        match node.try_series(name) {
            Ok(Series::Complete(s)) => Ok(Some(Attribute::Array(s.clone().to_attributes().into()))),
            Ok(Series::Masked(..)) => {
                if safe {
                    Ok(None)
                } else {
                    Err("The Series is not Complete".into())
                }
            }
            Err(_) if safe => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Fill the series with a value
    #[node_func(inplace = true)]
    fn sr_fill(
        node: &mut NodeInner,
        /// Name of the series
        name: &str,
        /// Value to fill the series with: needs to be same type
        value: Attribute,
        /// Fill the series inplace
        inplace: bool,
        /// New name for the series, adds `_filled` by default; ignored for inplace
        newname: Option<String>,
    ) -> Result<(), String> {
        if inplace {
            node.fill_series(name, value)
        } else {
            let ser = node.try_series(name)?.clone();
            let newname = newname.unwrap_or_else(|| format!("{name}_filled"));
            node.set_series(&newname, ser.fill_gaps(value).map_err(|e| e.to_string())?);
            Ok(())
        }
    }

    /// Get maximum value of the series
    #[node_func]
    fn sr_max(
        node: &mut NodeInner,
        /// Name of the series
        name: &str,
    ) -> Result<Option<Attribute>, String> {
        Ok(node.try_series(name)?.maximum()?)
    }

    /// Get minimum value of the series
    #[node_func]
    fn sr_min(
        node: &mut NodeInner,
        /// Name of the series
        name: &str,
    ) -> Result<Option<Attribute>, String> {
        Ok(node.try_series(name)?.minimum()?)
    }
}

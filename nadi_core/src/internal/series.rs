use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod series {
    use crate::prelude::*;
    use crate::timeseries::{FromSeries, Series};
    use nadi_plugin::node_func;

    /// Number of series in the node
    #[node_func]
    fn sr_count(node: &mut NodeInner) -> usize {
        node.series_map().len()
    }

    /// List all series in the node
    #[node_func]
    fn sr_list(node: &mut NodeInner) -> Vec<String> {
        node.series_map().keys().map(|s| s.to_string()).collect()
    }

    /// Type name of the series
    #[node_func(safe = false)]
    fn sr_dtype(
        node: &mut NodeInner,
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
    #[node_func(safe = false)]
    fn sr_len(
        node: &mut NodeInner,
        /// Name of the series
        name: &str,
        /// Do not error if series does't exist
        safe: bool,
    ) -> Result<Option<usize>, String> {
        match node.try_series(name) {
            Ok(s) => Ok(Some(s.len())),
            Err(_) if safe => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Type name of the series
    #[node_func]
    fn sr_mean(
        node: &mut NodeInner,
        /// Name of the series
        name: &str,
    ) -> Result<f64, String> {
        let sr = node.try_series(name)?;
        match sr {
            Series::Floats(ref vals) => Ok(vals.iter().sum::<f64>() / vals.len() as f64),
            Series::Integers(ref vals) => Ok(vals.iter().sum::<i64>() as f64 / vals.len() as f64),
            Series::Booleans(ref vals) => {
                Ok(vals.iter().filter(|v| **v).count() as f64 / vals.len() as f64)
            }
            s => Err(format!(
                "Incorrect Type: Mean cannot be calculated for series of type `{}`",
                s.type_name(),
            )),
        }
    }

    /// Sum of the series
    #[node_func]
    fn sr_sum(
        node: &mut NodeInner,
        /// Name of the series
        name: &str,
    ) -> Result<Attribute, String> {
        let sr = node.try_series(name)?;
        match sr {
            Series::Floats(ref vals) => Ok(vals.iter().sum::<f64>().into()),
            Series::Integers(ref vals) => Ok(vals.iter().sum::<i64>().into()),
            Series::Booleans(ref vals) => Ok(vals.iter().filter(|v| **v).count().into()),
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

    /// Make an array from the series
    #[node_func(safe = false)]
    fn sr_to_array(
        node: &mut NodeInner,
        /// Name of the series
        name: &str,
        /// Do not error if series does't exist
        safe: bool,
    ) -> Result<Option<Attribute>, String> {
        match node.try_series(name) {
            Ok(s) => Ok(Some(Attribute::Array(s.clone().to_attributes().into()))),
            Err(_) if safe => Ok(None),
            Err(e) => Err(e),
        }
    }
}

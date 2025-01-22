use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod core {
    use crate::prelude::*;
    use nadi_plugin::env_func;

    /// make a float from value
    #[env_func(parse = true)]
    fn float(
        /// Argument to convert to float
        value: Attribute,
        /// parse string to float
        parse: bool,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::String(s) if parse => s.parse::<f64>().map_err(|e| e.to_string())?,
            _ => f64::try_from_attr_relaxed(&value)?,
        };
        Ok(Attribute::Float(val))
    }

    /// make a string from value
    #[env_func(quote = false)]
    fn str(
        /// Argument to convert to float
        value: Attribute,
        /// quote it if it's literal string
        quote: bool,
    ) -> Result<Attribute, String> {
        let val = if quote {
            value.to_string()
        } else {
            String::try_from_attr_relaxed(&value)?
        };
        Ok(Attribute::String(val.into()))
    }

    /// make an int from the value
    #[env_func(parse = true, round = true, strfloat = false)]
    fn int(
        /// Argument to convert to int
        value: Attribute,
        /// parse string to int
        parse: bool,
        /// round float into integer
        round: bool,
        /// parse string first as float before converting to int
        strfloat: bool,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::String(s) if strfloat => {
                s.parse::<f64>().map_err(|e| e.to_string())?.round() as i64
            }
            Attribute::String(s) if parse => s.parse::<i64>().map_err(|e| e.to_string())?,
            Attribute::Float(f) if round => f.round() as i64,
            ref v => i64::try_from_attr_relaxed(v)?,
        };
        Ok(Attribute::Integer(val))
    }

    /// make an array from the arguments
    #[env_func]
    fn array(
        /// List of attributes
        #[args]
        attributes: &[Attribute],
    ) -> Attribute {
        Attribute::Array(attributes.to_vec().into())
    }

    /// make an array from the arguments
    #[env_func]
    fn attrmap(
        /// name and values of attributes
        #[kwargs]
        attributes: &AttrMap,
    ) -> Attribute {
        Attribute::Table(attributes.clone())
    }
}

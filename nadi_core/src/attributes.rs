use toml::Value;

pub trait AsValue {
    fn into_toml_string(self) -> Option<String>;
    fn into_string(self) -> Option<String>;
    fn into_loose_int(self) -> Option<i64>;
    fn into_loose_float(self) -> Option<f64>;
    fn into_loose_bool(self) -> bool;
}

impl AsValue for Value {
    fn into_toml_string(self) -> Option<String> {
        Some(self.to_string())
    }
    fn into_string(self) -> Option<String> {
        match self {
            toml::Value::String(v) => Some(v),
            _ => None,
        }
    }
    fn into_loose_int(self) -> Option<i64> {
        match self {
            toml::Value::Integer(val) => Some(val),
            toml::Value::Boolean(val) => Some(if val { 1 } else { 0 }),
            toml::Value::Float(val) => Some(val.floor() as i64),
            toml::Value::String(val) => val.parse::<i64>().ok(),
            _ => None,
        }
    }
    fn into_loose_float(self) -> Option<f64> {
        match self {
            toml::Value::Float(val) => Some(val),
            toml::Value::Integer(val) => Some(val as f64),
            toml::Value::Boolean(val) => Some(if val { 1.0 } else { 0.0 }),
            toml::Value::String(val) => val.parse::<f64>().ok(),
            _ => None,
        }
    }
    fn into_loose_bool(self) -> bool {
        match self {
            toml::Value::Boolean(val) => val,
            toml::Value::Integer(val) => val != 0,
            toml::Value::Float(val) => val != 0.0,
            _ => true,
        }
    }
}

impl AsValue for Option<Value> {
    fn into_toml_string(self) -> Option<String> {
        self.and_then(AsValue::into_toml_string)
    }
    fn into_string(self) -> Option<String> {
        self.and_then(AsValue::into_string)
    }
    fn into_loose_int(self) -> Option<i64> {
        self.and_then(AsValue::into_loose_int)
    }
    fn into_loose_float(self) -> Option<f64> {
        self.and_then(AsValue::into_loose_float)
    }
    fn into_loose_bool(self) -> bool {
        self.map(AsValue::into_loose_bool).unwrap_or(false)
    }
}

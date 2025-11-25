use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod math {
    use nadi_plugin::env_func;

    /// Integer power
    ///
    /// ```task
    /// env assert_eq(powi(10.0, 2), 100.0)
    /// ```
    #[env_func]
    fn powi(
        /// base value
        #[relaxed]
        value: f64,
        power: i64,
    ) -> f64 {
        value.powi(power as i32)
    }

    /// Float power
    ///
    /// ```task
    /// env assert_eq(powf(100.0, 0.5), 10.0)
    /// ```
    #[env_func]
    fn powf(
        /// base value
        #[relaxed]
        value: f64,
        power: f64,
    ) -> f64 {
        value.powf(power)
    }

    /// Exponential
    ///
    /// ```task
    /// env assert_eq(log(exp(5.0)), 5.0)
    /// ```
    #[env_func]
    fn exp(#[relaxed] value: f64) -> f64 {
        value.exp()
    }

    /// Square Root
    /// ```task
    /// env assert_eq(sqrt(25.0), 5.0)
    /// ```
    #[env_func]
    fn sqrt(#[relaxed] value: f64) -> f64 {
        value.sqrt()
    }

    /// Logarithm of a value, natural if base not given
    ///
    /// ```task
    /// env assert_eq(log(exp(2.0)), 2.0)
    /// env assert_eq(log(2.0, 2.0), 1.0)
    /// ```
    #[env_func]
    fn log(#[relaxed] value: f64, base: Option<f64>) -> f64 {
        if let Some(b) = base {
            value.log(b)
        } else {
            value.ln()
        }
    }
}

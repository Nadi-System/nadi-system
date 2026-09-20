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

    /// round a float value
    ///
    /// ```task
    /// env assert_eq(round(1.2), 1.0)
    /// env assert_eq(round(1.234, 1), 1.2)
    /// env assert_eq(round(1.8), 2.0)
    /// ```
    #[env_func(sig = 0u32)]
    fn round(#[relaxed] num: f64, sig: u32) -> f64 {
        if sig == 0 {
            num.round()
        } else {
            let factor = 10i64.pow(sig) as f64;
            (num * factor).round() / factor
        }
    }

    /// round a float to the lower integer
    ///
    /// ```task
    /// env assert_eq(floor(1.2), 1.0)
    /// env assert_eq(floor(1.8), 1.0)
    /// env assert_eq(floor(1.891, 1), 1.8)
    /// ```
    #[env_func(sig = 0u32)]
    fn floor(#[relaxed] num: f64, sig: u32) -> f64 {
        if sig == 0 {
            num.floor()
        } else {
            let factor = 10i64.pow(sig) as f64;
            (num * factor).floor() / factor
        }
    }

    /// round a float to the higher integer
    ///
    /// ```task
    /// env assert_eq(ceil(1.2), 2.0)
    /// env assert_eq(ceil(1.8), 2.0)
    /// ```
    #[env_func(sig = 0u32)]
    fn ceil(#[relaxed] num: f64, sig: u32) -> f64 {
        if sig == 0 {
            num.ceil()
        } else {
            let factor = 10i64.pow(sig) as f64;
            (num * factor).ceil() / factor
        }
    }

    /// constant PI
    ///
    /// ```task
    /// env assert_eq(round(pi(), 2), 3.14)
    /// env assert_eq(round(pi(), 4), 3.1416)
    /// ```
    #[env_func]
    fn pi() -> f64 {
        std::f64::consts::PI
    }

    /// Sine of a number in radians
    ///
    /// ```task
    /// env assert_eq(round(sin(pi()), 6), 0.0)
    /// env assert_eq(round(sin(30, true), 6), 0.5)
    /// ```
    #[env_func(degree = false)]
    fn sin(#[relaxed] value: f64, degree: bool) -> f64 {
        if degree {
            (value * std::f64::consts::PI / 180.0).sin()
        } else {
            value.sin()
        }
    }

    /// Cosine of a number in radians
    ///
    /// ```task
    /// env assert_eq(round(cos(0), 6), 1.0)
    /// env assert_eq(round(cos(60, true), 6), 0.5)
    /// ```
    #[env_func(degree = false)]
    fn cos(#[relaxed] value: f64, degree: bool) -> f64 {
        if degree {
            (value * std::f64::consts::PI / 180.0).cos()
        } else {
            value.cos()
        }
    }

    /// Tangent of a number in radians
    ///
    /// ```task
    /// env assert_eq(round(tan(pi() / 4), 6), 1.0)
    /// env assert_eq(round(tan(45, true), 6), 1.0)
    /// ```
    #[env_func(degree = false)]
    fn tan(#[relaxed] value: f64, degree: bool) -> f64 {
        if degree {
            (value * std::f64::consts::PI / 180.0).tan()
        } else {
            value.tan()
        }
    }
}

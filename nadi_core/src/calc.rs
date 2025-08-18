pub fn error(obs: &[f64], sim: &[f64], error: &str) -> Result<f64, String> {
    let err = match error {
        "rmse" => rmse(obs, sim),
        "nrmse" => nrmse(obs, sim),
        "abserr" => abserr(obs, sim),
        "nse" => nse(obs, sim),
        _ => return Err(String::from("Unknown Error type")),
    };
    Ok(err)
}

pub fn rmse(obs: &[f64], sim: &[f64]) -> f64 {
    let mut count: usize = 0;
    let mut sum_e: f64 = 0.0;
    obs.iter().zip(sim).for_each(|(kd, cd)| {
        if !kd.is_nan() && !cd.is_nan() {
            sum_e += (cd - kd).powi(2);
            count += 1;
        }
    });
    // not normalized
    (sum_e / count as f64).sqrt()
}

pub fn nrmse(obs: &[f64], sim: &[f64]) -> f64 {
    let mut total: f64 = 0.0;
    let mut count: usize = 0;
    let mut sum_e: f64 = 0.0;
    obs.iter().zip(sim).for_each(|(kd, cd)| {
        if !kd.is_nan() && !cd.is_nan() {
            sum_e += (cd - kd).powi(2);
            total += kd;
            count += 1;
        }
    });
    // normalized
    (sum_e / count as f64).sqrt() / (total / count as f64)
}

pub fn abserr(obs: &[f64], sim: &[f64]) -> f64 {
    let d = obs.iter().zip(sim).filter_map(|(kd, cd)| {
        if kd.is_nan() || cd.is_nan() {
            None
        } else {
            Some((cd - kd).abs())
        }
    });
    d.clone().sum::<f64>() / (d.count() as f64)
}

pub fn nse(obs: &[f64], sim: &[f64]) -> f64 {
    let non_nan = obs.iter().filter(|q| !q.is_nan());
    let mean = non_nan.clone().sum::<f64>() / (non_nan.count() as f64);
    let mut mse: f64 = 0.0;
    let mut denom: f64 = 0.0;
    obs.iter().zip(sim).for_each(|(kd, cd)| {
        if !kd.is_nan() && !cd.is_nan() {
            mse += (cd - kd) * (cd - kd);
            denom += (mean - kd) * (mean - kd)
        }
    });
    1.0 - mse / denom
}

use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod random {

    use nadi_plugin::env_func;
    use rand::rngs::SmallRng;
    use rand::RngExt;
    use rand::SeedableRng;

    /// Random bool given uniform probability
    #[env_func(prob = 0.5)]
    fn random_bool(prob: f64, seed: Option<u64>) -> bool {
        let mut rng: SmallRng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        let prob = prob.clamp(0.0, 1.0);
        rng.random_bool(prob)
    }

    // todo more
}

use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod optimum {

    use crate::prelude::*;

    use nadi_plugin::env_func;
    use rand::rngs::SmallRng;
    use rand::RngExt;
    use rand::SeedableRng;

    /// Crossing over of linear genes from two parent given uniform probability
    ///
    /// This method gives the same probability for each gene to be
    /// from one or the other parent.
    #[env_func(prob = 0.5)]
    fn uniform_crossing_over(
        parent1: Vec<Attribute>,
        parent2: Vec<Attribute>,
        prob: f64,
        seed: Option<u64>,
    ) -> Result<Vec<Attribute>, String> {
        if parent1.len() != parent2.len() {
            return Err(format!(
                "Parents gene length mismatch: {} != {}",
                parent1.len(),
                parent2.len()
            ));
        }
        let mut rng: SmallRng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        let prob = prob.clamp(0.0, 1.0);
        Ok(parent1
            .into_iter()
            .zip(parent2)
            .map(|(p1, p2)| if rng.random_bool(prob) { p1 } else { p2 })
            .collect())
    }
}

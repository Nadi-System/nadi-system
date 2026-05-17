use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod random {
    use nadi_core::prelude::*;
    use nadi_plugin::{env_func, network_func};
    use rand::rngs::SmallRng;
    use rand::seq::SliceRandom;
    use rand::RngExt;
    use rand::SeedableRng;

    /// Random bool with the given probability
    #[env_func(prob = 0.5)]
    fn random_bool(prob: f64, seed: Option<u64>) -> bool {
        let mut rng: SmallRng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        let prob = prob.clamp(0.0, 1.0);
        rng.random_bool(prob)
    }

    /// Random float given uniform probability
    #[env_func]
    fn random(seed: Option<u64>) -> f64 {
        let mut rng: SmallRng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        rng.random()
    }

    /// Randomly choose one of the value from the list
    #[env_func]
    fn choose(choices: Vec<Attribute>, seed: Option<u64>) -> Attribute {
        let mut rng: SmallRng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        choices[rng.random_range(0..(choices.len()))].clone()
    }

    /// Randomly choose multiple of the value from the list with replacement
    #[env_func]
    fn choose_w_rep(choices: Vec<Attribute>, count: usize, seed: Option<u64>) -> Vec<Attribute> {
        let mut rng: SmallRng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        (0..count)
            .map(|_| choices[rng.random_range(0..(choices.len()))].clone())
            .collect()
    }

    /// Randomly choose multiple values from the list without replacement
    #[env_func]
    fn choose_wo_rep(choices: Vec<Attribute>, count: usize, seed: Option<u64>) -> Vec<Attribute> {
        let mut rng: SmallRng = match seed {
            Some(s) => SmallRng::seed_from_u64(s),
            None => rand::make_rng(),
        };
        let mut ind: Vec<usize> = (0..count).collect();
        ind.shuffle(&mut rng);

        ind.into_iter()
            .take(count)
            .map(|i| choices[i].clone())
            .collect()
    }

    #[network_func(count = 10, max_inputs = 3)]
    fn random_tree(net: &mut Network, count: i64, max_inputs: i64) -> Result<(), String> {
        let mut rng: SmallRng = rand::make_rng();
        let mut vert: Vec<i64> = (0..count).collect();
        vert.shuffle(&mut rng);
        let mut edges = Vec::<(String, String)>::new();
        let mut leaves = Vec::new();
        while let Some(v) = vert.pop() {
            if !leaves.is_empty() {
                let l: i64 = leaves.remove(rng.random_range(0..leaves.len()));
                edges.push((v.to_string(), l.to_string()));
            }
            let n_inps = rng.random_range(0..=max_inputs);
            for _ in 0..n_inps {
                if vert.is_empty() {
                    break;
                }
                let inp = rng.random_range(0..(vert.len()));
                let inp = vert.remove(inp);
                edges.push((inp.to_string(), v.to_string()));
                leaves.push(inp);
            }
            if n_inps == 0 {
                leaves.push(v);
            }
        }
        let ed: Vec<(&str, &str)> = edges
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        *net = Network::from_edges(&ed, false)?;
        Ok(())
    }
}

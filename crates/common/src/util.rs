use rand::{Rng, distr::Alphanumeric, rng};
use std::iter;

pub fn generate_rand_string(length: usize) -> String {
    let mut rng = rng();
    iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(length)
        .collect::<String>()
}

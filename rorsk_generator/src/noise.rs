use std::num::Wrapping;

pub(crate) fn white_noise_1d_f32(x: usize) -> f32 {
    let mut n = Wrapping(31337);
    n ^= Wrapping(1619) * Wrapping(x as u32);

    n = n * n * n * Wrapping(60493);
    let d = n.0 as f64 / 2147483648.0f64;
    d as f32
}

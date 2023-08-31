use std::mem;

use compute::Compute;

mod compute;
mod conformant;
mod noise;
mod runner;

const DATA_SIZE: usize = 16 * 1000 * 1000; // 16 MB.

fn main() {
    f32_problems();
    i32_problems();
}

fn f32_problems() {
    let mut vec = Vec::with_capacity(DATA_SIZE / mem::size_of::<f32>() * 2);
    for i in 0..vec.capacity() {
        vec.push(noise::white_noise_1d_f32(i));
    }
    let c = Compute::new(vec);

    c.compute("f32-add", "float", "r = a + b;");
    c.compute("f32-sub", "float", "r = a - b;");
    c.compute("f32-mul", "float", "r = a * b;");
    c.compute("f32-div", "float", "r = a / b;");
}

fn i32_problems() {
    let mut vec = Vec::with_capacity(DATA_SIZE / mem::size_of::<i32>() * 2);
    for i in 0..vec.capacity() {
        let mut e = ((noise::white_noise_1d_f32(i) - 0.5) * 1000000000.0) as i32;
        if e == 0 {
            e = 1;
        }
        vec.push(e);
    }

    let mut has_positive = false;
    let mut has_negative = false;
    for i in &vec {
        match 0i32.cmp(i) {
            std::cmp::Ordering::Less => has_positive = true,
            std::cmp::Ordering::Equal => continue,
            std::cmp::Ordering::Greater => has_negative = true,
        };

        if has_positive && has_negative {
            break;
        }
    }

    if !has_positive || !has_negative {
        panic!("The generated data is not suitable for this test.");
    }

    let c = Compute::new(vec);

    c.compute("i32-add", "int", "r = a + b;");
    c.compute("i32-sub", "int", "r = a - b;");
    c.compute("i32-mul", "int", "r = a * b;");
    c.compute("i32-div", "int", "r = a / b;");
}

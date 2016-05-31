fn pi(dx: f64) -> f64 {
    let upper = 5.0;
    let mut sum = 0.0;
    for i in 0..(upper / dx) as u64 + 1 {
        let x = i as f64 * dx;
        sum += (-x * x).exp();
    }
    //sum += (1.0 + (-upper * upper).exp()) * 0.5;
    let sqrt_pi = sum * dx * 2.0;
    sqrt_pi * sqrt_pi
}
fn pi2(dx: f64) -> f64 {
    let upper = 26.0;
    let mut sum = 0.0;
    for i in 1..(upper / dx) as u64 {
        let x = i as f64 * dx;
        sum += (-x * x).exp();
    }
    sum += (1.0 + (-upper * upper).exp()) * 0.5;
    let sqrt_pi = sum * dx * 2.0;
    sqrt_pi * sqrt_pi
}

fn main() {
    let correct = std::f64::consts::PI;
    let dx = 0.5e-1;
    println!("{:e}", pi(dx) - correct);
    println!("{:e}", pi2(dx) - correct);
}

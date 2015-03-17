#![feature(test)]
extern crate test;
extern crate simple_parallel;

use std::num::Float;

fn expensive(x: u64) -> f64 {
    (x as f64)
        .sin().exp().cos().abs().ln()
        .sin().exp().cos().abs().ln()
        .sin().exp().cos().abs().ln()
        .sin().exp().cos().abs().ln()
}

const TOP: u64 = 100;

#[bench]
fn naive(b: &mut test::Bencher) {
    b.iter(|| {
        (0..TOP).map(expensive).collect::<Vec<_>>()
    })
}

#[bench]
fn pool(b: &mut test::Bencher) {
    let mut pool = simple_parallel::Pool::new(4);
    let f = expensive;
    b.iter(|| {
        pool.map(0..TOP, &f).collect::<Vec<_>>()
    })
}
#[bench]
fn nopool(b: &mut test::Bencher) {
    let f = expensive;
    b.iter(|| {
        simple_parallel::map(0..TOP, &f).collect::<Vec<_>>()
    })
}

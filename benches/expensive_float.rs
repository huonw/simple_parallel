#![cfg(feature = "unstable")]
#![feature(test)]
extern crate test;
extern crate crossbeam;
extern crate simple_parallel;

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
    b.iter(|| {
        crossbeam::scope(|scope| {
            pool.map(scope, 0..TOP, expensive).collect::<Vec<_>>()
        })
    })
}
#[bench]
fn nopool(b: &mut test::Bencher) {
    b.iter(|| {
        crossbeam::scope(|scope| {
            simple_parallel::map(scope, 0..TOP, expensive).collect::<Vec<_>>()
        })
    })
}

#![feature(test)]
extern crate test;
extern crate num_cpus;
extern crate simple_parallel;

fn run<F>(b: &mut test::Bencher, mut f: F) where F: FnMut(&[&[i32]]) -> i32 {
    let v = vec![0; 1000];
    let w = vec![&*v; 5000];
    b.iter(|| {
        f(test::black_box(&w))
    })
}

fn sum<I: Iterator<Item = i32>>(iter: I) -> i32 {
    iter.fold(0, |a, b| a + b)
}

fn sum_sum(w: &[&[i32]]) -> i32 {
    sum(w.iter().map(|v| sum(v.iter().cloned())))
}

#[bench]
fn naive(b: &mut test::Bencher) {
    run(b, sum_sum)
}
#[bench]
fn pool_individual(b: &mut test::Bencher) {
    let mut pool = simple_parallel::Pool::new(num_cpus::get());
    let f = |v: &&[i32]| sum(v.iter().cloned());
    run(b, |w| {
        sum(pool.map(w, &f))
    })
}

#[bench]
fn chunked(b: &mut test::Bencher) {
    let n = num_cpus::get();
    let f = sum_sum;
    run(b, |w| {
        let per_chunk = (w.len() + n - 1) / n;

        sum(simple_parallel::map(w.chunks(per_chunk), &f))
    })
}
#[bench]
fn pool_chunked(b: &mut test::Bencher) {
    let n = num_cpus::get();
    let mut pool = simple_parallel::Pool::new(n);
    let f = sum_sum;
    run(b, |w| {
        let per_chunk = (w.len() + n - 1) / n;

        sum(pool.map(w.chunks(per_chunk), &f))
    })
}

extern crate crossbeam;
extern crate simple_parallel;
extern crate num_cpus;

fn sum<I: Iterator<Item = i32>>(iter: I) -> i32 {
    iter.fold(0, |a, b| a + b)
}

fn main() {
    let mut pool = simple_parallel::Pool::new(3 * num_cpus::get());

    crossbeam::scope(|scope| {
        println!("{}", sum(pool.map(scope, 0..1_000_000, |_| 0)));
    })
}

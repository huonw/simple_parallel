extern crate simple_parallel;
extern crate crossbeam;

fn fib(n: u64) -> u64 {
    if n <= 2 { n } else { fib(n - 1) + fib(n - 3) }
}

fn main() {
    // 4 threads
    let mut pool = simple_parallel::Pool::new(1);

    crossbeam::scope(|scope| {
        for (i, f) in pool.map(scope, 0..50, |i| (i, fib(i))) {
            println!("{}: {}", i, f);
        }
    });
    let _ = 1;
}

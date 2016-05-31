extern crate simple_parallel;

fn fib(n: u64) -> u64 {
    if n <= 2 { n } else { fib(n - 1) + fib(n - 3) }
}

fn main() {
    let mut pool = simple_parallel::Pool::new(8);

    pool.for_((0..50).cycle().take(1000), |i| {
        println!("{}: {}", i, fib(i));
    })
}

fn fib(n: u64) -> u64 {
    if n <= 2 { n } else { fib(n - 1) + fib(n - 3) }
}

fn main() {
    let iter = std::env::args()
        .skip(1)
        .map(|s| {let i = s.parse().map(|i| fib(i)); (s, i) });

    for (input, ans) in iter {
        if let Ok(x) = ans {
            println!("{}: {}", input, x);
        } else {

        }
    }
}

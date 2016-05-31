extern crate simple_parallel;
extern crate crossbeam;

fn main() {
    let mut pool = simple_parallel::Pool::new(4);

    crossbeam::scope(|scope| {
        for _ in pool.map(scope, 0..100, |i| {
            println!("{}", i);
            if i == 10 {
                panic!("innermost");
            }
        }) {}
    });
}

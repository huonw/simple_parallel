#![cfg(feature = "unstable")]
#![feature(test, iter_arith)]
extern crate test;
extern crate num_cpus;
extern crate simple_parallel;
extern crate crossbeam;

fn busy(x: u32) {
    for i in 0..x { test::black_box(i); }
}

fn parse(x: &str) -> u32 {
    x[1..].parse().unwrap()
}
macro_rules! parse {
    ($x: ident) => {
        ::parse(stringify!($x))
    }
}
const N: usize = 100;

macro_rules! tests {
    ($($name: ident,)*) => {
        mod naive {
            $(#[bench]
              fn $name(b: &mut ::test::Bencher) {
                let n = parse!($name);
                let v = vec![(); ::N];
                b.iter(|| {
                    v.iter().map(|_| ::busy(n)).count()
                })
            })*
        }

        mod pool_chunked {
            $(#[bench]
              fn $name(b: &mut ::test::Bencher) {
                let v = vec![(); ::N];
                let n = parse!($name);
                let ncpus = ::num_cpus::get();
                let mut pool = ::simple_parallel::Pool::new(ncpus);
                b.iter(|| {
                    let per_chunk: usize = ::N / (3 * ncpus);
                    ::crossbeam::scope(|scope| -> usize {
                        pool.map(scope, v.chunks(per_chunk), |w: &[()]| {
                            w.iter().map(|_| ::busy(n)).count()
                        }).sum()
                    })
                })
            })*
        }
        mod pool_individual {
            $(#[bench]
              fn $name(b: &mut ::test::Bencher) {
                let v = vec![(); ::N];
                let n = parse!($name);
                let mut pool = ::simple_parallel::Pool::new(::num_cpus::get());
                b.iter(|| {
                    ::crossbeam::scope(|scope| {
                        pool.map(scope, v.iter(), |_| ::busy(n)).count()
                    })
                })
            })*
        }
    }
}
tests!{
    _0000001,
    _0000010,
    _0000100,
    _0001000,
    _0010000,
    _0100000,
    _0200000,
    _0400000,
}

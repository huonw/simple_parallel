extern crate simple_parallel;
extern crate crossbeam;

use std::sync::Mutex;

fn eq<I, J>(mut x: I, mut y: J) -> bool
    where I: Iterator, J: Iterator<Item = I::Item>, I::Item: PartialEq
{
    loop {
        match (x.next(), y.next()) {
            (None, None) => return true,
            (Some(ref x), Some(ref y)) if x == y => continue,
            _ => return false
        }
    }
}

// with this many elements, its (hopefully) likely that the
// threads won't execute sequentially.
const N: usize = 1000;
const M: usize = 100;

#[test]
fn unordered_map_probabilistic_out_of_ordering() {
    crossbeam::scope(|scope| {
        let iter = simple_parallel::unordered_map(scope, 0..N, |_| {});

        // see if there are any elements where the order they come out of
        // the original iterator is different to the order in which they
        // are yielded here.
        assert!(!eq(iter.map(|t| t.0), 0..N));
    });
}

#[test]
fn map_in_order() {
    crossbeam::scope(|scope| {
        let iter = simple_parallel::map(scope, 0..N, |i| i);

        assert!(eq(iter, 0..N));
    });
}

#[test]
fn unordered_map_delays() {
    let finished = Mutex::new(0);
    crossbeam::scope(|scope| {
        let iter = simple_parallel::unordered_map(scope, 0..M, |_| {
            *finished.lock().unwrap() += 1;
        });
        ::std::mem::forget(iter);
    });

    assert_eq!(*finished.lock().unwrap(), M);
}

#[test]
fn map_delays() {
    let finished = Mutex::new(0);
    crossbeam::scope(|scope| {
        let iter = simple_parallel::map(scope, 0..M, |_| {
            *finished.lock().unwrap() += 1;
        });
        ::std::mem::forget(iter);
    });

    assert_eq!(*finished.lock().unwrap(), M);
}

#[test]
fn pool_unordered() {
    crossbeam::scope(|scope| {
        let mut pool = simple_parallel::Pool::new(8);

        let iter = pool.unordered_map(scope, 0..N, |_| {});

        // see if there are any elements where the order they come out of
        // the original iterator is different to the order in which they
        // are yielded here.
        assert!(!eq(iter.map(|t| t.0), 0..N));
    });
}

#[test]
fn pool_map_in_order() {
    crossbeam::scope(|scope| {
        let mut pool = simple_parallel::Pool::new(8);

        let iter = pool.map(scope, 0..N, |i| i);

        assert!(eq(iter, 0..N));
    });
}

#[test]
fn pool_unordered_map_delays() {
    let mut pool = simple_parallel::Pool::new(8);
    let finished = Mutex::new(0);
    crossbeam::scope(|scope| {
        let iter = pool.unordered_map(scope, 0..M, |_| {
            *finished.lock().unwrap() += 1;
        });
        ::std::mem::forget(iter);
    });

    assert_eq!(*finished.lock().unwrap(), M);
}

#[test]
fn pool_map_delays() {
    let mut pool = simple_parallel::Pool::new(8);
    let finished = Mutex::new(0);
    crossbeam::scope(|scope| {
        let iter = pool.map(scope, 0..M, |_| {
            *finished.lock().unwrap() += 1;
        });
        ::std::mem::forget(iter);
    });

    assert_eq!(*finished.lock().unwrap(), M);
}

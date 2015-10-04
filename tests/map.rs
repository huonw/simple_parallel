#![feature(iter_order)]
extern crate simple_parallel;
extern crate crossbeam;

use std::sync::Mutex;

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
        assert!(iter.enumerate().any(|(yield_order, (true_order, ()))| yield_order != true_order));
    });
}

#[test]
fn map_in_order() {
    crossbeam::scope(|scope| {
        let iter = simple_parallel::map(scope, 0..N, |i| i);

        assert!(iter.enumerate().all(|(yield_order, true_order)| yield_order == true_order));
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
        let v: Vec<_> = iter.enumerate().collect();
        assert_eq!(v.len(), N);
        assert!(v.into_iter().any(|(yield_order, (true_order, ()))| yield_order != true_order));
    });
}

#[test]
fn pool_map_in_order() {
    crossbeam::scope(|scope| {
        let mut pool = simple_parallel::Pool::new(8);

        let iter = pool.map(scope, 0..N, |i| i);

        assert!(iter.eq(0..N));
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

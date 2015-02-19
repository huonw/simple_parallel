extern crate simple_parallel;

use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};

#[test]
fn probabilistic_out_of_ordering() {
    // with this many elements, its (hopefully) likely that the
    // threads won't execute sequentially.
    const N: usize = 100;

    let mut index = (0..N).map(|_| 0).collect::<Vec<_>>();

    static ORDER: AtomicUsize = ATOMIC_USIZE_INIT;
    ORDER.store(0, Ordering::SeqCst);

    simple_parallel::for_(index.iter_mut(), |x| {
        *x = ORDER.fetch_add(1, Ordering::SeqCst);
    });

    assert!(index.iter().zip(index[1..].iter()).any(|(a, b)| a > b));
}

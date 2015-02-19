extern crate simple_parallel;

#[test]
fn unordered_map_probabilistic_out_of_ordering() {
    // with this many elements, its (hopefully) likely that the
    // threads won't execute sequentially.
    const N: usize = 1000;

    let f = |_: usize| ();
    let iter = simple_parallel::unordered_map(0..N, &f);

    // see if there are any elements where the order they come out of
    // the original iterator is different to the order in which they
    // are yielded here.
    assert!(iter.enumerate().any(|(yield_order, (true_order, ()))| yield_order != true_order));
}

#[test]
fn map_in_order() {
    // with this many elements, its (hopefully) likely that the
    // threads won't execute sequentially.
    const N: usize = 1000;

    let f = |i: usize| i;
    let iter = simple_parallel::map(0..N, &f);

    assert!(iter.enumerate().all(|(yield_order, true_order)| yield_order == true_order));
}

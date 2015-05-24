#![cfg(feature = "unstable")]
extern crate simple_parallel;

#[test]
fn smoke() {
    let mut slice = [0u8; 10];
    let (l, r) = {
        let (a, b) = slice.split_at_mut(5);
        simple_parallel::both(a, b, |x| {
            for (i, y) in x.iter_mut().enumerate() {
                *y = i as u8
            }
            x.as_ptr() as usize
        })
    };

    assert_eq!(slice, [0, 1, 2, 3, 4, 0, 1, 2, 3, 4]);
    // check the tuple elements are in the correct order.
    assert_eq!(l, &slice[0] as *const _ as usize);
    assert_eq!(r, &slice[5] as *const _ as usize);
}

#![feature(core, std_misc)]
use std::thread::Thread;

/// Execute `f` on each element of `iter`, in their own `scoped`
/// thread.
///
/// If `f` panics, so does `for_`. If this occurs, the number of
/// elements of `iter` that have for executed is unspecified.
pub fn for_<I: Iterator, F>(iter: I, ref f: F)
    where I::Item: Send, F: Fn(I::Item) + Sync
{
    let _guards: Vec<_> = iter.map(|elem| {
        Thread::scoped(move || {
            f(elem)
        })
    }).collect();
}

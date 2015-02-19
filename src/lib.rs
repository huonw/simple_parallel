#![feature(unsafe_destructor)]
#![feature(core)]

use std::thread;
use std::iter::IntoIterator;

mod maps;

pub use maps::{unordered_map, UnorderedParMap, map, ParMap};

/// Execute `f` on each element of `iter`, in their own `scoped`
/// thread.
///
/// If `f` panics, so does `for_`. If this occurs, the number of
/// elements of `iter` that have had `f` called on them is
/// unspecified.
pub fn for_<I: IntoIterator, F>(iter: I, ref f: F)
    where I::Item: Send, F: Fn(I::Item) + Sync
{
    let _guards: Vec<_> = iter.into_iter().map(|elem| {
        thread::scoped(move || {
            f(elem)
        })
    }).collect();
}

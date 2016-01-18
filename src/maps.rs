use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::iter::IntoIterator;
use std::mem;

use {pool, Pool, num_cpus};
use crossbeam::Scope;

/// A parallel-mapping iterator that doesn't care about the order in
/// which elements come out. Constructed by calling `unordered_map`.
pub struct UnorderedParMap<'a, T: Send + 'a> {
    inner: pool::UnorderedParMap<'static, 'a, T>,
    // this needs to be second, so that it is dropped after `inner`.
    _pool: Box<Pool>,
}

impl<'a, T: Send> Iterator for UnorderedParMap<'a, T> {
    type Item = (usize, T);

    fn next(&mut self) -> Option<(usize, T)> {
        self.inner.next()
    }
}

/// Execute `f` on each element in `iter`, with unspecified yield order.
///
/// This behaves like `simple_parallel::map`, but does not make
/// efforts to ensure that the elements are returned in the order of
/// `iter`, hence this is cheaper.
pub fn unordered_map<'a, I: IntoIterator, F, T>(scope: &Scope<'a>, iter: I, f: F) -> UnorderedParMap<'a, T>
    where I: Send + 'a,
          I::Item: Sync + Send + 'a,
          F: 'a + Send + Sync + Fn(I::Item) -> T,
          T: Send + 'a
{
    let mut pool = Box::new(Pool::new(num_cpus::get()));

    let map: pool::UnorderedParMap<'static, 'a, T> = unsafe {
        mem::transmute(pool.unordered_map(scope, iter, f))
    };

    UnorderedParMap {
        _pool: pool,
        inner: map,
    }
}

struct Packet<T> {
    // this should be unique for a given instance of `*ParMap`
    idx: usize,
    data: Option<T>,
}

impl<T> PartialOrd for Packet<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl<T> Ord for Packet<T> {
    // reverse the ordering, to work with the max-heap
    fn cmp(&self, other: &Self) -> Ordering { other.idx.cmp(&self.idx) }
}
impl<T> PartialEq for Packet<T> {
    fn eq(&self, other: &Self) -> bool { self.idx == other.idx }
}
impl<T> Eq for Packet<T> {}

/// A parallel-mapping iterator. Constructed by calling `map`.
pub struct ParMap<'a, T: Send + 'a> {
    unordered: UnorderedParMap<'a, T>,
    looking_for: usize,
    queue: BinaryHeap<Packet<T>>
}

impl<'a, T: Send> Iterator for ParMap<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        loop {
            if self.queue.peek().map_or(false, |x| x.idx == self.looking_for) {
                // we've found what we want, so lets return it

                let packet = self.queue.pop().unwrap();
                self.looking_for += 1;
                match packet.data {
                    Some(x) => return Some(x),
                    None => panic!("simple_parallel::map: closure panicked")
                }
            }
            match self.unordered.next() {
                // this could be optimised to check for `packet.idx ==
                // self.looking_for` to avoid the BinaryHeap
                // interaction if its what we want.
                Some((idx, x)) => self.queue.push(Packet { idx: idx, data: Some(x) }),
                // all done
                None => return None,
            }
        }
    }
}

/// Execute `f` on `iter`, yielding the results in the order the
/// correspond to in `iter`.
///
/// This is a drop-in replacement for `iter.map(f)`, that runs in
/// parallel, and eagerly consumes `iter` spawning a thread for each
/// element.
pub fn map<'a, I: IntoIterator, F, T>(scope: &Scope<'a>, iter: I, f: F) -> ParMap<'a, T>
    where I: Send + 'a,
          I::Item: 'a + Send + Sync,
          F: 'a + Send + Sync + Fn(I::Item) -> T,
          T: Send + 'a
{
    ParMap {
        unordered: unordered_map(scope, iter, f),
        looking_for: 0,
        queue: BinaryHeap::new(),
    }
}

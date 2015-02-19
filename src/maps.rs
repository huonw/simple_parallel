use std::sync::mpsc::{self, Sender, Receiver};
use std::thread::{self,JoinGuard};
use std::cmp::Ordering;
use std::collections::BinaryHeap;


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


/// A parallel-mapping iterator that doesn't care about the order in
/// which elements come out.
pub struct UnorderedParMap<'a, T: 'a + Send> {
    rx: Receiver<Packet<T>>,
    _guards: Vec<JoinGuard<'a, ()>>
}

impl<'a,T: 'static + Send> Iterator for UnorderedParMap<'a, T> {
    type Item = (usize, T);

    fn next(&mut self) -> Option<(usize, T)> {
        match self.rx.recv() {
            Ok(Packet { data: Some(x), idx }) => Some((idx, x)),
            Ok(Packet { data: None, .. }) => {
                panic!("simple_parallel::unordered_map: closure panicked")
            }
            Err(mpsc::RecvError) => None,
        }
    }
}

struct Panicker<T: Send + 'static> {
    tx: Sender<Packet<T>>,
    idx: usize,
    all_ok: bool
}
#[unsafe_destructor]
impl<T: Send + 'static> Drop for Panicker<T> {
    fn drop(&mut self) {
        if !self.all_ok {
            let _ = self.tx.send(Packet { idx: self.idx, data: None });
        }
    }
}

/// Execute `f` on each element in `iter`, with unspecified yield order.
///
/// This behaves like `simple_parallel::map`, but does not make
/// efforts to ensure that the elements are returned in the order of
/// `iter`, hence this is cheaper.
pub fn unordered_map<'a, I: Iterator, F, T>(iter: I, f: &'a F) -> UnorderedParMap<'a, T>
    where I::Item: Send + 'a,
          F: 'a + Sync + Fn(I::Item) -> T,
          T: Send + 'static
{
    let (tx, rx) = mpsc::channel();

    let guards = iter.enumerate().map(|(idx, elem)| {
        let tx = tx.clone();
        let f = f.clone();

        thread::scoped(move || {
            let mut p = Panicker { tx: tx, idx: idx, all_ok: false };
            let val = f(elem);
            let _ = p.tx.send(Packet { idx: idx, data: Some(val) });
            p.all_ok = true;
        })
    }).collect();

    UnorderedParMap {
        rx: rx,
        _guards: guards,
    }
}

/// A parallel-mapping iterator.
pub struct ParMap<'a, T: 'a + Send> {
    unordered: UnorderedParMap<'a, T>,
    looking_for: usize,
    queue: BinaryHeap<Packet<T>>
}

impl<'a, T: Send + 'static> Iterator for ParMap<'a, T> {
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
            match self.unordered.rx.recv() {
                // this could be optimised to check for `packet.idx ==
                // self.looking_for` to avoid the BinaryHeap
                // interaction if its what we want.
                Ok(packet) => self.queue.push(packet),
                // all done
                Err(mpsc::RecvError) => return None,
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
pub fn map<'a, I: Iterator, F, T>(iter: I, f: &'a F) -> ParMap<'a, T>
    where I::Item: Send + 'a,
          F: 'a + Sync + Fn(I::Item) -> T,
          T: Send + 'static
{
    ParMap {
        unordered: unordered_map(iter, f),
        looking_for: 0,
        queue: BinaryHeap::new(),
    }
}

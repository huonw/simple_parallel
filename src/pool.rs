use std::collections::BinaryHeap;
use std::iter::IntoIterator;
use std::{marker, mem};
use std::sync::{mpsc, atomic, Mutex, Arc};
use std::thread;
use fnbox::FnBox;

use crossbeam::{self, Scope};

type JobInner<'b> =  Box<for<'a> FnBox<&'a [mpsc::Sender<Work>]> + Send + 'b>;
struct Job {
    func: JobInner<'static>,
}

/// A thread pool.
///
/// This pool allows one to spawn several threads in one go, and then
/// execute any number of "short-lifetime" jobs on those threads,
/// without having to pay the thread spawning cost, or risk exhausting
/// system resources.
///
/// The pool currently consists of some number of worker threads
/// (dynamic, chosen at creation time) along with a single supervisor
/// thread. The synchronisation overhead is currently very large.
///
/// # "Short-lifetime"?
///
/// Jobs submitted to this pool can have any lifetime at all, that is,
/// the closures passed in (and elements of iterators used, etc.) can
/// have borrows pointing into arbitrary stack frames, even stack
/// frames that don't outlive the pool itself. This differs to
/// something like
/// [`scoped_threadpool`](https://crates.io/crates/scoped_threadpool),
/// where the jobs must outlive the pool.
///
/// This extra flexibility is achieved with careful unsafe code, by
/// exposing an API that is a generalised version of
/// [`crossbeam`](https://github.com/aturon/crossbeam) `Scope::spawn`
/// and the old `std::thread::scoped`: at the lowest-level a submitted
/// job returns a `JobHandle` token that ensures that job is finished
/// before any data the job might reference is invalidated
/// (i.e. manages the lifetimes). Higher-level functions will usually
/// wrap or otherwise hide the handle.
///
/// However, this comes at a cost: for easy of implementation `Pool`
/// currently only exposes "batch" jobs like `for_` and `map` and
/// these jobs take control of the whole pool. That is, one cannot
/// easily incrementally submit arbitrary closures to execute on this
/// thread pool, which is functionality that `threadpool::ScopedPool`
/// offers.
///
/// # Example
///
/// ```rust
/// extern crate crossbeam;
/// extern crate simple_parallel;
/// use simple_parallel::Pool;
///
/// // a function that takes some arbitrary pool and uses the pool to
/// // manipulate data in its own stack frame.
/// fn do_work(pool: &mut Pool) {
///     let mut v = [0; 8];
///     // set each element, in parallel
///     pool.for_(&mut v, |element| *element = 3);
///
///     let w = [2, 0, 1, 5, 0, 3, 0, 3];
///
///     // add the two arrays, in parallel
///     let z: Vec<_> = crossbeam::scope(|scope| {
///         pool.map(scope, v.iter().zip(w.iter()), |(x, y)| *x + *y).collect()
///     });
///
///     assert_eq!(z, &[5, 3, 4, 8, 3, 6, 3, 6]);
/// }
///
/// # fn main() {
/// let mut pool = Pool::new(4);
/// do_work(&mut pool);
/// # }
/// ```
pub struct Pool {
    job_queue: mpsc::Sender<(Option<Job>, mpsc::Sender<Result<(), ()>>)>,
    job_status: Option<Arc<Mutex<JobStatus>>>,
    n_threads: usize,
}
#[derive(Copy, Clone)]
pub struct WorkerId { n: usize }

type WorkInner<'a> = &'a mut (FnMut(WorkerId) + Send + 'a);
struct Work {
    func: WorkInner<'static>
}

struct JobStatus {
    wait: bool,
    job_finished: mpsc::Receiver<Result<(), ()>>,
}

/// A token representing a job submitted to the thread pool.
///
/// This helps ensure that a job is finished before borrowed resources
/// in the job (and the pool itself) are invalidated.
///
/// If the job panics, this handle will ensure the main thread also
/// panics (either via `wait` or in the destructor).
pub struct JobHandle<'pool, 'f> {
    pool: &'pool mut Pool,
    status: Arc<Mutex<JobStatus>>,
    _funcs: marker::PhantomData<&'f ()>,
}

impl JobStatus {
    fn wait(&mut self) {
        if self.wait {
            self.wait = false;
            self.job_finished.recv().unwrap().unwrap();
        }
    }
}

impl<'pool, 'f> JobHandle<'pool, 'f> {
    /// Block until the job is finished.
    ///
    /// # Panics
    ///
    /// This will panic if the job panicked.
    pub fn wait(&self) {
        self.status.lock().unwrap().wait();
    }
}
impl<'pool, 'f> Drop for JobHandle<'pool, 'f> {
    fn drop(&mut self) {
        self.wait();
        self.pool.job_status = None;
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.job_queue.send((None, tx)).unwrap();
        rx.recv().unwrap().unwrap();
    }
}
struct PanicCanary<'a> {
    flag: &'a atomic::AtomicBool
}
impl<'a> Drop for PanicCanary<'a> {
    fn drop(&mut self) {
        if thread::panicking() {
            self.flag.store(true, atomic::Ordering::SeqCst)
        }
    }
}
impl Pool {
    /// Create a new thread pool with `n_threads` worker threads.
    pub fn new(n_threads: usize) -> Pool {
        let (tx, rx) = mpsc::channel::<(Option<Job>, mpsc::Sender<Result<(), ()>>)>();

        thread::spawn(move || {
            let panicked = Arc::new(atomic::AtomicBool::new(false));

            let mut _guards = Vec::with_capacity(n_threads);
            let mut txs = Vec::with_capacity(n_threads);

            for i in 0..n_threads {
                let id = WorkerId { n: i };
                let (subtx, subrx) = mpsc::channel::<Work>();
                txs.push(subtx);

                let panicked = panicked.clone();
                _guards.push(thread::spawn(move || {
                    let _canary = PanicCanary {
                        flag: &panicked
                    };
                    loop {
                        match subrx.recv() {
                            Ok(mut work) => {
                                (work.func)(id)
                            }
                            Err(_) => break,
                        }
                    }
                }))
            }

            loop {
                match rx.recv() {
                    Ok((Some(job), finished_tx)) => {
                        (job.func).call_box(&txs);
                        let job_panicked = panicked.load(atomic::Ordering::SeqCst);
                        let msg = if job_panicked { Err(()) } else { Ok(()) };
                        finished_tx.send(msg).unwrap();
                        if job_panicked { break }
                    }
                    Ok((None, finished_tx)) => {
                        finished_tx.send(Ok(())).unwrap();
                        break
                    }
                    Err(_) => break,
                }
            }
        });

        Pool {
            job_queue: tx,
            job_status: None,
            n_threads: n_threads,
        }
    }

    /// Execute `f` on each element of `iter`.
    ///
    /// This panics if `f` panics, although the precise time and
    /// number of elements consumed after the element that panics is
    /// not specified.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use simple_parallel::Pool;
    ///
    /// let mut pool = Pool::new(4);
    ///
    /// let mut v = [0; 8];
    ///
    /// // set each element, in parallel
    /// pool.for_(&mut v, |element| *element = 3);
    ///
    /// assert_eq!(v, [3; 8]);
    /// ```
    pub fn for_<Iter: IntoIterator, F>(&mut self, iter: Iter, ref f: F)
        where Iter::Item: Send,
              Iter: Send,
              F: Fn(Iter::Item) + Sync

    {
        let (needwork_tx, needwork_rx) = mpsc::channel();
        let mut work_txs = Vec::with_capacity(self.n_threads);
        let mut work_rxs = Vec::with_capacity(self.n_threads);
        for _ in 0..self.n_threads {
            let (t, r) = mpsc::channel();
            work_txs.push(t);
            work_rxs.push(r);
        }

        let mut work_rxs = work_rxs.into_iter();

        crossbeam::scope(|scope| unsafe {
            let handle = self.execute(
                scope,
                needwork_tx,
                |needwork_tx| {
                    let mut needwork_tx = Some(needwork_tx.clone());
                    let mut work_rx = Some(work_rxs.next().unwrap());
                    move |id| {
                        let work_rx = work_rx.take().unwrap();
                        let needwork = needwork_tx.take().unwrap();
                        loop {
                            needwork.send(id).unwrap();
                            match work_rx.recv() {
                                Ok(Some(elem)) => {
                                    f(elem);
                                }
                                Ok(None) | Err(_) => break
                            }
                        }
                    }
                },
                move |needwork_tx| {
                    let mut iter = iter.into_iter().fuse();
                    drop(needwork_tx);
                    loop {
                        match needwork_rx.recv() {
                            // closed, done!
                            Err(_) => break,
                            Ok(id) => {
                                work_txs[id.n].send(iter.next()).unwrap();
                            }
                        }
                    }
                });

            handle.wait();
        })
    }

    /// Execute `f` on each element in `iter` in parallel across the
    /// pool's threads, with unspecified yield order.
    ///
    /// This behaves like `map`, but does not make efforts to ensure
    /// that the elements are returned in the order of `iter`, hence
    /// this is cheaper.
    ///
    /// The iterator yields `(uint, T)` tuples, where the `uint` is
    /// the index of the element in the original iterator.
    ///
    /// # Examples
    ///
    /// ```rust
    /// extern crate crossbeam;
    /// extern crate simple_parallel;
    /// # fn main() {
    /// use simple_parallel::Pool;
    ///
    /// let mut pool = Pool::new(4);
    ///
    /// // adjust each element in parallel, and iterate over them as
    /// // they are generated (or as close to that as possible)
    /// crossbeam::scope(|scope| {
    ///     for (index, output) in pool.unordered_map(scope, 0..8, |i| i + 10) {
    ///         // each element is exactly 10 more than its original index
    ///         assert_eq!(output, index as i32 + 10);
    ///     }
    /// })
    /// # }
    /// ```
    pub fn unordered_map<'pool, 'a, I: IntoIterator, F, T>(&'pool mut self, scope: &Scope<'a>, iter: I, f: F)
        -> UnorderedParMap<'pool, 'a, T>
        where I: 'a + Send,
              I::Item: Send + 'a,
              F: 'a + Sync + Send + Fn(I::Item) -> T,
              T: Send + 'a
    {
        let nthreads = self.n_threads;
        let (needwork_tx, needwork_rx) = mpsc::channel();
        let (work_tx, work_rx) = mpsc::channel();
        struct Shared<Chan, Atom, F> {
            work: Chan,
            sent: Atom,
            finished: Atom,
            func: F,
        }
        let shared = Arc::new(Shared {
            work: Mutex::new(work_rx),
            sent: atomic::AtomicUsize::new(0),
            finished: atomic::AtomicUsize::new(0),
            func: f,
        });

        let (tx, rx) = mpsc::channel();

        const INITIAL_FACTOR: usize = 4;
        const BUFFER_FACTOR: usize = INITIAL_FACTOR / 2;

        let handle = unsafe {
            self.execute(scope, (needwork_tx, shared),
                         move |&mut (ref needwork_tx, ref shared)| {
                             let mut needwork_tx = Some(needwork_tx.clone());
                             let tx = tx.clone();
                             let shared = shared.clone();
                             move |_id| {
                                 let needwork = needwork_tx.take().unwrap();
                                 loop {
                                     let data =  {
                                         let guard = shared.work.lock().unwrap();
                                         guard.recv()
                                     };
                                     match data {
                                         Ok(Some((idx, elem))) => {
                                             let data = (shared.func)(elem);
                                             let status = tx.send(Packet {
                                                 idx: idx, data: data
                                             });
                                             // the user disconnected,
                                             // so there's no point
                                             // computing more.
                                             if status.is_err() {
                                                 let _ = needwork.send(true);
                                                 break
                                             }
                                         }
                                         Ok(None) | Err(_) => {
                                             break
                                         }
                                     };
                                     let old =
                                         shared.finished.fetch_add(1, atomic::Ordering::SeqCst);
                                     let sent = shared.sent.load(atomic::Ordering::SeqCst);

                                     if old + BUFFER_FACTOR * nthreads == sent {
                                         if needwork.send(false).is_err() {
                                             break
                                         }
                                     }
                                 }
                             }
                         },
                         move |(needwork_tx, shared)| {
                             let mut iter = iter.into_iter().fuse().enumerate();
                             drop(needwork_tx);

                             let mut send_data = |n: usize| {
                                 shared.sent.fetch_add(n, atomic::Ordering::SeqCst);

                                 for _ in 0..n {
                                     // TODO: maybe this could instead send
                                     // several elements at a time, to
                                     // reduce the number of
                                     // allocations/atomic operations
                                     // performed.
                                     //
                                     // Downside: work will be
                                     // distributed chunkier.
                                     let _ = work_tx.send(iter.next());
                                 }
                             };


                             send_data(INITIAL_FACTOR * nthreads);

                             loop {
                                 match needwork_rx.recv() {
                                     // closed, done!
                                     Ok(true) | Err(_) => break,
                                     Ok(false) => {
                                         // ignore return, because we
                                         // need to wait until the
                                         // workers have exited (i.e,
                                         // the Err arm above)
                                         let _ = send_data(BUFFER_FACTOR * nthreads);
                                     }
                                 }
                             }
                         })
        };
        UnorderedParMap {
            rx: rx,
            _guard: handle,
        }
    }

    /// Execute `f` on `iter` in parallel across the pool's threads,
    /// returning an iterator that yields the results in the order of
    /// the elements of `iter` to which they correspond.
    ///
    /// This is a drop-in replacement for `iter.map(f)`, that runs in
    /// parallel, and consumes `iter` as the pool's threads complete
    /// their previous tasks.
    ///
    /// See `unordered_map` if the output order is unimportant.
    ///
    /// # Examples
    ///
    /// ```rust
    /// extern crate crossbeam;
    /// extern crate simple_parallel;
    /// use simple_parallel::Pool;
    ///
    /// # fn main() {
    /// let mut pool = Pool::new(4);
    ///
    /// // create a vector by adjusting 0..8, in parallel
    /// let elements: Vec<_> = crossbeam::scope(|scope| {
    ///     pool.map(scope, 0..8, |i| i + 10).collect()
    /// });
    ///
    /// assert_eq!(elements, &[10, 11, 12, 13, 14, 15, 16, 17]);
    /// # }
    /// ```
    pub fn map<'pool, 'a, I: IntoIterator, F, T>(&'pool mut self, scope: &Scope<'a>, iter: I, f: F)
        -> ParMap<'pool, 'a, T>
        where I: 'a + Send,
              I::Item: Send + 'a,
              F: 'a + Send + Sync + Fn(I::Item) -> T,
              T: Send + 'a
    {
        ParMap {
            unordered: self.unordered_map(scope, iter, f),
            looking_for: 0,
            queue: BinaryHeap::new(),
        }
    }
}

/// Low-level/internal functionality.
impl Pool {
    /// Run a job on the thread pool.
    ///
    /// `gen_fn` is called `self.n_threads` times to create the
    /// functions to execute on the worker threads. Each of these is
    /// immediately called exactly once on a worker thread (that is,
    /// they are semantically `FnOnce`), and `main_fn` is also called,
    /// on the supervisor thread. It is expected that the workers and
    /// `main_fn` will manage any internal coordination required to
    /// distribute chunks of work.
    ///
    /// The job must take pains to ensure `main_fn` doesn't quit
    /// before the workers do.
    pub unsafe fn execute<'pool, 'f, A, GenFn, WorkerFn, MainFn>(
        &'pool mut self, scope: &Scope<'f>, data: A, gen_fn: GenFn, main_fn: MainFn) -> JobHandle<'pool, 'f>

        where A: 'f + Send,
              GenFn: 'f + FnMut(&mut A) -> WorkerFn + Send,
              WorkerFn: 'f + FnMut(WorkerId) + Send,
              MainFn: 'f + FnOnce(A) + Send,
    {
        self.execute_nonunsafe(scope, data, gen_fn, main_fn)
    }

    // separate function to ensure we get `unsafe` checking inside this one
    fn execute_nonunsafe<'pool, 'f, A, GenFn, WorkerFn, MainFn>(
        &'pool mut self, scope: &Scope<'f>, mut data: A,
        mut gen_fn: GenFn, main_fn: MainFn) -> JobHandle<'pool, 'f>

        where A: 'f + Send,
              GenFn: 'f + FnMut(&mut A) -> WorkerFn + Send,
              WorkerFn: 'f + FnMut(WorkerId) + Send,
              MainFn: 'f + FnOnce(A) + Send,
    {
        let n_threads = self.n_threads;
        // transmutes scary? only a little: the returned `JobHandle`
        // ensures safety by connecting this job to the outside stack
        // frame.
        let func: JobInner<'f> = Box::new(move |workers: &[mpsc::Sender<Work>]| {
            assert_eq!(workers.len(), n_threads);
            let mut worker_fns: Vec<_> = (0..n_threads).map(|_| gen_fn(&mut data)).collect();

            for (func, worker) in worker_fns.iter_mut().zip(workers.iter()) {
                let func: WorkInner = func;
                let func: WorkInner<'static> = unsafe {
                    mem::transmute(func)
                };
                worker.send(Work { func: func }).unwrap();
            }

            main_fn(data)
        });
        let func: JobInner<'static> = unsafe {
            mem::transmute(func)
        };
        let (tx, rx) = mpsc::channel();
        self.job_queue.send((Some(Job { func: func }), tx)).unwrap();

        let status = Arc::new(Mutex::new(JobStatus {
            wait: true,
            job_finished: rx,
        }));
        // this probably isn't quite right? what happens to older jobs
        // (e.g. if a previous one was mem::forget'd)
        self.job_status = Some(status.clone());
        let status_ = status.clone();
        scope.defer(move || {
            status_.lock().unwrap().wait();
        });
        JobHandle {
            pool: self,
            status: status,
            _funcs: marker::PhantomData,
        }
    }
}


use std::cmp::Ordering;

struct Packet<T> {
    // this should be unique for a given instance of `*ParMap`
    idx: usize,
    data: T,
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

/// A parallel-mapping iterator, that yields elements in the order
/// they are computed, not the order from which they are yielded by
/// the underlying iterator. Constructed by calling
/// `Pool::unordered_map`.
pub struct UnorderedParMap<'pool, 'a, T: 'a + Send> {
    rx: mpsc::Receiver<Packet<T>>,
    _guard: JobHandle<'pool, 'a>,
}
impl<'pool, 'a,T: 'a + Send> Iterator for UnorderedParMap<'pool , 'a, T> {
    type Item = (usize, T);

    fn next(&mut self) -> Option<(usize, T)> {
        match self.rx.recv() {
            Ok(Packet { data, idx }) => Some((idx, data)),
            Err(mpsc::RecvError) => None,
        }
    }
}

/// A parallel-mapping iterator, that yields elements in the order
/// they are yielded by the underlying iterator. Constructed by
/// calling `Pool::map`.
pub struct ParMap<'pool, 'a, T: 'a + Send> {
    unordered: UnorderedParMap<'pool, 'a, T>,
    looking_for: usize,
    queue: BinaryHeap<Packet<T>>
}

impl<'pool, 'a, T: Send + 'a> Iterator for ParMap<'pool, 'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        loop {
            if self.queue.peek().map_or(false, |x| x.idx == self.looking_for) {
                // we've found what we want, so lets return it

                let packet = self.queue.pop().unwrap();
                self.looking_for += 1;
                return Some(packet.data)
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

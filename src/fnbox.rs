pub trait FnBox<A> {
    fn call_box(self: Box<Self>, arg: A);
}

impl<F: FnOnce(A), A> FnBox<A> for F {
    fn call_box(self: Box<Self>, arg: A) {
        (*self)(arg)
    }
}

//! A parallel radix-2 decimation-in-time fast Fourier transform.

extern crate strided; // https://crates.io/crates/strided
extern crate num; // https://crates.io/crates/num
extern crate simple_parallel;

use std::f64;
use num::complex::{Complex, Complex64};
use strided::{MutStride, Stride};

/// Writes the forward DFT of `input` to `output`.
fn fft(input: Stride<Complex64>, mut output: MutStride<Complex64>) {
    // check it's a power of two.
    assert!(input.len() == output.len() && input.len().count_ones() == 1);

    // base case: the DFT of a single element is itself.
    if input.len() == 1 {
        output[0] = input[0];
        return
    }

    // split the input into two arrays of alternating elements ("decimate in time")
    let (evens, odds) = input.substrides2();
    // break the output into two halves (front and back, not alternating)
    let (mut start, mut end) = output.split_at_mut(input.len() / 2);

    // recursively perform two FFTs on alternating elements of the input, writing the
    // results into the first and second half of the output array respectively.
    if evens.len() >= 2 {
        // run in parallel if there's enough data to be "worth it" (2
        // is just an example, to ensure that some parallelism
        // happens)
        simple_parallel::both((evens, start.reborrow()), (odds, end.reborrow()),
                              |(in_, out)| fft(in_, out));
    } else {
        fft(evens, start.reborrow());
        fft(odds, end.reborrow());
    }

    // exp(-2πi/N)
    let twiddle = Complex::from_polar(&1.0, &(-2.0 * f64::consts::PI / input.len() as f64));

    let mut factor = Complex::new(1., 0.);

    // combine the subFFTs with the relations:
    //   X_k       = E_k + exp(-2πki/N) * O_k
    //   X_{k+N/2} = E_k - exp(-2πki/N) * O_k
    for (even, odd) in start.iter_mut().zip(end.iter_mut()) {
        let twiddled = factor * *odd;
        let e = *even;

        *even = e + twiddled;
        *odd = e - twiddled;
        factor = factor * twiddle;
    }
}

fn main() {
    let a = [Complex::new(3., 0.), Complex::new(1., 0.),
             Complex::new(2., 0.), Complex::new(-1., 0.)];
    let mut b = [Complex::new(0., 0.); 4];

    fft(Stride::new(&a), MutStride::new(&mut b));
    println!("forward:\n{:?}\n->\n{:?}", a, b);
}

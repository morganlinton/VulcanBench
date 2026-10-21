//! Fast Fourier Transform sub-dialect.
//!
//! ROADMAP H2  -  FFT convolution for large kernels. This module
//! ships the fixed-size 4-point base (`fft4_complex`), arbitrary
//! power-of-two radix-2 FFT (`fft_radix2_complex`), and circular
//! convolution wrapper (`fft_convolve_circular_complex`).
//!
//! Complex values are represented as interleaved (re, im) pairs in
//! a length-`2 * N` F32 buffer. `fft4_complex` consumes a length-8
//! buffer (4 complex values) and produces a length-8 output (4
//! complex frequency bins).
//!
//! ## Why 4-point first
//!
//! The 4-point FFT is the smallest non-trivial DFT: it has
//! distinct twiddle factors (`W_4 = 1, -i, -1, i`) and exercises
//! every code path of the radix-2 butterfly (real-axis combine,
//! imaginary-axis combine, sign flip, cross-axis swap). A working
//! 4-point reference unblocks the recursive caller for N=8, 16,
//! ... powers of two; convolution then composes forward FFTs,
//! pointwise complex multiply, and inverse FFT.

mod common;
pub mod convolution;
pub mod fft4;
pub mod fft_radix2;

pub use convolution::fft_convolve_circular_complex;
pub use fft4::fft4_complex;
pub use fft_radix2::fft_radix2_complex;

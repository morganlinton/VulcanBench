#![cfg_attr(not(feature = "std"), no_std)]

//!
//! ## Crate Feature Flags
//!
//! - `std`
//!   + Default. Disabling `std` requires Rust 1.6 or later
//!   + Disabling `std` makes the crate a `#![no_std]` crate (works with core)

#[cfg(not(feature = "std"))]
extern crate core as std;

use std::marker::PhantomData;

pub use lexical::LexicalPermutation;

mod lexical;
#[macro_use]
pub mod control;

use control::ControlFlow;

/// Heap's algorithm for generating permutations, recursive version.
///
/// The recursive algorithm supports slices of any size (even though
/// only a small number of elements is practical), and is generally
/// a bit faster than the iterative version.
///
/// The closure `f` may return either `()` to simply run through all
/// permutations, or a `Control` value that permits breaking the
/// iteration early.
///
/// ```
/// use permutohedron::heap_recursive;
///
/// let mut data = [1, 2, 3, 4, 5, 6];
/// let mut permutations = Vec::new();
/// heap_recursive(&mut data, |permutation| {
///     permutations.push(permutation.to_vec())
/// });
///
/// assert_eq!(permutations.len(), 720);
/// ```
pub fn heap_recursive<T, F, C>(xs: &mut [T], mut f: F) -> C
    where F: FnMut(&mut [T]) -> C,
          C: ControlFlow,
{
    match xs.len() {
        0 | 1 => f(xs),
        2 => {
            // [1, 2], [2, 1]
            try_control!(f(xs));
            xs.swap(0, 1);
            f(xs)
        }
        n => heap_unrolled_(n, xs, &mut f),
    }
}

// TODO: Find a more parallel version with less data dependencies:
// i.e. don't swap the same items (for example index 0) every time.

/// Unrolled version of heap's algorithm due to Sedgewick
fn heap_unrolled_<T, F, C>(n: usize, xs: &mut [T], f: &mut F) -> C
    where F: FnMut(&mut [T]) -> C,
          C: ControlFlow,
{
    debug_assert!(n >= 3);
    match n {
        3 => {
            // [1, 2, 3], [2, 1, 3], [3, 1, 2], [1, 3, 2], [2, 3, 1], [3, 2, 1]
            try_control!(f(xs));
            xs.swap(0, 1);
            try_control!(f(xs));
            xs.swap(0, 2);
            try_control!(f(xs));
            xs.swap(0, 1);
            try_control!(f(xs));
            xs.swap(0, 2);
            try_control!(f(xs));
            xs.swap(0, 1);
            f(xs)
        }
        n => {
            for i in 0..n - 1 {
                try_control!(heap_unrolled_(n - 1, xs, f));
                let j = if n % 2 == 0 { i } else { 0 };
                // One swap *between* each iteration.
                xs.swap(j, n - 1);
            }
            heap_unrolled_(n - 1, xs, f)
        }
    }
}

/// Maximum number of elements we can generate permutations for using
/// Heap's algorithm (iterative version).
pub const MAXHEAP: usize = 16;

/// Heap's algorithm for generating permutations.
///
/// An iterative method of generating all permutations of a sequence.
///
/// Note that for *n* elements there are *n!* (*n* factorial) permutations.
///
/// ```
/// use permutohedron::Heap;
///
/// let mut data = vec![1, 2, 3];
/// let heap = Heap::new(&mut data);
///
/// let mut permutations = Vec::new();
/// for data in heap {
///     permutations.push(data.clone());
/// }
///
/// assert_eq!(permutations.len(), 6);
/// ```
// lock the repr since it performs the best in this order..(?)
#[repr(C)]
pub struct Heap<'a, Data: 'a + ?Sized, T: 'a> {
    data: &'a mut Data,
    // c, and n: u8 is enough range.
    // u32 performs better for n, u8 for c.
    // n: == !0 at start, 0 after first permutation is emitted
    n: u32,
    // c[x] is the counter for the (x + 1) th location
    c: [u8; MAXHEAP - 1],
    _element: PhantomData<&'a mut T>
}

impl<'a, T, Data: ?Sized> Heap<'a, Data, T>
    where Data: AsMut<[T]>
{
    /// Create a new `Heap`.
    ///
    /// ***Panics*** if the number of elements is too large (see `MAXHEAP`)
    ///
    /// The `heap_recursive` function has no length limit, but
    /// note that for *n* elements there are *n!* (*n* factorial) permutations,
    /// which gets impractical for surprisingingly small values of *n* anyway.
    pub fn new(data: &'a mut Data) -> Self {
        assert!(data.as_mut().len() <= MAXHEAP);
        Heap {
            data: data,
            c: [0; MAXHEAP - 1],
            n: !0,
            _element: PhantomData,
        }
    }

    /// Return a reference to the inner data
    pub fn get(&self) -> &Data {
        self.data
    }

    /// Return a mutable reference to the inner data
    pub fn get_mut(&mut self) -> &mut Data {
        self.data
    }

    /// Reset the permutations walker, without changing the data. It allows
    /// generating permutations again with the current state as starting
    /// point.
    pub fn reset(&mut self) {
        self.n = !0;
        for c in &mut self.c[..] { *c = 0; }
    }

    /// Step the internal data into the next permutation and return
    /// a reference to it. Return `None` when all permutations
    /// have been visited.
    ///
    /// Note that for *n* elements there are *n!* (*n* factorial) permutations.
    pub fn next_permutation(&mut self) -> Option<&mut Data> {
        if self.n == !0 {
            self.n = 0;
            Some(self.data)
        } else {
            while 1 + (self.n as usize) < self.data.as_mut().len() {
                let n = self.n as u8;
                let nu = self.n as usize;
                let c = &mut self.c;
                if c[nu] <= n {
                    // `n` acts like the current length - 2 of the slice we are permuting
                    // `c[n]` acts like `i` in the recursive algorithm
                    let j = if nu % 2 == 0 { c[nu] as usize } else { 0 };
                    self.data.as_mut().swap(j, nu + 1);
                    c[nu] += 1;
                    self.n = 0;
                    return Some(self.data);
                } else {
                    c[nu] = 0;
                    self.n += 1;
                }
            }
            None
        }
    }
}

#[cfg(feature = "std")]
/// Iterate the permutations
///
/// **Note:** You can also generate the permutations lazily by using
/// `.next_permutation()`.
impl<'a, Data: ?Sized, T> Iterator for Heap<'a, Data, T>
    where Data: AsMut<[T]> + ToOwned,
{
    type Item = Data::Owned;
    fn next(&mut self) -> Option<Self::Item> {
        match self.next_permutation() {
            None => None,
            Some(perm) => Some(perm.to_owned()),
        }
    }
}

/// Compute *n!* (*n* factorial)
pub fn factorial(n: usize) -> usize {
    (1..n + 1).fold(1, |a, b| a * b)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use control::Control;

    #[test]
    fn first_and_reset() {
        let mut data = [1, 2, 3];
        let mut heap = Heap::new(&mut data);
        let mut perm123 = vec![[1, 2, 3], [2, 1, 3], [3, 1, 2], [1, 3, 2], [2, 3, 1], [3, 2, 1]];
        assert_eq!(heap.by_ref().collect::<Vec<_>>(), perm123);

        // test reset
        heap.reset();
        // for the 1,2,3 case this happens to work out to the reverse order
        perm123.reverse();
        assert_eq!(heap.by_ref().collect::<Vec<_>>(), perm123);
    }

    #[test]
    fn permutations_0_to_6() {
        let mut data = [0; 6];
        for n in 0..data.len() {
            let count = factorial(n);
            for (index, elt) in data.iter_mut().enumerate() {
                *elt = index + 1;
            }
            let mut permutations = Heap::new(&mut data[..n]).collect::<Vec<_>>();
            assert_eq!(permutations.len(), count);
            permutations.sort();
            permutations.dedup();
            assert_eq!(permutations.len(), count);
            // Each permutation contains all of 1 to n
            assert!(permutations.iter().all(|perm| perm.len() == n));
            assert!(permutations.iter().all(|perm| (1..n + 1).all(|i| perm.iter().position(|elt| *elt == i).is_some())));
        }
    }

    #[test]
    fn count_permutations_iter() {
        let mut data = [0; 10];
        for n in 0..data.len() + 1 {
            let count = factorial(n);
            let mut permutations = Heap::new(&mut data[..n]);
            let mut i = 0;
            while let Some(_) = permutations.next_permutation() {
                i += 1;
            }
            assert_eq!(i, count);
            println!("{}! = {} ok", n, count);
        }
    }

    #[test]
    fn count_permutations_recur() {
        let mut data = [0; 10];
        for n in 0..data.len() + 1 {
            let count = factorial(n);
            let mut i = 0;
            heap_recursive(&mut data[..n], |_| i += 1);
            assert_eq!(i, count);
            println!("{}! = {} ok", n, count);
        }
    }

    #[test]
    fn permutations_0_to_6_recursive() {
        let mut data = [0; 6];
        for n in 0..data.len() {
            let count = factorial(n);
            for (index, elt) in data.iter_mut().enumerate() {
                *elt = index + 1;
            }
            let mut permutations = Vec::with_capacity(count);
            heap_recursive(&mut data[..n], |elt| permutations.push(elt.to_owned()));
            println!("{:?}", permutations);
            assert_eq!(permutations.len(), count);
            permutations.sort();
            permutations.dedup();
            assert_eq!(permutations.len(), count);
            // Each permutation contains all of 1 to n
            assert!(permutations.iter().all(|perm| perm.len() == n));
            assert!(permutations.iter().all(|perm| (1..n + 1).all(|i| perm.iter().position(|elt| *elt == i).is_some())));
        }
    }

    #[test]
    fn permutations_break() {
        let mut data = [0; 8];
        let mut i = 0;
        heap_recursive(&mut data, |_perm| {
            i += 1;
            if i >= 10_000 {
                Control::Break(i)
            } else {
                Control::Continue
            }
        });
        assert_eq!(i, 10_000);
    }
}

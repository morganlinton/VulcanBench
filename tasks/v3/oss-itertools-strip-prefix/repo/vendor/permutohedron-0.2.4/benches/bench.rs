#![feature(test)]
extern crate permutohedron;
extern crate test;

use test::{Bencher, black_box};

use permutohedron::{
    Heap,
    heap_recursive,
};

#[bench]
fn heap_iterative_7(b: &mut Bencher) {
    let mut data = [0; 7];
    b.iter(|| {
        let mut heap = Heap::new(&mut data);
        while let Some(elt) = heap.next_permutation() {
            black_box(elt[0]);
        }
    });
}

#[bench]
fn heap_iterative_7_iter(b: &mut Bencher) {
    let mut data = [0; 7];
    b.iter(|| {
        let heap = Heap::new(&mut data);
        for elt in heap {
            black_box(elt[0]);
        }
    });
}

#[bench]
fn heap_recursive_7(b: &mut Bencher) {
    let mut data = [0; 7];
    b.iter(|| {
        heap_recursive(&mut data, |elt| {
            black_box(elt[0]);
        });
    });
}

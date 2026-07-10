
permutohedron
=============

Generate permutations of sequences.

Please read the `API documentation here`__

__ https://docs.rs/permutohedron/

|build_status|_ |crates|_

.. |build_status| image:: https://travis-ci.org/bluss/permutohedron.svg?branch=master
.. _build_status: https://travis-ci.org/bluss/permutohedron

.. |crates| image:: http://meritbadge.herokuapp.com/permutohedron
.. _crates: https://crates.io/crates/permutohedron

How to use with cargo::

    [dependencies]
    permutohedron = "0.2"

Recent Changes
--------------

- 0.2.4

  - Document a panic case in ``Heap::new`` that was not mentioned before

- 0.2.3

  - Add license files to the repo and package, thanks to @ignatenkobrain

- 0.2.2

  - Add trait ``ControlFlow`` and allow optionally terminating ``heap_recursive``
    early.

- 0.2.1

  - Add feature ``std`` (enabled by default) that allows you to opt out of
    using libstd.

- 0.2.0

  - Add trait LexicalPermutation for lexicographical order of permutations,
    from Rust's libcore

- 0.1.6

  - Small optimization of heap_recursive

- 0.1.5

  - Cache-use / data layout optimizations for Heap

- 0.1.4

  - Optimizations for Heap

- 0.1.3

  - Fix an error in heap_recursive

- 0.1.1

  - Add heap_recursive, a faster callback-based version of Heap's algorithm
    that can compute permutations of a sequence.

- 0.1.0

  - Add Heap, an iterative walker & iterator using Heap's algorithm to
    compute permutations of a sequence.

License
-------

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
http://www.apache.org/licenses/LICENSE-2.0 or the MIT license
http://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.

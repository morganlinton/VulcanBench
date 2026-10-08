# yyjson mishandles incremental parsing, an iterator after removal, and zero-length writes

The workspace at `/app` is yyjson, a C JSON library built with CMake. Its
test suite builds and passes as given:

```
cmake -S . -B build -DYYJSON_BUILD_TESTS=ON -DCMAKE_BUILD_TYPE=Debug
cmake --build build -j
cd build && ctest --output-on-failure
```

Users report wrong behaviour the current tests do not cover:

- **Incremental reading (`yyjson_incr_read`)** feeds the parser one chunk of
  bytes at a time. Two inputs are mishandled when the document is split across
  chunks. A JSON document that is a bare number (for example `123`) is
  finalized as soon as the digits seen so far parse, so when more digits
  arrive in a later chunk the value is wrong; a number at the top level must
  not be treated as complete while more input may still extend it. Separately,
  a complete document followed by trailing content is accepted if the trailing
  bytes have not arrived yet; trailing non-whitespace content must be rejected
  once it is seen, not skipped because the parser stopped at a chunk boundary.
- **Mutable array and object iterators** (`yyjson_mut_arr_iter_remove`,
  `yyjson_mut_obj_iter_remove`) leave the iterator's internal predecessor
  pointer stale after a removal, so a second removal without an intervening
  step corrupts the container. Removing the current element must leave the
  iterator in a state where the next removal is either well defined or
  rejected, never acting on a stale predecessor.
- **File writers** (`yyjson_write_file` and the internal file-writing helper)
  check `fwrite`'s return value incorrectly, so a legitimate zero-length write
  is reported as a failure and, for non-zero writes, a short write may be
  misclassified. A write succeeds iff all requested bytes were written.

Fix these in `src` so the behaviour is correct for arbitrary chunk
boundaries, repeated iterator removals and all write lengths. Do not change
the public API. Do not special-case the examples above; fix the underlying
causes.

Grading copies only your `src` tree (`yyjson.c` and `yyjson.h`) into a clean
checkout that has the held-out test suite and build files, builds the test
executables, and runs them. A held-out set of executables must newly pass and
every other executable must keep passing; nothing outside `src` is graded.

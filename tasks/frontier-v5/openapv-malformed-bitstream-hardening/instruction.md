# Harden the APV decoder against malformed bitstreams

The workspace at `/app` is the OpenAPV codec library (C, CMake). Build it
with `cmake -S . -B build && cmake --build build`; the test suite is `ctest`
run from the build directory, and all of it passes today.

Our fuzzing job feeds mutated APV access units to the decoder with
AddressSanitizer and UndefinedBehaviorSanitizer enabled. It is reporting
failures across several independent parts of the bitstream parsing and
reconstruction path: out-of-bounds reads past the end of the access unit,
shifts and arithmetic that overflow their types, reads driven by
syntax-element values the spec never allows, and allocations that grow
without bound when a stream repeats an element. Every one of these should
have been a clean `OAPV_ERR_MALFORMED_BITSTREAM` (or another `OAPV_ERR_*`)
return from the public decoder API. None of the failing inputs are valid APV
streams; all of them are small.

Make the decoder robust: any malformed access unit, at any point in the
parse or reconstruction, must be rejected through the normal error-return
path with no sanitizer report, no crash and no hang. The decode of valid
streams must not change: every existing test must keep passing, and the
decoded frames of the conformance bitstreams must still match their embedded
hashes. Do not fix this by rejecting inputs the conformance streams rely on,
by disabling checks, or by catching signals.

Grading rebuilds `src/`, `inc/`, `app/` and `CMakeLists.txt` from your
workspace in a clean container with a release build plus ASan and UBSan, runs
the full upstream `ctest` suite, and then runs a held-out set of malformed
access units through the library. Files outside those four paths are not
graded. The sanitizer toolchain (clang, libFuzzer) is installed in this
container.

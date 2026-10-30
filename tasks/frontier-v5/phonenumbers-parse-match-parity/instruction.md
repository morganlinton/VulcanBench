# Port phone number parsing, as-you-type formatting and matching to match the Java reference exactly

The workspace at `/app` holds python-phonenumbers, a Python port of Google's
libphonenumber, in `/app/python/phonenumbers`. Four parts of it have been
removed and must be rebuilt:

- **parsing** a phone number string into a `PhoneNumber` (`parse`, with or
  without keeping the raw input), and `is_alpha_number`;
- **dialing-context formatting**: `format_in_original_format`,
  `format_out_of_country_calling_number`,
  `format_out_of_country_keeping_alpha_chars`,
  `format_number_for_mobile_dialing`,
  `format_national_number_with_carrier_code` and
  `format_national_number_with_preferred_carrier_code`;
- the **as-you-type formatter** (`AsYouTypeFormatter`);
- **finding phone numbers in text** (`PhoneNumberMatcher`, at every
  `Leniency`).

Right now each of these raises `NotImplementedError`, and so does anything
that depends on them (for example `is_number_match` on strings). Everything
else (formatting a `PhoneNumber` with `format_number`, validation, number
types, the metadata) works and must keep working.

The behaviour to reproduce is that of the reference implementation, Google's
Java libphonenumber v9.0.42, whose library source is in
`/app/reference/libphonenumber/java/libphonenumber/src`, with the metadata it
is generated from in `/app/reference/libphonenumber/resources`. The Python
package's own metadata modules are generated from the same files and must not
be changed. The reference is provided as source only: there is no Java
runtime in this environment. Where the reference does something surprising,
match the reference.

A visible sample of 200 cases with the reference's results is in
`/app/corpus/visible_corpus.jsonl`; `python3 /app/corpus/check_visible.py`
runs the package on it and shows mismatches (`--show N`, `--op OP`). Grading
uses a much larger hidden corpus of the same operations on many more
numbers, regions and texts, so matching the visible sample is not enough:
port the behaviour, do not special-case the examples.

Grading runs `/app/corpus/pnrun.py` (as provided; your copy is not used) on
every case against your package and compares each result string with the
reference's byte for byte. It reads, from `/app/python/phonenumbers`, only the
top-level `*.py` modules; everything else in the package (the metadata
subpackages, the `.pyi` files) is restored from the original. A case passes
only on an exact match, and every case must pass, including those that
already match today. Do not read files, start processes or exit the process
at run time, and do not touch the interpreter's import machinery: the package
must compute every result itself.

# Comparing, hashing and editing URL paths disagrees with normalization

The workspace at `/app` is Boost.URL, a C++ library for parsing and
modifying URLs. It builds against the Boost 1.92.0 sources in `/opt/boost`
and tests offline:

```
cmake -S /app -B /tmp/build -G Ninja -DBOOST_SRC_DIR=/opt/boost \
      -DBOOST_URL_BUILD_TESTS=ON -DBOOST_URL_BUILD_EXAMPLES=OFF \
      -DBUILD_TESTING=ON -DCMAKE_BUILD_TYPE=Debug
cmake --build /tmp/build --target boost_url_unit_tests
/tmp/build/test/unit/boost_url_unit_tests
```

The suite passes as given. The library promises that two URLs compare equal
exactly when their paths normalize to the same path (`url_base::normalize`),
that URLs which compare equal hash equal, and that every modifier keeps the
URL's internal sizes consistent (checked in Debug builds after each
modifier). Users report these failures, which the current tests do not
catch:

- Comparing URLs whose relative paths cancel out entirely never returns:
  `url_view(".") == url_view(".")`, `url_view("a/..") == url_view("b")`.

- `compare` disagrees with normalization. `"/a/b/.."` and `"/a/"` normalize
  to the same path but compare unequal, while `"a/b/.."` and `"x/./y/.."`,
  or `"a%2Fb"` and `"a/b"`, normalize differently but compare equal. Unequal
  paths also come out in the wrong order (`"../"` sorts after `"../g"`),
  which breaks `std::map` and `std::sort`. Paths that normalize to a
  `"./"`-prefixed relative path, such as `"a/..//b"` (normalized `".//b"`),
  compare equal to the absolute path `"/b"`.

- `std::hash<url_view>` differs for URLs that compare equal, for example
  `"/."` and `"/"`, or `"a%2Fb"` and `"a%2fb"`.

- After `url u("a/..//b"); u.normalize();`, iterating `u.encoded_segments()`
  runs past the end of the path.

- The decoded path is the wrong size after some modifiers, so `path()`
  returns the wrong string or Debug builds assert: adding an authority, user,
  password or userinfo to a URL with a relative path (`url("a/b")` with
  `set_encoded_authority("h")` gives a `path()` of `"/a/"`);
  `remove_authority()` on a path starting with `"//"`; `set_scheme()` on a
  path that needed a `"./"` prefix; and `remove_scheme()` on a path with a
  `"./"` prefix, which also leaves stray characters in the URL.

Fix the underlying causes so that comparison, hashing and the modifiers all
agree with `normalize()` and with each other, and so that everything that
works today keeps working. Do not special-case the examples above; the
grading cases cover the same rules in more combinations.

Grading copies only your `include` and `src` trees into a clean environment
that has the pristine tests, build configuration and Boost sources, then
builds and runs the unit test suite in Debug: held-out tests for these
rules, and every existing test as a regression wall. Nothing outside
`include` and `src` is graded.

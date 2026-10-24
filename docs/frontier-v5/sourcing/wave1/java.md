# F5 wave 1 sourcing: Java candidates

Date: 2026-10-24. Sourcing only; nothing was built or run. Every judgment below is
from the public record (GitHub PRs, issues, review threads, partial clones of the
repositories read under the scratchpad). The authoring step must verify by running.

Method: for 62 repositories, listed every PR merged on or after 2026-06-01
(`gh pr list --state merged --search "merged:>=2026-06-01"`, up to 400 per repo),
filtered titles for fix language, grouped the touched `src/main` files to find
clusters in one subsystem, then read the PR bodies, issue threads, review comments
and diffs of the clusters that survived. Floor 8 was judged from: first fix then
follow-up or revert, fixes that add guards for adjacent behaviour, maintainer
comments rejecting a local fix, and existing tests the PR text says had to keep
passing.

Floor 8 reminder: "the obvious fix is wrong in a way the project's own tests
detect". The strongest evidence is an upstream PR body that names the existing
tests the author had to keep green while writing the structural fix. Candidate 1
has that in writing for four separate fixes.

## (a) Ranked candidates

| Rank | Repo (licence) | Subsystem and files | Fixes in cluster (merge dates) | Interaction | Floor 8 strength | Gold (main lines) | Expert hours | Build and test | Contamination |
|---|---|---|---|---|---|---|---|---|---|
| 1 | javaparser/javaparser (dual Apache-2.0 or LGPL-3.0) | symbol solver member and method resolution: `MemberResolutionLogic` (new), `MethodCallExprContext`, `JavaParserTypeDeclarationAdapter`, `CompilationUnitContext`, `MethodResolutionLogic`, `ReferenceTypeImpl` | #5139, #5141 (2026-09-18); #5145, #5147 (2026-09-24); #5154, #5157 (2026-10-10) | strong: #5141 moves code into the class #5139 created; #5154 extends it again; #5147 and #5154 edit the same `isApplicable` region | strong, written in PR bodies (named existing tests) | 4-fix subset about +395/-141; 6-fix about +900/-410 | 8 to 16 (4-fix), 12 to 24 (6-fix) | Maven, JDK 8 API via `<release>`, builds on 17 and 21; CI build-and-test job 14 min wall; expect 8 to 12 min | merged 2026-09 to 2026-10; JLS-derived rules |
| 2 | jhy/jsoup (MIT) | HTML tokeniser and tree builder: `TokeniserState`, `Tokeniser`, `HtmlTreeBuilder`, `HtmlTreeBuilderState` | #2581 (2026-08-22), #2596 (2026-09-07), #2615 (2026-09-21), #2616 (2026-09-22), #2618 (2026-09-23), f79f21d2 (2026-09-24), 81718491 (2026-09-27), f062a8ab (2026-09-28), #2635 and #2636 (2026-09-29) | medium to strong: #2615 builds on #2581's fragment-context distinction; #2635 and #2636 rewrite adjacent tokeniser states; #2616 and #2618 rewrite the same character-insertion code | strong for #2636 (XML guard named in PR), medium elsewhere | 7-fix tokeniser and template subset about +99/-91; with #2616 and #2618 about +330/-200 | 4 to 8 (7-fix), 6 to 12 (9-fix) | Maven, Java 8 target plus multi-release 11; 1959 `@Test`; CI Build about 2 min; ITs are failsafe-only | merged 2026-08 to 2026-09; WHATWG spec and WPT derived |
| 3 | FasterXML/jackson-core (Apache-2.0) | JSON tokenizer backends: `UTF8DataInputJsonParser`, `NonBlockingUtf8JsonParserBase`, `UTF8StreamJsonParser`, shared `JsonParserBase`, `CharTypes` | #1743, #1747 (2026-10-09); #1745, #1751, #1757 (2026-10-10); #1759 (2026-10-11); #1796 (2026-10-24); plus #1781, #1785, #1788 (2026-10-20) | strong textual coupling: five PRs add helpers to `JsonParserBase`; #1759 reuses #1743's resume states; #1757 moves name decoding out of three parsers and #1781/#1785 extend it | medium: the obvious fix for #1757 was committed first and then had to be redone; #1747 changed 10 existing assertions (a complication) | DataInput plus async subset (#1745, #1751, #1757, #1759, #1796) about +1086/-983 including refactoring; minimal fix lines about 250 | 6 to 12 | `./mvnw`, parent `jackson-base` is a SNAPSHOT on every branch (must be pre-cached, run `-o`); 1103 `@Test`, about 1689 executions, 1 to 2 min | merged 2026-10; most PRs generated with Claude Code and say "found via differential fuzzing" |
| 4 | FasterXML/jackson-databind (Apache-2.0), branch 3.x | `@JsonInclude` evaluation: `ser/PropertyBuilder`, `ser/jackson/JsonValueSerializer`, `util/ArrayBuilders`, `MapperFeature` | #6283 (2026-10-21), #6318, #6322 (2026-10-22), #6323 (2026-10-23), #6297 (2026-10-21, 2.x), earlier #6076, #6102 (2026-07-17), #6089 (2026-07-10) | strong: #6318 deletes the flag #6283 added one day earlier; #6322 edits the same `buildWriter` branch | medium: a documented regression chain (#1757 feature, #6275 regression, #6283 special case, #6317 removal, #6319 found while removing); but #6318 flips an existing test | about +240/-60 for #6318 (net of #6283), #6322, #6323, #6297 | 4 to 8 | Maven, JDK 17+, parent and jackson-core are 3.3.0-SNAPSHOT (pre-cache); CI 5 to 6 min | merged 2026-07 to 2026-10 |
| 5 | FasterXML/jackson-dataformats-text (Apache-2.0), module `toml` | `TomlGenerator` value termination and string quoting | #723 (2026-09-18), #726 (2026-10-06), #746 (2026-10-06, follow-up to #726), #757 (2026-10-10, 3.x, fixes #747) | medium: #757 exists because #723 and #746 were the same bug class; #746 builds on #726 | medium to weak: follow-up chain and a mutation check, but few guards in a small class | 2.x about +80/-15; 3.x #757 +105/-97 | 2 to 4 | Maven multi-module, `-pl toml`; 924 TOML tests; SNAPSHOT parent | merged 2026-09 to 2026-10; Claude Code PRs |
| 6 | ThreeTen/threeten-extra (BSD-3-Clause) | amount classes and formats: `AmountFormats`, `Days`/`Hours`/`Minutes`/`Months`/`Seconds`/`Weeks`/`Years`, `HourMinute`, `Interval` | #420 (2026-10-22); #422, #423 (2026-10-23); #424, #426, #427, #428, #429, #430 (2026-10-24) | medium: #427 wraps the overflow #426-style checks raise; #428 tightens the regexes #427 touched; #423 and #424 rewrite the same methods | weak: no revert or follow-up; guards are the parameterized valid-parse tables | about +260/-110, much of it duplicated across seven classes | 2 to 4 | Maven, JDK 11+; 13,000+ test executions in about 1 min; CI 3 to 4 min | merged 2026-10-22 to 2026-10-24 by the owner |

### Candidate 1: javaparser symbol solver

Repository: https://github.com/javaparser/javaparser (LICENSE: "available either
under the terms of the LGPL License or the Apache License").

Fixes, each with its own regression test and no change to existing tests unless
noted:

| PR | Merged | Title | Main files | Regression tests added |
|---|---|---|---|---|
| https://github.com/javaparser/javaparser/pull/5139 | 2026-09-18 | resolve qualified calls against members only (fixes #5105) | `MemberResolutionLogic` (+155, new), `MethodCallExprContext` (+45/-1), `JavaParserTypeDeclarationAdapter` (+4/-33), `EnumDeclarationContext` (+8), five `JavaParser*Declaration` classes | `Issue5105Test` plus `resources/issue5105/qq/*.java`; commits a9eed9999, 15bf16123, 7bd6057ac |
| https://github.com/javaparser/javaparser/pull/5141 | 2026-09-18 | resolve qualified nested types against members only (fixes #5140) | `CompilationUnitContext` (+10/-29), `JavaParserTypeDeclarationAdapter` (+3/-49), `MemberResolutionLogic` (+83) | `Issue5140Test` plus `resources/issue5140`; commits 6e846b477, 05fc8e3cd, 24ccbcec6 |
| https://github.com/javaparser/javaparser/pull/5145 | 2026-09-24 | tolerate unresolvable ancestors at any depth (fixes #4985) | `ReferenceTypeImpl` (+38/-2), `ResolvedReferenceType` (+17/-4) | `Issue4985Test` plus `resources/issue4985` |
| https://github.com/javaparser/javaparser/pull/5147 | 2026-09-24 | primitive arrays are invariant in method resolution (fixes #4991) | `MethodResolutionLogic` (+15/-7); commit 1e70c7182 | `Issue4991Test` |
| https://github.com/javaparser/javaparser/pull/5154 | 2026-10-10 | resolve overloads that only differ by type variables of their declaring type (fixes #4628) | `MethodResolutionLogic` (+218/-205), `MemberResolutionLogic` (+178/-12), `AbstractJavaParserContext` (+47/-45), `MethodResolutionCapability` (+11), declarations; commits 8299c4a5a, 518d5899f, 3ac6e2dd8 | `Issue4628Test` |
| https://github.com/javaparser/javaparser/pull/5157 | 2026-10-10 | skip type variables matched against themselves (fixes #4625) | `MethodCallExprContext` (+12/-1) | `Issue4625Test` |
| optional: https://github.com/javaparser/javaparser/pull/5126 | 2026-09-09 | resolve a type's own type parameters before the enclosing scope | `JavaParserTypeDeclarationAdapter` (+10/-9) | `JavaParserTypeDeclarationAdapterTest` (+57) |
| exclude: https://github.com/javaparser/javaparser/pull/5159 | 2026-10-10 | least upper bound per JLS 4.10.4 (fixes #5158) | `LeastUpperBoundLogic` (+204/-46) and 6 others | changes two existing `LeastUpperBoundTest` expectations, so it conflicts with visible tests |

Interaction evidence: the file history of
`javaparser-symbol-solver-core/.../logic/MemberResolutionLogic.java` is exactly
this cluster: a9eed9999 and 15bf16123 (#5139), 6e846b477, 05fc8e3cd and
24ccbcec6 (#5141), 518d5899f and 3ac6e2dd8 (#5154). #5141 does not apply without
#5139 (it moves `checkAncestorsForType` into the class #5139 created). #5154 and
#5147 both edit `MethodResolutionLogic.isApplicable`; #5154's first commit
8299c4a5a removes the "unused declaring-type substitution" that #5147 left in
place.

Floor 8 evidence (quotes are from the PR bodies):

- #5139: "Two adjustments the test suite required: a call without a receiver is a
  lexical lookup and must keep reaching its own file's imports, so
  `MethodCallExprContext` sends those to the enclosing scopes instead of to the
  declaration of the type of `this` -- `Issue3200Test`, `Issue4047Test`,
  `Issue4358Test` and `CompilationUnitContextResolutionTest` pass unmodified;
  `values()` and `valueOf(String)` are members no AST node declares (JLS 8.9.3),
  so `EnumDeclarationContext` asks the declaration before the adapter." All five
  named tests exist at the pre-fix base a9eed9999^, as does
  `resolution/EnumResolutionTest`.
- #5141: "The ancestors are still searched, so a nested type inherited from a
  superclass keeps resolving -- that was the stated reason for going through the
  context, and `JavaParserTypeDeclarationAdapterTest#issue3550` covers it."
  Present at the base (line 78 of that test).
- #5147: "Component-wise boxing and widening only applies to the last parameter
  of a variable arity method, where the trailing arguments are grouped into an
  array (JLS 15.12.2.4), which keeps #4890 working." The #4890 guard is
  `resolution/MethodsResolutionLogicTest.testVariadicBoxedToPrimitive`
  (`print(Integer... values)` called with `print(1, 2, 3)`), present at base
  1e70c7182^. Issue https://github.com/javaparser/javaparser/issues/4991 is itself
  a regression report ("fails in 3.28.0 ... does not in 2.27.1") caused by the
  #4890 fix, so the cluster already contains one fix-then-regress pair.
- #5145: "Type inference (least upper bound, bound sets) still walks the
  hierarchy strictly." and "when they cannot, the original
  `UnsolvedSymbolException` is still raised, so no wrong negative answer is ever
  returned", pinning two adjacent behaviours a blanket "ignore unresolvable
  ancestors" would change.
- #5154: the maintainer's comment on https://github.com/javaparser/javaparser/issues/4628
  (jlerbsc, 2026-10-10) rejects the reporter's local fix in `isMoreSpecific`: "A
  rule that prefers `setId(T)` because its bound (`GenericClass`) is more
  specific than `Object` would select `setId(T)` for **both** calls: right for
  the first one by chance, silently wrong for the second one, where javac selects
  `setId(U)`."

Build and test: Maven reactor, `java.version` 1.8 compiled with `<release>`, CI
runs on JDK 8 through 21. The symbol-solver module regenerates nothing; `javaparser-core`
runs JavaCC from a cached plugin. `javaparser-symbol-solver-testing` has 1959
`@Test` annotations and 329 resource files (five small jars, no network);
`javaparser-core-testing` has 2803. CI job "Build and test (using maven)" took 14
min wall on 2026-10-23 (run 39989385964) including setup, so a container run of
`-pl javaparser-symbol-solver-testing -am` should be 8 to 12 min. Surefire excludes
`SlowTest` unless the `AlsoSlowTests` profile is active.

Gold and hours: 4-fix subset (#5139, #5141, #5145, #5147) about +395/-141 main
lines, 8 to 16 expert hours from symptoms (the symbol solver's context versus
declaration distinction is the hard part); 6-fix about +900/-410, 12 to 24 hours.

Contamination: all merged 2026-09-18 to 2026-10-10. The rules come from the JLS,
which models know; the implementation decisions (context versus declaration,
`MemberResolutionLogic`) are project specific. Test class names embed issue
numbers.

### Candidate 2: jsoup HTML tokeniser and tree builder

Repository: https://github.com/jhy/jsoup (MIT). All fixes by the maintainer (jhy),
driven by the WHATWG spec and WPT tree-construction tests.

| Change | Merged | Title | Main files | Regression tests added | Existing tests changed |
|---|---|---|---|---|---|
| https://github.com/jhy/jsoup/pull/2581 (cad054a6) | 2026-08-22 | Template stack checks need to exclude the fragment context | `HtmlTreeBuilder` (+6), `HtmlTreeBuilderState` (+2/-2) | `HtmlParserTest.templateFragmentContextCannotBeClosedByInputAtEof`, `templateFragmentCanCloseNestedTemplate` | none |
| https://github.com/jhy/jsoup/pull/2596 (cd22c55b) | 2026-09-07 | Decode named entities before - and _ in attributes (fixes #2588) | `Tokeniser` (+2/-2) | `HtmlParserTest` (+33), `ParserTest` (+13) | none |
| https://github.com/jhy/jsoup/pull/2615 (1eb02ea3) | 2026-09-21 | Fix template parse for EOF and forms (fixes #2614; 16 WPT `template.dat` failures) | `HtmlTreeBuilder` (+8/-7), `HtmlTreeBuilderState` (+14/-20) | `nestedTemplatesCloseAtEof`, `deeplyNestedTemplatesCloseAtEof`, `tableModeEofClosesTemplate`, `formInTableUsesTemplateRules`, `formsInTemplateFragmentIgnoreFormPointer`, `formEndInTemplateFragmentUsesScope` | none |
| https://github.com/jhy/jsoup/pull/2616 (fcdb20f8) | 2026-09-22 | Align HTML null and initial newline handling with the spec | `Tokeniser` (+30/-2), `TokeniserState` (+16/-4), `HtmlTreeBuilderState` (+16/-16), `Token` (+7/-6), `Printer` (+12), `TagSet` (+1/-1) | `CharacterTokenTest` (new, 86), `HtmlTextHandlingTest` (new, 170) | 2 assertions in `HtmlParserTest` (textarea initial LF) |
| https://github.com/jhy/jsoup/pull/2618 (387c6e12) | 2026-09-23 | Fixed HTML parser edge cases and streamlined node insertion | `HtmlTreeBuilder` (+104/-67), `HtmlTreeBuilderState` (+26/-19), `TokeniserState` (+1) | `fostersCharactersBeforeTable`, `retainsWhitespaceFromMixedFramesetText`, `<!doctypehtml>` case, `HtmlTreeBuilderTest` foster tests | `PositionTest` order expectations (+5/-4), `HtmlTreeBuilderTest` API rename |
| f79f21d2 (direct commit) | 2026-09-24 | Double-escaped script tags in body were being closed too early (fixes #2627) | `HtmlTreeBuilderState` (+3/-3), `TokeniserState` (+3/-2) | `HtmlTreeBuilderTest` (+14), `TokeniserStateTest` (+9) | none |
| 81718491 (direct commit) | 2026-09-27 | Consume the `/` from a malformed end tag (fixes #2631) | `TokeniserState` (-1) | `TokeniserStateTest` (+13) | 1 line in `HtmlParserTest` |
| f062a8ab (direct commit) | 2026-09-28 | Correct fragment ignores (fixes #2634) | `HtmlTreeBuilderState` (+6/-4) | `HtmlParserTest` (+13) | none |
| https://github.com/jhy/jsoup/pull/2635 (1aa1234e) | 2026-09-29 | Cleanup malformed doctype tokenisation | `TokeniserState` (+36/-17) | `TokeniserStateTest` (+66), `HtmlParserTest` (+8) | none |
| https://github.com/jhy/jsoup/pull/2636 (0dc96b3b) | 2026-09-29 | Clean up comment and incomplete tag tokenisation | `TokeniserState` (+16/-35) | `TokeniserStateTest.commentStartDashRetainsHyphen`, `eofBeforeAttributeValueDropsTag`, `missingAttributeValueReportsGreaterThan`, `scriptLessThanAtEofReportsOnlyUnclosedScript` | `dropsUnterminatedAttribute` rewritten as parameterized (same meaning) |

Interaction evidence: #2615 introduces
`isParsingTemplateContents() { return onStack("template") || fragmentContextIs("template"); }`
and replaces every `onStack("template")` form check, which only makes sense on top
of #2581's `onStackAboveContext` distinction; #2615's
`formsInTemplateFragmentIgnoreFormPointer` test depends on the fragment context
surviving EOF, which #2581 established. #2635 and #2636 convert adjacent tokeniser
states from consume-then-unconsume to current-then-advance and share the bogus
DOCTYPE and comment transitions. #2616 and #2618 both rewrite the `Character`
branches of `InBody`, `InTableText` and `InFrameset` in `HtmlTreeBuilderState`.

Floor 8 evidence:

- #2636 body: "I kept the XML parser's existing recovery for a tag ending after
  `=` at EOF as we had specific coverage for it and don't see anything in that
  spec either way." The guard is `XmlTreeBuilderTest.testDoesHandleEOFInTag`
  (input `<img src=asdf onerror="alert(1)" x=`, expects
  `<img src="asdf" onerror="alert(1)" x=""></img>`), present at base 1aa1234e.
  The shipped code branches on syntax:
  `if (t.syntax == xml && !t.attributeFragment) t.emitTagPending(); else t.emit(new Token.EOF());`.
- #2581 and #2615: #2581's `templateFragmentContextCannotBeClosedByInputAtEof`
  (content `</template><p><template>` set on a template context must yield
  `<p><template></template></p>`) is visible at #2615's base and forbids the
  obvious "pop every template at EOF" loop from popping the synthetic context.
  #2581 itself adds `templateFragmentCanCloseNestedTemplate`, pinning that the
  narrower fix must not stop nested templates closing. Older guards:
  `templateFragment` (#1315) and the `template.dat` derived tests in
  `HtmlParserTest`.
- #2616: `HtmlParserTest.preSkipsFirstNewline` (line 1807 at base fcdb20f8^;
  `<pre>\n\nOne\nTwo\n</pre>` must keep `"\nOne\nTwo\n"`) guards the initial
  newline rule, and the PR had to add `Printer.addHead` compensation (emit one LF
  when a `pre`, `listing` or `textarea` first text starts with LF or CR) so that
  serialising and reparsing stays stable; the new `HtmlTextHandlingTest` pins the
  round trip.
- #2596 (maintainer's issue reply, https://github.com/jhy/jsoup/issues/2588,
  2026-09-07): the fix must make `&copy-` decode in attributes "while preserving
  the text behavior and the required protection for `&timestamp=` and `&param`",
  behaviours covered by the existing entity tests in `HtmlParserTest` and
  `EntitiesTest`.

Build and test: Maven, no parent, Java 8 target with a multi-release 11 layer;
surefire runs unit tests; `ConnectIT`, `SessionIT`, `ParserSoakIT` are failsafe
only and excluded by group `long-running`; `ConnectTest`, `SessionTest`,
`ProxyTest` use a local Jetty or Netty test server, no outbound network. 1959
`@Test` annotations; CI "Build" job about 2 min on 2026-10-23.

Gold and hours: 7-fix tokeniser and template subset (#2581, #2615, #2635, #2636,
f79f21d2, 81718491, f062a8ab) about +99/-91 main lines, 4 to 8 expert hours
(the HTML spec shortens the search but jsoup's state machine conventions do not);
adding #2616 and #2618 about +330/-200 and 6 to 12 hours, at the cost of changing
visible assertions.

Contamination: merged 2026-08-22 to 2026-09-29. The target behaviour is the
WHATWG parsing spec and html5lib or WPT expectations, which models know well, so
the task's difficulty is in the jsoup implementation rather than in knowing the
right answer.

### Candidate 3: jackson-core JSON tokenizer backends

Repository: https://github.com/FasterXML/jackson-core (Apache-2.0). Branches: the
first five fixes landed on `2.21` and were merged forward to `2.x`; the rest on
`2.x`. 2.x commit ids given.

| PR | Merged | Base | Title | Main files | Regression tests |
|---|---|---|---|---|---|
| https://github.com/FasterXML/jackson-core/pull/1743 (466964ed) | 2026-10-09 | 2.21 | async parser not resuming after split in escaped surrogate pair in property name (#1742) | `NonBlockingUtf8JsonParserBase` (+311/-157), `NonBlockingJsonParserBase` (+6) | `AsyncSplitSurrogateInFieldName1742Test`, `AsyncEOFMessagesTest`, `AsyncInvalidCharsTest`, `AsyncCommentParsingTest` (+38) |
| https://github.com/FasterXML/jackson-core/pull/1747 (1e7e0ca6) | 2026-10-09 | 2.21 | accept leading-decimal-point numbers after `,`/`:` in async parser (#1746), plus number alignment | `NonBlockingUtf8JsonParserBase` (+197/-142), `JsonParserBase` (+20), three blocking parsers | `AsyncLeadingDecimalPoint1746Test`, `AsyncSignedNumberTest`, `AsyncLeadingZeroSignTest`, `AsyncNumberDotWithoutDigitsTest`, `AsyncExponentAtEOFTest`, `AsyncRootNumberSeparationTest`, `NumberDotWithoutDigitsTest`; changes 10 assertions in `AsyncNonStandardNumberParsingTest` |
| https://github.com/FasterXML/jackson-core/pull/1745 (026686f7) | 2026-10-10 | 2.21 | escaped surrogate pairs in property names in `UTF8DataInputJsonParser` (#1744) | `UTF8DataInputJsonParser` (+123/-68), `UTF8StreamJsonParser` (+72/-112), `ReaderBasedJsonParser` (+57) | `EscapedSurrogateInFieldName1744Test` (ALL_MODES) |
| https://github.com/FasterXML/jackson-core/pull/1751 (fecef69a) | 2026-10-10 | 2.21 | honour `ALLOW_UNESCAPED_CONTROL_CHARS` in single-quoted string values (#1750) | `JsonParserBase` (+44), four parsers | `AposUnescapedControlChars1750Test` (ALL_MODES), async variant |
| https://github.com/FasterXML/jackson-core/pull/1757 (3b4a611c) | 2026-10-10 | 2.x | allow non-ASCII unquoted property names in UTF-8 byte parsers (#1748) | `CharTypes` (+2/-1), `JsonParserBase` (+248), `UTF8DataInputJsonParser` (+17/-157), `UTF8StreamJsonParser` (+11/-149), `NonBlockingJsonParserBase` (+10/-137) | `UnquotedNonAsciiNames1748Test`, `UnquotedNameSymbolCollision1748Test` |
| https://github.com/FasterXML/jackson-core/pull/1759 (5f26351d) | 2026-10-11 | 2.x | decode multi-byte UTF-8 char after backslash in async parser (#1756) | `NonBlockingUtf8JsonParserBase` (+67/-31), `JsonParserBase` (+44), two blocking parsers | `AsyncEscapedMultiByteChar1756Test`, `EscapedMultiByteChar1756Test` |
| https://github.com/FasterXML/jackson-core/pull/1796 (233fa488) | 2026-10-24 | 2.x | reject control characters between tokens in `UTF8DataInputJsonParser` (#1795) | `UTF8DataInputJsonParser` (+244/-210) | `ControlCharBetweenTokens1795Test` (ALL_MODES) |
| also https://github.com/FasterXML/jackson-core/pull/1781, /1785, /1788 | 2026-10-20 | 2.x | overlong UTF-8 in String values; 0xFF lead byte in last quad; non-ASCII hex digits in `\u` | `JsonParserBase`, `CharTypes`, the byte parsers | `UTF8Overlong1782Test`, `PropertyName0xFF1784Test`, `HexEscapeNonAscii1787Test` and async variants |

Interaction evidence: #1759's body: "This works with the existing
`MINOR_VALUE_STRING_ESCAPE` / `MINOR_FIELD_NAME_ESCAPE` states and the
low-surrogate resume path, so no new minor states are needed" (that path is
#1743's) and "matching the blocking parsers (#1744 / #1745)". #1757 commit
affd563b0 "Refactor UTF-8 name decoding into `JsonParserBase`" removes about 115
lines from each of three parsers, and #1781 and #1785 then extend that shared
code. #1745, #1747, #1751, #1757 and #1759 all add methods to `JsonParserBase`,
so the patches conflict textually and must be composed by hand. The #1795 thread
(pjfanning, 2026-10-23) widened the fix scope after the report: "`_skipColon2()`
has the same unchecked `else` branch ... A fix should cover all four".

Floor 8 evidence:

- #1757: the first PR commit 9b738d1e3 is the obvious fix, changing
  `Arrays.fill(table, 128, 128, 0)` to `Arrays.fill(table, 128, 256, 0)` with a
  test. It was followed by c10e7ad61 "Validate name chars for unquoted names",
  affd563b0 (the refactor), d731a510d adding `UnquotedNameSymbolCollision1748Test`
  whose header reads "Unquoted names must not share symbol table entries with
  quoted names that have leading `\u0000` escapes (see [core#148] for quoted
  names)", and b050b28fd "More fixes". The obvious fix routes raw high bytes into
  the quad-hashing name path without validation, so an unquoted `ab` and a quoted
  `"\u0000\u0000ab"` canonicalise to the same symbol. Caveat: that guard is new in
  the PR, not pre-existing; the pre-existing pin is `ExceptionsTest` line 115
  (`verifyException(e, "in field name")`), which the message change in the first
  commit was written to satisfy.
- #1766 maintainer comment (cowtowncoder, 2026-10-14,
  https://github.com/FasterXML/jackson-core/issues/1766): "stream read and write
  constraints are meant to protect against BIG things, not small -- nor are limits
  expected to have high precision. But they are expected to be conservative in
  only applying if length is certain to exceed (never throw for below-max-length
  cases)." This pins the direction any length-limit fix may err in; the existing
  `LargeNumberReadTest` and `AsyncLargeNumberReadTest` are the guards.
- #1764, #1773, #1774 (2026-10-14, branch 3.2): a hex-length fix was merged,
  reverted ("Accidentally merged before review, need to revert, re-do") and
  re-merged unchanged. Procedural, not a test failure; recorded so nobody counts
  it as floor-8 evidence.
- Weakness: `NonStandardAllowRSTest` at the pre-cluster base cdaaa4b0 exercises
  String, Reader, byte[] and async parsers but not `DataInput`, so the record
  separator rule that #1796 preserves had no pre-existing DataInput guard; the
  guard is the new ALL_MODES test. #1747 changed existing expectations
  (`+123` text becomes `123`, `-0.123` becomes `-.123`, six tests gain a token
  before the failure), so a task including #1747 needs a base after it or must
  drop those assertions.

Build and test: `./mvnw verify`, about 1689 test executions (per #1759), 1103
`@Test` annotations at base, CI "Build and Deploy Snapshot" 2 to 3 min. The parent
POM is `jackson-base` 2.23.0-SNAPSHOT on 2.x and 2.21.9-SNAPSHOT on 2.21, so the
pre-warmed repository must hold the snapshot parent and Maven must run offline
(`-o`) to avoid update checks. JDK 8 API; builds on 17 and 21.

Gold and hours: DataInput plus async subset (#1745, #1751, #1757, #1759, #1796)
about +1086/-983 main lines because the gold refactors heavily; the minimal fix
footprint is about 250 lines. 6 to 12 expert hours; the async split-state machine
and the quad-based symbol table are the time sinks.

Contamination: merged 2026-10-09 to 2026-10-24; most PR bodies end with
"Generated with Claude Code" and say "Found via differential fuzzing of the parser
backends", so a model may recognise the style; the inputs are post-cutoff.

### Candidate 4: jackson-databind inclusion semantics

Repository: https://github.com/FasterXML/jackson-databind (Apache-2.0), branch
3.x unless noted.

| PR | Merged | Title | Main files | Tests |
|---|---|---|---|---|
| https://github.com/FasterXML/jackson-databind/pull/6283 | 2026-10-21 | Preserve explicit nulls for per-type NON_DEFAULT inclusion (fixes #6275) | `PropertyBuilder` (+26/-12), adds `_suppressNullsOnRealDefaults` | `JsonInclude1757Test` (+40) |
| https://github.com/FasterXML/jackson-databind/pull/6318 | 2026-10-22 | global default for `NON_NULL` (fixes #6317) | `PropertyBuilder` (+15/-34), removes that flag; `MapperFeature` | `JsonInclude1757Test` (+72/-35), flips `testGlobalNonDefaultSuppressesNull` to `globalNonDefaultWritesExplicitNull` |
| https://github.com/FasterXML/jackson-databind/pull/6322 | 2026-10-22 | global `NON_DEFAULT` for non-instantiable types (fixes #6319) | `PropertyBuilder` (+5/-3) | `JsonInclude1757Test` (+120) |
| https://github.com/FasterXML/jackson-databind/pull/6323 | 2026-10-23 | `@JsonValue` honors inclusion config override of the value type (fixes #5380; alternative to #5655) | `JsonValueSerializer` (+105/-9) | `JsonValueInclusion5380Test` (293) |
| https://github.com/FasterXML/jackson-databind/pull/6297 (2.x) | 2026-10-21 | `NON_DEFAULT` omits a changed array when the default contains `null` (fixes #6295) | `ArrayBuilders` (+2/-4) | `JsonIncludeArrayTest`, `ArrayBuildersTest` |
| https://github.com/FasterXML/jackson-databind/pull/6076 | 2026-07-17 | `@JsonValue` should honor `@JsonInclude` on annotated accessor (fixes #4762) | `JsonValueSerializer` (+91/-5) | `JsonValueIgnoresJsonInclude4762Test` promoted out of `tofix/` |
| https://github.com/FasterXML/jackson-databind/pull/6102 | 2026-07-17 | omit NON_EMPTY Map when CUSTOM content filter removes all entries (fixes #6101) | `MapSerializer` (+2/-2) | `JsonIncludeMapContentFilter6101Test` |
| https://github.com/FasterXML/jackson-databind/pull/6089 | 2026-07-10 | `APPLY_JSON_INCLUDE_FOR_CONTAINERS` does not fully remove empty collection (fixes #6065) | 11 container serializers | `JsonIncludeContainerEmpty6065Test` (354) |

Interaction and floor 8: the chain is documented in the issues. #1757 (feature,
3.2.0) made the real-default branch force `suppressNulls = true`; #6275 (2026-10-13)
reports that per-type `@JsonInclude(NON_DEFAULT)` lost explicit nulls as a result
("3.0.4: {"mode":null} 3.2.0: {}"); #6283 fixed it with a special-case flag; the
maintainer opened #6317 the same day the flag merged: "remove
`PropertyBuilder._suppressNullsOnRealDefaults` (added in #6283) and the
special-casing; flip `testGlobalNonDefaultSuppressesNull` to expect
`{"someFieldWithDefault":null}`", and found #6319 while doing it ("that check is
gated on `!_useRealPropertyDefaults` rather than on whether real defaults were
actually used"). The maintainer also wrote on #6275: "the reasoning/logic of how
`NON_DEFAULT` has been convoluted and a major PITA". #6323 is "alternative to
#5655, without re-introspection in `PropertyBuilder`"; the bot review of #5655
flags the regression the simpler approach caused ("Config override on the
@JsonValue wrapper type is now silently ignored ... That is a behavior regression
for existing users"). Weakness for a composed task: #6318 changes a visible
expectation, so the base must be after #6283 with that one test dropped, or the
cluster should be #6322, #6323, #6297 plus the July fixes.

Build and test: Maven, JDK 17+, parent `jackson-base` 3.3.0-SNAPSHOT and
`jackson-core` 3.3.0-SNAPSHOT (both must be pre-cached); CI "Build and Deploy
Snapshot" 5 to 6 min. Gold about +240/-60 main lines; 4 to 8 expert hours.

### Candidate 5: jackson-dataformats-text TomlGenerator

Repository: https://github.com/FasterXML/jackson-dataformats-text (Apache-2.0),
module `toml`.

- https://github.com/FasterXML/jackson-dataformats-text/pull/723 (2026-09-18, 2.21):
  `writeNumber(short)` writes an extra empty line (`TomlGenerator` +2/-1,
  `TomlGeneratorTest` +16).
- https://github.com/FasterXML/jackson-dataformats-text/pull/726 (2026-10-06, 2.21):
  `writeString(char[],int,int)` reads `text[offset + len]` instead of
  `text[offset + i]` (+1/-1, test +28).
- https://github.com/FasterXML/jackson-dataformats-text/pull/746 (2026-10-06, 2.x):
  "Fix remaining `TomlGenerator` String write issues (follow-up to #726)": null
  array, range validation, `writeUTF8String()` double terminator, wrong error
  message (+76/-12, test +204).
- https://github.com/FasterXML/jackson-dataformats-text/pull/757 (2026-10-10, 3.x,
  fixes #747): centralize value termination behind a `_valueLineOpen` flag and
  share quoting (+105/-97, test +106, `valueLinesTerminatedOnce` over 26 writes).

Floor 8 evidence: issue https://github.com/FasterXML/jackson-dataformats-text/issues/747
(maintainer, 2026-10-06): "This has caused the same kind of bug several times ...
Any new override, delegation between public methods, or newly inherited
`GeneratorBase` value method can repeat this." #757: "Verified locally that output
stays correct (test still passes) with the terminator deliberately removed from
`writeBoolean()` and duplicated in `writeNumber(int)`." Weakness: the obvious
fixes (delete the duplicate `writeValueEnd()` call, fix the index) do pass, and
the class has few adjacent guards; floor 8 here would rest on round-trip tests in
`TomlGeneratorTest` and on the `writeRawValue()` terminator case.

Build and test: Maven multi-module; `-pl toml -am` builds `toml` alone; 924 TOML
tests; parent is a SNAPSHOT. Gold 2.x about +80/-15 (3.x #757 +105/-97); 2 to 4
expert hours.

### Candidate 6: ThreeTen-Extra amount classes

Repository: https://github.com/ThreeTen/threeten-extra (BSD-3-Clause). All by the
owner (jodastephen), 2026-10-22 to 2026-10-24, main branch, commits fb231be,
efcd492, 4a7bf6d, 2fc799b, a363d94, 93827c8, 212bab9, add6ea7, 4b472a3.

- #420 Interval and LocalDateRange parse of strings starting with `/` throw
  `DateTimeParseException` (+6 main).
- #422 long overflows in `AmountFormats.parseUnitBasedDuration()` (scale overflow
  and intermediate product; `BigInteger` fallback) (+14/-2).
- #423 long overflows in `wordBased` printing (`int[]` to `long[]`, `IntPredicate`
  to `LongPredicate`) (+24/-24).
- #424 negative durations in `wordBased` printing ("-1 second and -500
  milliseconds" rather than "-2 seconds and 500 milliseconds"; the PR says this
  "could be considered a breaking change") (+60/-9).
- #426 `dividedBy(-1)` on seven amount classes overflows silently (+3 each).
- #427 parse overflow in the seven classes throws `DateTimeParseException` not
  `ArithmeticException` (+2 to +25 each).
- #428 trailing `T` ("P1DT") rejected via lookahead `(?:T(?=[-+]?[0-9])` in
  `Hours`, `Minutes`, `Seconds` (+1 each).
- #429 `HourMinute.plus/minus(Duration)` (+64/-43).
- #430 `Interval` parsing near `Instant.MAX`, exception types, empty interval
  containment (+16/-6).

Interaction: #427 exists because #422 and #426 style exactness checks surface
`ArithmeticException` through `parse`; #428 edits the regexes #427 wrapped; #423
and #424 rewrite the same three `wordBased` methods. Floor 8: weak. No fix
changed an existing test; the guards are the parameterized valid-parse tables
(`test_parse_CharSequence_valid`) that a careless regex tightening for #428 would
break, and `duration_wordBased` rows such as `Duration.ofSeconds(Long.MAX_VALUE)`.
Separately, https://github.com/ThreeTen/threeten-extra/pull/385 (2026-09-27,
`InternationalFixedDate.ofEpochDay`) states "The naive fix does not work, and
that is the argument for the loop. Setting `doy = DAYS_IN_YEAR + 1` unconditionally
passes every row of the sample table ... and is caught only by the loop: it throws
on year 69", but it is a single fix in the `chrono` package with only #396
(Symmetry calendars, 2026-10-22) nearby, so it does not make a cluster.

Build and test: Maven, Java 11 source profile, 13,000+ executions in about 1 min,
CI 3 to 4 min. Gold about +260/-110 with heavy duplication; 2 to 4 expert hours.

## (b) Predicted naive fix and the guard it should break

Candidate 1, javaparser. Symptom: `Mid.ping()` resolves to `qq.Sink.ping()` because
`Mid.java` has `import static qq.Sink.*`; `Mid.Nested` resolves to `Sink.Nested`;
`Arrays.copyOf(int[], 3)` is ambiguous; `setId(T, String)` versus `setId(U, String)`
is ambiguous; a missing grand-ancestor interface fails every call. The naive fix a
competent reader writes first is to make every method lookup on a
`JavaParser*Declaration` members-only (stop at the type, never consult the
compilation-unit context), and to stop the boxing fallback comparing arrays
component-wise. The first breaks unqualified calls that legitimately reach their
own file's static imports (`Issue3200Test`, `Issue4047Test`, `Issue4358Test`,
`CompilationUnitContextResolutionTest`) and enum `values()`/`valueOf()`
(`EnumResolutionTest`, `Issue4047Test`), and the nested-type variant breaks
inherited member types (`JavaParserTypeDeclarationAdapterTest.issue3550`). The
second breaks `MethodsResolutionLogicTest.testVariadicBoxedToPrimitive`
(`print(Integer...)` with `print(1, 2, 3)`) unless the component-wise rule is kept
for the last parameter of a variable-arity method. For the ancestor fix, the
obvious "ignore any unresolvable ancestor everywhere" would change the
type-inference tests that rely on strict walking and would return wrong negative
`isAssignableBy` answers instead of the `UnsolvedSymbolException` existing tests
expect.

Candidate 2, jsoup. Symptoms: `<p id=` at EOF emits a paragraph (spec drops it);
`<!---x` loses the dash; `</template>` inside a template fragment throws
`ValidationException`; unclosed nested templates are not closed at EOF; forms in a
template capture inputs outside it; `<!DOCTYPE >Hello` swallows `Hello`. The naive
fix for the attribute case is to make `BeforeAttributeValue` on EOF emit an EOF
token and drop the pending tag for every tokeniser, which breaks
`XmlTreeBuilderTest.testDoesHandleEOFInTag` (XML input `... x=` must still produce
`x=""`). The naive fix for the template fragment is to pop every `template` on the
stack at EOF or on `</template>`, which pops the synthetic fragment context and
breaks `templateFragmentContextCannotBeClosedByInputAtEof` (and the older
`templateFragment` tests); the shipped code distinguishes `onStackAboveContext`
and `isParsingTemplateContents`. For the initial-newline rule, stripping the LF
in the tree builder for `textarea` the way `pre` did breaks the textarea
round-trip assertion in `HtmlParserTest` (and would break
`HtmlTextHandlingTest` once it exists) unless the Printer compensates.

Candidate 3, jackson-core. Symptoms: `{été:1}` fails in byte parsers with
`ALLOW_UNQUOTED_FIELD_NAMES`; `[1,\x01 2]` is accepted by the DataInput parser;
`['a\tb']` is rejected with both lenient features on; escaped surrogate pairs in
names fail from `DataInput`; `[1, .5]` fails in the async parser. The naive fix
for the first is the one-line `Arrays.fill(table, 128, 256, 0)`; upstream
committed exactly that first and then had to add name-character validation and a
symbol-table collision fix because raw high bytes then flow into the quad hashing
path and an unquoted `ab` canonicalises like `"\u0000\u0000ab"`
(`UnquotedNameSymbolCollision1748Test`; the pre-existing guard that constrains the
error message is `ExceptionsTest`, "in field name"). The naive fix for control
characters is to reject every byte below 0x20 other than tab, LF and CR in the
three named skip methods, which misses `_skipColon2()` (the maintainer widened the
scope in #1795) and must keep `ALLOW_RS_CONTROL_CHAR` working between root values
only, as `NonStandardAllowRSTest` pins for the other backends. For the async
number fix, adding `case '.'` to the three `_startValue*` methods passes the issue's
case but leaves feed-boundary dependent behaviour that the ALL-split tests catch;
note that the gold also rewrote ten existing assertions.

Candidate 4, jackson-databind. Symptom: explicit `null` over a non-null default is
dropped under `NON_DEFAULT`, differently for per-type and global modes; an
"empty" custom value is written under global `NON_DEFAULT` only when
`USE_REAL_INCLUDE_NON_DEFAULT` is on; a `@JsonValue` wrapper ignores the value
type's inclusion override. The naive fix is #6283 itself: a flag that keeps
`suppressNulls = true` for the global path only. Upstream removed that flag the
next day (#6318) because the two modes must agree; the guard that the final shape
must keep is `globalNonDefaultNullDefaultSuppressesNull` and
`testPerTypeNonDefaultNullDefaultSuppressesNull` (a null real default still
suppresses nulls) together with `testOldBehaviorWithoutFeature`. For #6322 the
naive gate on `_useRealPropertyDefaults` is exactly the bug; the fix must gate on
whether a default bean was actually created so that `MARKER_FOR_EMPTY` still
applies, without disturbing the per-type round-trip tests. For #6323 the naive fix
(swap the property type for the `@JsonValue` type in `PropertyBuilder`, as #5655
did) drops overrides registered on the wrapper type, which the review flagged.

Candidate 5, TomlGenerator. Symptom: a blank line after every `short` and every
`writeUTF8String`, garbage from `writeString(char[],...)` when escaping is needed,
no line feed after `writeRawValue`. The naive fix deletes the duplicate
`writeValueEnd()` calls and fixes the index; it passes until the next inherited
`GeneratorBase` method appears. The guard the structural fix adds is
`valueLinesTerminatedOnce` (26 write kinds, each exactly one line), and the
adjacent behaviour that must survive is inline arrays and tables separated by
`, ` with no line feed, pinned by the existing `TomlGeneratorTest` cases.

Candidate 6, ThreeTen-Extra. Symptom: `Seconds.parse("PT2147483647M")` throws
`ArithmeticException`; `Days.of(MIN_VALUE).dividedBy(-1)` returns `MIN_VALUE`;
`"P1DT"` parses; `wordBased(Duration.ofMillis(-1500))` prints "-2 seconds and 500
milliseconds". The naive fix for the trailing `T` is to require a time section
after `T` in the regex, which breaks `"PT0S"`-style rows in
`test_parse_CharSequence_valid` unless written as a lookahead; the naive fix for
overflow is to catch `ArithmeticException` around the whole parse, which also
converts the existing `NumberFormatException` paths and changes messages the
invalid-parse tests check. Weakest floor 8 of the six.

## (c) Survey list

Kept: javaparser/javaparser (candidate 1), jhy/jsoup (2), FasterXML/jackson-core
(3), FasterXML/jackson-databind (4), FasterXML/jackson-dataformats-text (5, TOML
module), ThreeTen/threeten-extra (6).

- FasterXML/jackson-core: kept; see candidate 3 (dense October 2026 cluster across the four tokenizer backends).
- FasterXML/jackson-databind: kept; see candidate 4 (`@JsonInclude` regression chain).
- FasterXML/jackson-dataformats-text: kept; see candidate 5 (`TomlGenerator`); the YAML `OutputStream` close chain (#719, #734, #735, #748, #749, #752) is resource lifecycle, not semantics.
- FasterXML/jackson-dataformat-xml: rejected; six fixes in different areas (name processors #886 and #895, `nextTextValue` #902, `@JsonApplyView` #873); #872 "revert #627" is a single revert.
- google/gson: rejected; four fixes in unrelated classes (#3059 `JsonPrimitive.hashCode`, #3067 URI adapter, #3096 exception locations, #3116 strict surrogates).
- jhy/jsoup: kept; see candidate 2.
- google/guava: rejected; fixes are singletons (#8579 `ExecutionSequencer`, #8704 `BaseEncoding`, #8737 `InetAddresses`, #8775 `BiMap.replaceAll`) and the suite runs far beyond 20 min.
- ben-manes/caffeine: rejected; two small fixes (#1986 climber sketch reset, #2010 stats unregister), the rest is NullAway churn.
- apache/commons-text: rejected; seven fixes in seven classes; `StringSubstitutor` has only two (#752, #764).
- apache/commons-collections: rejected; 17 fixes with at most two per class (`IndexedCollection` #683 and #688, multiset #692 and #701).
- apache/commons-csv: rejected; four fixes (#612, #613, #620, #631), two thin ones in quoting.
- apache/commons-io: rejected; the October "f0xx" audit is 15 fixes in 13 classes; `FilenameUtils`/`FileSystem` has three (#890, #891, #896) but #891 is documentation and #896 rewrites its own test suite.
- apache/commons-compress: rejected; the June to August fixes are bounds and sanity checks next to security hardening (#786, #787, #791, #800); the two correctness fixes (#774 XZ, #775 TAR sparse) are different formats.
- apache/commons-lang: excluded by instruction.
- apache/commons-codec: rejected but noted as reserve; Base16/Base32/Base58 builder fixes #434, #435, #437 all merged 2026-06-18 form a three-fix family (custom alphabets ignored or mis-wired) with existing round-trip guards, but they are June, self-contained and weakly coupled.
- apache/commons-validator: rejected; five fixes in five validators.
- apache/commons-beanutils: rejected; three fixes, two of which are the same converter bug on two branches.
- apache/commons-bcel: rejected; two correctness fixes (#499, #510).
- apache/commons-jexl: rejected; #412 bundles five independent bugs across parser and interpreter in one PR; only two share `StringParser`; JavaCC build.
- apache/commons-configuration: rejected; build fixes only.
- apache/commons-vfs: rejected; one fix in a network provider.
- apache/commons-math: rejected; three June overflow fixes in array bounds checks, thin.
- apache/commons-numbers, commons-rng, commons-geometry, commons-imaging, commons-dbutils, commons-exec, commons-pool, commons-fileupload, commons-net, commons-cli: rejected; zero to two fixes each since June.
- junit-team/junit-framework (junit5 was renamed): rejected; 400 merged PRs are features, Kotlin docs and build; no fix cluster in the title scan; Gradle with toolchain provisioning.
- assertj/assertj: rejected; 132 merged PRs, none matched fix language; new assertions and dependency updates.
- mockito/mockito: rejected; the "fixes" are features (#3835, #3853); Gradle plus ByteBuddy agent setup.
- antlr/antlr4: rejected; two merged PRs in all of 2026, no fixes; multi-target build.
- jgrapht/jgrapht: rejected; two fixes in different algorithms (#1376, #1379).
- ThreeTen/threeten-extra: kept at rank 6.
- micrometer-metrics/micrometer: rejected; 400 PRs, no fix cluster in the scan; instrumentation features.
- apache/jena: rejected; June to September is a JUnit 6 migration (#3986, #3989, #4206); very large build.
- apache/calcite: rejected; `RexSimplify` has two fixes (#5226, #5234) and two unrelated reverts (#5028, #5129); Gradle build and suite far over 20 min.
- javaparser/javaparser: kept; see candidate 1. A second subsystem, lexical preservation (#5088 2026-08-18, #5100 2026-08-25, #5151 2026-10-07, #5179 and #5180 2026-10-19 in `DifferenceElementCalculator` and `ReshuffledDiffElementExtractor`), is a fallback cluster with about 680 LPP tests as guards but weaker written floor-8 evidence.
- diffplug/spotless: rejected; plugin fixes depend on external formatters and npm; not unit-testable offline.
- checkstyle/checkstyle: rejected; 400 PRs are new checks, Xpath test migrations and infra; no correctness cluster; long suite.
- pmd/pmd: rejected; rule fixes are independent; #7084 is one follow-up; heavy multi-module build.
- google/error-prone: rejected; the Reachability pair (#6199 tests, #6200 fix) is a single fix; JDK-specific build and long tests.
- h2database/h2database: rejected; fixes spread across `Select`, `Database`, `Recover`, `MVStore`; custom build and `TestAll` harness far over 20 min.
- pgjdbc/pgjdbc: rejected; needs a PostgreSQL server.
- JSQLParser/JSqlParser: rejected; 114 fix PRs but 82 touch the single grammar file `JSqlParserCC.jjt` for unrelated dialect features; one regression (#2731); no coupled cluster.
- jline/jline3: rejected; fixes are ReDoS guards and terminal handling.
- spotbugs/spotbugs: rejected; detectors are independent; no cluster.
- eclipse-vertx/vert.x: rejected; networking, needs sockets; the revert #6301 is connection pooling.
- netty/netty: rejected; codec fixes are protocol singletons (STOMP #17664, QPACK #17778); huge build.
- openrewrite/rewrite: rejected; multi-language monorepo; reverts (#8955, #8962) concern Kotlin templates; too heavy.
- square/okio, square/moshi, remkop/picocli, fusesource/jansi, classgraph/classgraph, cbeust/testng, jqno/equalsverifier, jqwik-team/jqwik: rejected; zero to four merged PRs since June or no fixes.

## (d) Gaps and uncertainties

- Nothing was built. Test runtimes are CI wall times; arm64 container timings are
  estimates. The javaparser full reactor is the only candidate near the 20 min
  limit; a `-pl javaparser-symbol-solver-testing -am` run should be well under it
  but has not been measured.
- javaparser: #5159 changes two existing `LeastUpperBoundTest` expectations, so it
  must be excluded or the base placed after it. #5154 touches
  `AbstractJavaParserContext` and `MethodResolutionLogic` broadly (+218/-205) and
  may be too large for a gold; the 4-fix subset avoids it. Whether the five
  `JavaParser*Declaration` edits in #5139 and the `JavaParserRecordDeclaration`
  edits in #5154 compose cleanly at one base is unverified.
- jsoup: #2616, #2618, #2630 and 81718491 change visible assertions; the
  7-fix subset does not, but its gold is small (about 190 changed lines), so the
  expert-hours estimate (4 to 8) rests on the state-machine subtlety rather than
  volume. Several fixes are direct commits without PRs or linked issues
  (f79f21d2, f062a8ab, 0719f9cf), so symptom descriptions must come from the
  commit messages and tests.
- jackson-core: the cluster spans four parser classes plus `JsonParserBase`, more
  than the three-file guideline unless the task is restricted to
  `UTF8DataInputJsonParser` (#1745, #1751, the DataInput part of #1757, #1796),
  where the ALL_MODES tests of the other backends act as guards. #1747 modifies
  ten existing assertions. The SNAPSHOT parent POM must be present in the warmed
  repository. The strongest floor-8 item (#1757) is evidenced by the PR's own
  commit sequence, not by a pre-existing test.
- jackson-databind: #6318 flips a visible test; 3.x depends on SNAPSHOT
  jackson-core; the maintainer calls the `NON_DEFAULT` logic convoluted, which
  cuts both ways (hard task, but the oracle is less settled). #6323 is 3.3-only
  because of new `@since 3.3` methods.
- Contamination: all merges are after 2026-06-01, most after 2026-09-01, so the
  patches are post-cutoff for current models. The jsoup and javaparser fixes
  implement published specifications (WHATWG HTML parsing, JLS) that models know,
  so the hard part must be the project-specific implementation, which the guards
  enforce. Many jackson PRs were generated with Claude Code; their bodies and test
  names are highly descriptive and must not leak into task prompts.
- Not checked: whether any candidate's tests are flaky on arm64, exact
  `javaparser` test execution counts per module, and whether jsoup's
  `ConnectTest` and `SessionTest` local servers bind ports that a locked-down
  container forbids (they can be excluded by package if so).

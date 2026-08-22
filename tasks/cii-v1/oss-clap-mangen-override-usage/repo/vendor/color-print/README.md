# color-print

Colorize and stylize strings for terminal at compile-time, by using an HTML-like syntax.

This library provides the following macros:

 - `cformat!(<FORMAT_STRING> [, ARGS...])`
 - `cprint!(<FORMAT_STRING> [, ARGS...])`
 - `cprintln!(<FORMAT_STRING> [, ARGS...])`
 - `ceprint!(<FORMAT_STRING> [, ARGS...])`
 - `ceprintln!(<FORMAT_STRING> [, ARGS...])`
 - `cwrite!(f, <FORMAT_STRING> [, ARGS...])`
 - `cwriteln!(f, <FORMAT_STRING> [, ARGS...])`
 - `cstr!(<FORMAT_STRING>)`
 - `untagged!(<FORMAT_STRING>)`

The macros have the same syntax as their corresponding `std` variants:
- `cformat!()` as `format!()`
- `cprint!()` as `print!()`
- `cprintln!()` as `println!()`
- `ceprint!()` as `eprint!()`
- `ceprintln!()` as `eprintln!()`
- `cwrite!()` as `write!()`
- `cwriteln!()` as `writeln!()`

But they accept an additional syntax inside the
format string: HTML-like tags which add ANSI colors/styles at compile-time.

`cstr!()` only transforms the given string literal into another string literal, without
formatting anything else than the colors tag.

`untagged!()` removes all the tags found in the given string literal.

### What does it do ?

By default, the provided macros will replace the tags found in the format string by ANSI
hexadecimal escape codes. E.g.:

```rust
cprintln!("HELLO <green>WORLD</green>");
cprintln!("HELLO <green>WORLD</>"); // Alternative, shorter syntax
```

will be replaced by:

```rust
println!("HELLO \u{1b}[31mWORLD\u{1b}[39m")
```

*Note*: it is possible to change this behaviour by activating the feature `terminfo`. Then it
will question the `terminfo` database at runtime in order to know which sequence to write for
each kind of styling/colorizing (see below for more detail).

## Pros/cons of this crate

### Pros

* Styling is processed at compile-time, so the runtime payload is  inexistant (unless the
  feature `terminfo` is activated);
* Nested tags are well handled, e.g. `"<green>...<blue>...</blue>...</green>"`;
* Some optimizations are performed to avoid redundant ANSI sequences, because these
  optimizations can be done at compile-time without impacting the runtime;
* Almost every tag has a short name, so colorizing can be done quickly: `"my <b>blue</> word"`;
* Each provided macro can be used exactly in the same way as the standard `format!`-like
  macros; e.g., positional arguments and named arguments can be used as usual;
* Supports 16, 256 and 16M colors;
* Fine-grained error handling (errors will be given at compile-time).

### Cons

* Not compatible with non-ANSI terminals.

## Introduction

### Basic example

```rust
use color_print::cprintln;
cprintln!("Hello <green>world</green>!");
```

### Closing a tag more simply: the `</>` tag

Basically, tags must be closed by giving *exactly* the same colors/styles as their matching
open tag (with a slash `/` at the beginning), e.g: `<blue,bold>...</blue,bold>`. But it can be
tedious!

So, it is also possible to close the last open tag simply with `</>`:

```rust
cprintln!("Hello <green>world</>!");
```

### Combining colors and styles

Multiple styles and colors can be combined into a single tag by separating them with the `,`
comma character:

```rust
cprintln!("This a <green,bold>green and bold text</green,bold>.");
// The same, but closing with the </> tag:
cprintln!("This a <green,bold>green and bold text</>.");
```

### Nesting tags

Any tag can be nested with any other.

*Note*: The closing tags must match correctly (following the basic rules of nesting for HTML
tags), but it can always be simplified by using the tag `</>`.

Example of nested tags:

```rust
cprintln!("<green>This is green, <bold>then green and bold</bold>, then green again</green>");
cprintln!("<green>This is green, <bold>then green and bold</>, then green again</>");

// Colors can be nested as well:
cprintln!("<green>This is green, <blue>then blue</blue>, then green again</green>");
cprintln!("<green>This is green, <blue>then blue</>, then green again</>");
```

### Unclosed tags are automatically closed at the end of the format string

Tags which have not been closed manually will be closed automatically, which means that the
ANSI sequences needed to go back to the original state will be added:

```rust
// The two following lines are strictly equivalent:
cprintln!("<green><bold>Hello");
cprintln!("<green><bold>Hello</></>");
```

### How to display the chars `<` and `>` verbatim

As for `{` and `}` in standard format strings, the chars `<` and `>` have to be doubled in
order to display them verbatim:

```rust
cprintln!("This is an angle bracket character: <<, and here is another one: >>");
```

## Optimization: no redundant ANSI codes

The expanded format string will only contain the *needed* ANSI codes. This is done by making a
diff of the different style attributes, each time a tag is encountered, instead of mechanically
adding the ANSI codes.

E.g., several nested `<bold>` tags will only produce one bold ANSI sequence:

```rust
cprintln!("<bold><bold> A <bold,blue> B </> C </></>")
```

will be replaced by:

```rust
println!("\u{1b}[1m A \u{1b}[34m B \u{1b}[39m C \u{1b}[22m")
//        ^-------^   ^--------^   ^--------^   ^--------^
//          bold         blue         color        bold
//                                    reset        reset
```

## The feature `terminfo`

Instead of inserting ANSI sequences directly into the format string, it is possible to activate
the feature `terminfo`: this will add the format sequences at runtime, by consulting the
`terminfo` database.

This has one pro and several cons:

##### Pros

* This adds a level of compatibility for some terminals.

##### Cons

* This adds a little runtime payload;
* This adds two dependencies: `lazy_static` and `terminfo`;
* The styles `<strike>` and `<conceal>` are not handled;
* With `terminfo`, many styles are not resettable individually, which implies longer format
  sequences for the same result;
* For now, the provided macros can only be used in one thread.

`lazy_static`: https://crates.io/crates/lazy_static
`terminfo`: https://crates.io/crates/terminfo

## Naming rules of the tags:

Each tag has at least a **long name**, like `<magenta>` or `<underline>`.

The tags directly relative to *colors* (like `<red>`, `<bg:blue>`, `<bg:bright-green>`..., as
opposed to *style* tags like `<bold>`, `<italics>`...) have some common naming rules:

 * Each tag has four variants:
   - `<mycolor>`: the normal, foreground color;
   - `<bright-mycolor>` or `<mycolor!>`: the bright, foreground color;
   - `<bg:mycolor>`, `<MYCOLOR>`: the normal, background color;
   - `<bg:bright-mycolor>`, `<bg:mycolor!>`, `<BRIGHT-MYCOLOR>` or `<MYCOLOR!>`: the bright,
     background color;
 * Each tag has a *shortcut*, with a base letter for each color; example with the `x` letter:
   - `<x>`: the normal, foreground color;
   - `<x!>`: the bright, foreground color;
   - `<bg:x>`, `<X>`: the normal, background color;
   - `<bg:x!>`, `<X!>`: the bright, background color;
 * Each color's shortcut letter is simply the **first letter of its name** (excepted for `<k>`
   which is the shortcut for `<black>`), e.g. `<y>` is the shortcut for `<yellow>`;
 * Each color's tag which is uppercase is a **background color**;
 * Each tag which has a trailing exclamation point `!` is a **bright color**;

## List of accepted tags:

The two first columns show which styles are supported, respectively with the default crate
features (ANSI column), and with the feature `terminfo` being activated.

| ANSI | Terminfo | Shortcuts | Long names              | Aliases                                         |
| ---- | -------- | --------- | ----------------------- | ----------------------------------------------- |
| X    | X        | `<s>`     | `<strong>`              | `<em>` `<bold>`                                 |
| X    | X        |           | `<dim>`                 |                                                 |
| X    | X        | `<u>`     | `<underline>`           |                                                 |
| X    |          |           | `<strike>`              |                                                 |
| X    | X        |           | `<reverse>`             | `<rev>`                                         |
| X    |          |           | `<conceal>`             | `<hide>`                                        |
| X    | X        | `<i>`     | `<italics>`             | `<italic>`                                      |
| X    | X        |           | `<blink>`               |                                                 |
| X    | X        | `<k>`     | `<black>`               |                                                 |
| X    | X        | `<r>`     | `<red>`                 |                                                 |
| X    | X        | `<g>`     | `<green>`               |                                                 |
| X    | X        | `<y>`     | `<yellow>`              |                                                 |
| X    | X        | `<b>`     | `<blue>`                |                                                 |
| X    | X        | `<m>`     | `<magenta>`             |                                                 |
| X    | X        | `<c>`     | `<cyan>`                |                                                 |
| X    | X        | `<w>`     | `<white>`               |                                                 |
| X    | X        | `<k!>`    | `<bright-black>`        | `<black!>`                                      |
| X    | X        | `<r!>`    | `<bright-red>`          | `<red!>`                                        |
| X    | X        | `<g!>`    | `<bright-green>`        | `<green!>`                                      |
| X    | X        | `<y!>`    | `<bright-yellow>`       | `<yellow!>`                                     |
| X    | X        | `<b!>`    | `<bright-blue>`         | `<blue!>`                                       |
| X    | X        | `<m!>`    | `<bright-magenta>`      | `<magenta!>`                                    |
| X    | X        | `<c!>`    | `<bright-cyan>`         | `<cyan!>`                                       |
| X    | X        | `<w!>`    | `<bright-white>`        | `<white!>`                                      |
| X    | X        | `<K>`     | `<bg:black>`            | `<BLACK>`                                       |
| X    | X        | `<R>`     | `<bg:red>`              | `<RED>`                                         |
| X    | X        | `<G>`     | `<bg:green>`            | `<GREEN>`                                       |
| X    | X        | `<Y>`     | `<bg:yellow>`           | `<YELLOW>`                                      |
| X    | X        | `<B>`     | `<bg:blue>`             | `<BLUE>`                                        |
| X    | X        | `<M>`     | `<bg:magenta>`          | `<MAGENTA>`                                     |
| X    | X        | `<C>`     | `<bg:cyan>`             | `<CYAN>`                                        |
| X    | X        | `<W>`     | `<bg:white>`            | `<WHITE>`                                       |
| X    | X        | `<K!>`    | `<bg:bright-black>`     | `<BLACK!>` `<bg:black!>` `<BRIGHT-BLACK>`       |
| X    | X        | `<R!>`    | `<bg:bright-red>`       | `<RED!>` `<bg:red!>` `<BRIGHT-RED>`             |
| X    | X        | `<G!>`    | `<bg:bright-green>`     | `<GREEN!>` `<bg:green!>` `<BRIGHT-GREEN>`       |
| X    | X        | `<Y!>`    | `<bg:bright-yellow>`    | `<YELLOW!>` `<bg:yellow!>` `<BRIGHT-YELLOW>`    |
| X    | X        | `<B!>`    | `<bg:bright-blue>`      | `<BLUE!>` `<bg:blue!>` `<BRIGHT-BLUE>`          |
| X    | X        | `<M!>`    | `<bg:bright-magenta>`   | `<MAGENTA!>` `<bg:magenta!>` `<BRIGHT-MAGENTA>` |
| X    | X        | `<C!>`    | `<bg:bright-cyan>`      | `<CYAN!>` `<bg:cyan!>` `<BRIGHT-CYAN>`          |
| X    | X        | `<W!>`    | `<bg:bright-white>`     | `<WHITE!>` `<bg:white!>` `<BRIGHT-WHITE>`       |
| X    |          |           | `<rgb(r,g,b)>`          | `<#RRGGBB>`                                     |
| X    |          |           | `<bg:rgb(r,g,b)>`       | `<bg:#RRGGBB>` `<RGB(r,g,b)>`                   |
| X    |          | `<0>`...`<255>` | `<palette(...)>`  | `<p(...)>` `<pal(...)>`                         |
| X    |          | `<P(...)>` | `<bg:palette(...)>` | `<PALETTE(...)>` `<PAL(...)>` `<bg:p(...)>` `<bg:pal(...)>` |

License: MIT OR Apache-2.0

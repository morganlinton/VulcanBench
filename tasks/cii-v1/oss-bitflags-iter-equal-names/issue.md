# Add iter_equal_names(): the defined names equal to a flags value

`iter_names()` decomposes a value into its component flags, but there is no
way to ask the reverse question: which **defined names** (including aliases
and convenience flags) denote exactly this value?

```rust
bitflags! {
    struct MyFlags: u8 {
        const A = 1;
        const B = 2;
        const AB = Self::A.bits() | Self::B.bits();
        const ALIAS_A = 1;
    }
}

MyFlags::A.iter_equal_names()               // yields "A", then "ALIAS_A"
(MyFlags::A | MyFlags::B).iter_equal_names() // yields "AB"
```

- A method on the `Flags` trait (so every generated type gets it), yielding
  `&'static str` names in definition order.
- A name is yielded iff its defined flags value's bits **equal** the
  value's bits — aliases included, convenience flags included.
- A combination that no defined name denotes yields nothing; so does an
  empty value unless an empty flag is defined.
- Existing iteration (`iter_names`, `contains`, `bits`, equality) is
  unchanged.

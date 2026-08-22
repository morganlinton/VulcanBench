# Styles cannot use `transparent` as a color

Style definitions accept hex colors and pass CSS `var()`/`calc()` through,
but the CSS keyword `transparent` is rejected at style-definition time:

```python
class TransparentStyle(Style):
    styles = {Name: 'transparent bg:transparent border:transparent'}
# AssertionError: wrong color format 'transparent'
```

`transparent` is a legitimate CSS color and useful for themes that want a
token to inherit the page background.

Expected: `transparent` is accepted anywhere a style color is (foreground,
`bg:`, `border:`), preserved verbatim through `style_for_token`, and emitted
verbatim by the HTML formatter's CSS (`color: transparent`,
`background-color: transparent`, `border: 1px solid transparent`) — never
mangled into a hex form. Hex colors (including 6→3-digit shortening),
`var()`/`calc()` pass-through, and rejection of unknown color words are
unchanged.

"""Hidden fail-to-pass tests: 'transparent' as a style color. Styles are built
inside each test so the base-time rejection fails each one individually."""

from pygments.formatters import HtmlFormatter
from pygments.style import Style
from pygments.token import Name


def _transparent_style():
    class TransparentStyle(Style):
        styles = {Name: "transparent bg:transparent border:transparent"}

    return TransparentStyle


def test_transparent_accepted_in_style_definition():
    style = _transparent_style()
    ndef = style.style_for_token(Name)
    assert ndef["color"] == "transparent"
    assert ndef["bgcolor"] == "transparent"
    assert ndef["border"] == "transparent"


def test_transparent_emitted_in_css():
    style = _transparent_style()
    css = HtmlFormatter(style=style).get_style_defs()
    assert "color: transparent" in css
    assert "background-color: transparent" in css
    assert "border: 1px solid transparent" in css


def test_transparent_not_hex_mangled():
    style = _transparent_style()
    css = HtmlFormatter(style=style).get_style_defs()
    assert "#transparent" not in css

"""Hidden pass-to-pass guards: existing color handling unchanged."""

import pytest
from pygments.formatters import HtmlFormatter
from pygments.style import Style
from pygments.token import Name


def test_hex_colors_still_work():
    class HexStyle(Style):
        styles = {Name: "#ff0000 bg:#00ff00"}

    ndef = HexStyle.style_for_token(Name)
    assert ndef["color"] == "ff0000"
    assert ndef["bgcolor"] == "00ff00"
    css = HtmlFormatter(style=HexStyle).get_style_defs()
    assert "color: #f00" in css.lower()


def test_var_and_calc_pass_through():
    class VarStyle(Style):
        styles = {Name: "var(--fg) bg:calc(1px)"}

    ndef = VarStyle.style_for_token(Name)
    assert ndef["color"] == "var(--fg)"
    assert ndef["bgcolor"] == "calc(1px)"


def test_invalid_color_still_rejected():
    with pytest.raises(AssertionError):
        class BadStyle(Style):
            styles = {Name: "notacolor"}


def test_default_style_renders():
    css = HtmlFormatter().get_style_defs()
    assert ".k {" in css or ".k{" in css

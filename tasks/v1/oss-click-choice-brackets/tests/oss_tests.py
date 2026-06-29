import click
from click.testing import CliRunner


def test_choice_argument_optional_metavar():
    runner = CliRunner()

    @click.command()
    @click.argument("method", type=click.Choice(["foo", "bar", "baz"]), nargs=-1)
    def cli_variadic(method):
        pass

    @click.command()
    @click.argument("method", type=click.Choice(["foo", "bar", "baz"]), required=False)
    def cli_optional(method):
        pass

    variadic = runner.invoke(cli_variadic, ["--help"]).output
    assert "[foo|bar|baz]..." in variadic
    assert "[[foo|bar|baz]]" not in variadic
    optional = runner.invoke(cli_optional, ["--help"]).output
    assert "[foo|bar|baz]" in optional
    assert "[[foo|bar|baz]]" not in optional


def test_plain_optional_argument_still_bracketed():
    runner = CliRunner()

    @click.command()
    @click.argument("name", required=False)
    def cli(name):
        pass

    out = runner.invoke(cli, ["--help"]).output
    assert "[NAME]" in out

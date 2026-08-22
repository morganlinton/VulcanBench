"""Hidden pass-to-pass guards: the frozen version_option and basic commands."""

import click
from click.testing import CliRunner


def test_version_option_still_works():
    @click.command()
    @click.version_option(version="2.5", prog_name="tool")
    def cli():
        pass

    result = CliRunner().invoke(cli, ["--version"])
    assert result.exit_code == 0
    assert "tool" in result.output
    assert "2.5" in result.output


def test_plain_command_runs():
    @click.command()
    @click.option("--n", default=1, type=int)
    def cli(n):
        click.echo(str(n * 2))

    result = CliRunner().invoke(cli, ["--n", "4"])
    assert result.exit_code == 0
    assert result.output.strip() == "8"


def test_help_still_renders():
    @click.command()
    def cli():
        """Does things."""

    result = CliRunner().invoke(cli, ["--help"])
    assert result.exit_code == 0
    assert "Does things." in result.output

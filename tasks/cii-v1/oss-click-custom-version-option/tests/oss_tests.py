"""Hidden fail-to-pass tests: click.custom_version_option — a --version option
whose output comes from a callback. The API does not exist at base, so the
decorator lookup fails each test individually."""

import click
from click.testing import CliRunner


def test_prints_callback_output_and_exits():
    @click.command()
    @click.custom_version_option(lambda ctx: "custom 9.9.9")
    def cli():
        click.echo("body")

    result = CliRunner().invoke(cli, ["--version"])
    assert result.exit_code == 0
    assert result.output == "custom 9.9.9\n"
    assert "body" not in result.output


def test_custom_param_decls():
    @click.command()
    @click.custom_version_option(lambda ctx: "v", "-V", "--version")
    def cli():
        pass

    for args in (["-V"], ["--version"]):
        result = CliRunner().invoke(cli, args)
        assert result.exit_code == 0
        assert result.output == "v\n"


def test_callback_receives_context():
    @click.command()
    @click.custom_version_option(lambda ctx: f"{ctx.info_name} 1.0")
    def cli():
        pass

    result = CliRunner().invoke(cli, ["--version"], prog_name="mytool")
    assert result.exit_code == 0
    assert result.output == "mytool 1.0\n"


def test_eager_before_required_arguments():
    @click.command()
    @click.custom_version_option(lambda ctx: "early")
    @click.argument("needed")
    def cli(needed):
        pass

    result = CliRunner().invoke(cli, ["--version"])
    assert result.exit_code == 0
    assert result.output == "early\n"

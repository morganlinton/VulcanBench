# Add custom_version_option: a --version option rendered by a callback

`version_option` prints a fixed message built from a small set of values
(prog name, package, version). Users keep asking for more slots — a file
path, the Python version, git metadata — and every addition complicates the
common case (discussion #3527).

Add the customizable companion instead, and freeze `version_option`:

```python
@click.command()
@click.custom_version_option(lambda ctx: f"{ctx.info_name} 1.0 (py{sys.version_info[0]})")
def cli(): ...
```

- `click.custom_version_option(callback, *param_decls, **kwargs)`: adds a
  flag option (default declaration `--version`) that calls `callback` with
  the current `Context`, prints its return value, and exits the program
  with status 0. Custom declarations like `-V` work.
- The option behaves like `version_option` operationally: eager (it wins
  even when required arguments are missing), not exposed to the command
  callback, help text provided; extra kwargs pass through to `option()`.
- Exported from the top-level `click` namespace.
- `version_option` itself is unchanged (its parameters are now frozen),
  and ordinary options/help keep working.

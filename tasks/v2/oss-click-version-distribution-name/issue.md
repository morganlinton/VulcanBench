`click.version_option` fails with "is not installed" when a package's import name differs from its installed distribution name.

Many packages are imported under a different name than the one they are installed under: you `pip install Pillow` but `import PIL`, `pip install beautifulsoup4` but `import bs4`. When a CLI lives in such a package, `@click.version_option()` (whether the package name is auto-detected from the calling frame or passed explicitly as the import name) raises:

```
RuntimeError: 'PIL' is not installed. Try passing 'package_name' instead.
```

even though the distribution is installed and its version is perfectly knowable. The standard library exposes the mapping needed to resolve this (`importlib.metadata.packages_distributions`).

Expected behavior: when the package name does not match an installed distribution, resolve it as an import (top-level module) name to its distribution and report that distribution's version. If the module name maps to more than one installed distribution, raise a clear error telling the user to pass `package_name` to disambiguate. Lookups by actual distribution name must keep working exactly as before.

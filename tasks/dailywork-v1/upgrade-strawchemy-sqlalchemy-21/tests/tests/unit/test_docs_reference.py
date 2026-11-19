"""Tests for the griffe2md-backed docs reference generator."""

from __future__ import annotations

import importlib.util
import re
import sys
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

if TYPE_CHECKING:
    from types import ModuleType

    import griffe
    from tools.docs_reference import GraphQLEmitter, GriffeRenderer, ReferenceGenerator

REPO_ROOT = Path(__file__).resolve().parents[2]
VERSION = "0.0.1"


@pytest.fixture(scope="module")
def docs_reference() -> ModuleType:
    """Import `tools/docs_reference.py`, which lives outside any package."""
    path = REPO_ROOT / "tools" / "docs_reference.py"
    spec = importlib.util.spec_from_file_location("docs_reference", path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def generator(docs_reference: ModuleType) -> ReferenceGenerator:
    """A generator driven by the specification the command line uses."""
    return docs_reference.ReferenceGenerator(docs_reference.DEFAULT_SPEC)


@pytest.fixture
def package(generator: ReferenceGenerator) -> griffe.Module:
    """A private copy of the documented package, so a test may mutate it."""
    return generator.load_package()


@pytest.fixture
def renderer(docs_reference: ModuleType) -> GriffeRenderer:
    """A renderer pinned to a fixed version, so source links stay stable across releases."""
    return docs_reference.GriffeRenderer(docs_reference.DEFAULT_SPEC.layout.source_url, VERSION)


@pytest.fixture
def pages(docs_reference: ModuleType, package: griffe.Module, renderer: GriffeRenderer) -> dict[str, str]:
    """The rendered object pages, keyed by path and in declaration order."""
    spec = docs_reference.DEFAULT_SPEC
    emitter = docs_reference.ObjectPageEmitter(package, renderer, spec.layout.header, spec.pages)
    return {page.path: page.content for page in emitter.emit()}


@pytest.fixture
def graphql_emitter(docs_reference: ModuleType) -> GraphQLEmitter:
    """An emitter for the operator matrix and the example schema."""
    spec = docs_reference.DEFAULT_SPEC
    return docs_reference.GraphQLEmitter(spec.layout.header, spec.operator_exports)


def test_anchor_headings_shortens_the_displayed_name(docs_reference: ModuleType) -> None:
    """Test that a heading displays the object's own name and anchors its full path."""
    assert docs_reference.anchor_headings("#### `strawchemy.QueryHook.load`") == (
        "#### `load` {#strawchemy-QueryHook-load}"
    )


def test_insert_source_link_follows_the_heading(docs_reference: ModuleType) -> None:
    """Test that the source link is placed between the heading and the body."""
    rendered = docs_reference.insert_source_link("### `A` {#A}\n\n```python\nA()\n```", "[source](url)")
    assert rendered.splitlines()[:3] == ["### `A` {#A}", "", "[source](url)"]


def test_resolve_crossrefs_links_within_a_page(docs_reference: ModuleType) -> None:
    """Test that a reference to an object on the same page becomes a bare fragment."""
    page = docs_reference.Page(
        "reference/api/hooks",
        "Hooks",
        "#### `load` {#strawchemy-QueryHook-load}\n[`load`](#strawchemy.QueryHook.load)",
    )
    (resolved,) = docs_reference.resolve_crossrefs([page])
    assert "[`load`](#strawchemy-QueryHook-load)" in resolved.content


def test_resolve_crossrefs_links_across_pages(docs_reference: ModuleType) -> None:
    """Test that a reference to an object on another page becomes a site-absolute link."""
    config = docs_reference.Page(
        "reference/config", "Configuration options", "## `DTOConfig` {#strawchemy-dto-types-DTOConfig}"
    )
    hooks = docs_reference.Page(
        "reference/api/hooks", "Hooks", "<code>[DTOConfig](#strawchemy.dto.types.DTOConfig)</code>"
    )
    _, resolved = docs_reference.resolve_crossrefs([config, hooks])
    assert "[DTOConfig](/reference/config#strawchemy-dto-types-DTOConfig)" in resolved.content


def test_resolve_crossrefs_drops_undocumented_targets(docs_reference: ModuleType) -> None:
    """Test that a reference nothing documents keeps its text and loses its link."""
    page = docs_reference.Page("reference/api/hooks", "Hooks", "<code>[Any](#typing.Any)</code>")
    (resolved,) = docs_reference.resolve_crossrefs([page])
    assert resolved.content == "<code>Any</code>"


def test_resolve_crossrefs_reports_an_anchor_claimed_by_two_pages(docs_reference: ModuleType) -> None:
    """Test that the same anchor on two pages names both of them in the error."""
    first = docs_reference.Page("reference/api/config", "Configuration types", "## `C` {#pkg-C}")
    second = docs_reference.Page("reference/config", "Configuration options", "## `C` {#pkg-C}")
    with pytest.raises(docs_reference.ReferenceGenerationError, match="pkg-C"):
        docs_reference.resolve_crossrefs([first, second])


def test_every_export_is_covered_by_a_page(generator: ReferenceGenerator, package: griffe.Module) -> None:
    """Test that every name in `__all__` is claimed by a reference page."""
    generator.check_exports_covered(package)


def test_uncovered_export_is_reported(
    docs_reference: ModuleType, generator: ReferenceGenerator, package: griffe.Module
) -> None:
    """Test that an export belonging to no page names itself in the error."""
    package.exports = [*(package.exports or ()), "NotDocumentedAnywhere"]
    with pytest.raises(docs_reference.ReferenceGenerationError, match="NotDocumentedAnywhere"):
        generator.check_exports_covered(package)


def test_spec_claims_the_exports_of_all_its_pages(docs_reference: ModuleType) -> None:
    """Test that a specification claims the names of its pages and of its operator aliases, and no others."""
    spec = docs_reference.ReferenceSpec(
        repo_root=REPO_ROOT,
        layout=docs_reference.DEFAULT_SPEC.layout,
        pages=(docs_reference.PageSpec("reference/api/mapper", "Mapper", "", ("Strawchemy",)),),
        operator_exports=("EqualityOperator",),
    )
    assert spec.documented_exports == {"Strawchemy", "EqualityOperator"}


def test_generate_is_deterministic(generator: ReferenceGenerator) -> None:
    """Test that two consecutive runs produce identical pages."""
    first = generator.generate()
    second = generator.generate()
    assert [(page.path, page.content) for page in first] == [(page.path, page.content) for page in second]


def test_pages_cover_every_spec(docs_reference: ModuleType, pages: dict[str, str]) -> None:
    """Test that one page is emitted per page specification, in declaration order."""
    assert list(pages) == [spec.path for spec in docs_reference.DEFAULT_SPEC.pages]


def test_every_export_has_a_version_pinned_source_link(pages: dict[str, str]) -> None:
    """Test that a documented symbol carries a version-pinned link to its source line."""
    mapper = pages["reference/api/mapper"]
    assert "## `Strawchemy` {#strawchemy-mapper-Strawchemy}" in mapper
    assert f"blob/v{VERSION}/src/strawchemy/mapper.py#L" in mapper


def test_config_page_documents_both_configuration_classes(pages: dict[str, str]) -> None:
    """Test that the options page renders each configuration class under its own heading."""
    config = pages["reference/config"]
    assert "## `StrawchemyConfig` {#strawchemy-config-base-StrawchemyConfig}" in config
    assert "## `DTOConfig` {#strawchemy-dto-types-DTOConfig}" in config


def test_anchor_headings_anchors_every_heading(pages: dict[str, str]) -> None:
    """Test that every heading griffe2md emits below the page title carries an explicit anchor."""
    for path, content in pages.items():
        for line in content.splitlines():
            if line.startswith("##"):
                assert re.search(r" \{#[\w-]+\}$", line), f"{path}: {line}"


def test_no_generated_page_contains_an_unresolved_anchor(generator: ReferenceGenerator) -> None:
    """Test that no reference survives pointing at an anchor no page defines, and no page risks Vue interpolation."""
    anchors = {anchor for page in generator.generate() for anchor in re.findall(r"\{#([^}]+)\}", page.content)}
    for page in generator.generate():
        for target in re.findall(r"\]\(#([^)]*)\)", page.content):
            assert target in anchors, f"{page.path}: {target}"
        assert "{{" not in page.content, page.path


def test_generate_produces_pages_with_explicit_anchors(pages: dict[str, str]) -> None:
    """Test that every object page carries at least one explicit VitePress anchor."""
    assert pages
    for path, content in pages.items():
        assert "{#" in content, path


def test_literal_values_flattens_nested_unions(graphql_emitter: GraphQLEmitter) -> None:
    """Test that an alias built on another alias reports both vocabularies, sorted."""
    from strawchemy.typing import EqualityOperator, OrderOperator

    assert graphql_emitter.literal_values(EqualityOperator) == ("eq", "in", "is_null", "neq", "nin")
    equality = set(graphql_emitter.literal_values(EqualityOperator))
    assert equality < set(graphql_emitter.literal_values(OrderOperator))


def test_graphql_pages_are_emitted(graphql_emitter: GraphQLEmitter) -> None:
    """Test that the GraphQL emitter produces the operator matrix and the example schema."""
    pages = {page.path: page.content for page in graphql_emitter.emit()}
    assert set(pages) == {"reference/graphql/operators", "reference/graphql/example-schema"}


def test_operator_matrix_maps_operators_to_comparison_aliases(graphql_emitter: GraphQLEmitter) -> None:
    """Test that each operator row lists the aliases exposing it, one cell per column."""
    pages = {page.path: page.content for page in graphql_emitter.emit()}
    matrix = pages["reference/graphql/operators"]
    row = next(line for line in matrix.splitlines() if line.startswith("| `eq`"))
    assert len(re.findall(r"(?<!\\)\|", row)) == 3
    assert "EqualityOperator" in row
    assert "| `overlap` |" in matrix


def test_example_schema_page_contains_sdl(graphql_emitter: GraphQLEmitter) -> None:
    """Test that the example application's schema is rendered as a GraphQL code block."""
    pages = {page.path: page.content for page in graphql_emitter.emit()}
    sdl = pages["reference/graphql/example-schema"]
    assert "```graphql" in sdl
    assert "type Query {" in sdl


def test_sidebar_groups_pages_by_directory(
    docs_reference: ModuleType, generator: ReferenceGenerator, tmp_path: Path
) -> None:
    """Test that sidebar groups are ordered and link every generated page."""
    writer = docs_reference.SiteWriter(tmp_path, docs_reference.DEFAULT_SPEC.layout)
    entries = writer.sidebar_entries(generator.generate())
    texts = [entry["text"] for entry in entries]
    assert texts == sorted(texts)
    links = {item["link"] for entry in entries for item in entry["items"]}
    assert "/reference/api/mapper" in links
    assert "/reference/graphql/operators" in links
    assert "/reference/config" in links


def test_command_writes_pages_and_sidebar(docs_reference: ModuleType, tmp_path: Path) -> None:
    """Test that a run writes every page plus the sidebar the VitePress config imports, then exits successfully."""
    with pytest.raises(SystemExit) as exit_info:
        docs_reference.app(["--output", str(tmp_path)])
    assert exit_info.value.code == 0
    assert (tmp_path / "reference" / "api" / "mapper.md").is_file()
    assert (tmp_path / "reference" / "config.md").is_file()
    assert (tmp_path / "reference" / "graphql" / "operators.md").is_file()
    assert (tmp_path / ".vitepress" / "reference-sidebar.json").is_file()

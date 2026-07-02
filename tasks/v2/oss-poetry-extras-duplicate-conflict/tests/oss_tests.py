"""Hidden test for oss-poetry-extras-duplicate-conflict.

Poetry's dependency solver must resolve a project in which the same package is an
optional dependency of two different extras and is also required (with an extra of
its own) by a dependency group. The buggy build synthesises an empty-constraint
placeholder dependency for each duplicate subgroup's leftover marker space, and the
placeholder conflicts with the sibling subgroup: solving fails with "depends on both
A (*) and A (<empty>)". Expected behavior captured from the upstream fix
(python-poetry/poetry PR #10943, issue #10447).

Run with PYTHONPATH=src so the workspace's src/poetry is the poetry under test.
"""
from __future__ import annotations

from cleo.io.null_io import NullIO
from packaging.utils import canonicalize_name
from poetry.core.packages.package import Package
from poetry.core.packages.project_package import ProjectPackage

from poetry.factory import Factory
from poetry.puzzle import Solver
from poetry.repositories import Repository
from poetry.repositories import RepositoryPool


def _solve(package: ProjectPackage, repo: Repository):
    pool = RepositoryPool([repo])
    solver = Solver(package, pool, [], [], NullIO())
    transaction = solver.solve()
    ops = transaction.calculate_operations()
    return [(op.job_type, str(op.package.name), str(op.package.version)) for op in ops]


def test_optional_extras_and_group_dep_with_extras_resolves():
    # Package A is optional in two extras of the root project AND required with an
    # extra by the "test" dependency group. This must resolve to a single install
    # of A, not fail with a spurious "A (*) and A (<empty>)" conflict.
    package = ProjectPackage("root", "1.0")
    repo = Repository("repo")

    dep_alchemy = Factory.create_dependency("A", {"version": "*", "optional": True})
    dep_alchemy._in_extras = [canonicalize_name("alchemy")]
    dep_databases = Factory.create_dependency("A", {"version": "*", "optional": True})
    dep_databases._in_extras = [canonicalize_name("databases")]
    package.extras = {
        canonicalize_name("alchemy"): [dep_alchemy],
        canonicalize_name("databases"): [dep_databases],
    }
    package.add_dependency(dep_alchemy)
    package.add_dependency(dep_databases)
    package.add_dependency(
        Factory.create_dependency(
            "A", {"version": "*", "extras": ["mypy"]}, groups=["test"]
        )
    )

    package_a = Package("A", "1.0")
    package_a.extras = {canonicalize_name("mypy"): []}
    repo.add_package(package_a)

    assert _solve(package, repo) == [("install", "a", "1.0")]


def test_basic_dependency_resolves():
    # Ordinary single-dependency resolution must keep working.
    package = ProjectPackage("root", "1.0")
    repo = Repository("repo")
    package.add_dependency(Factory.create_dependency("B", "*"))
    repo.add_package(Package("B", "2.1"))

    assert _solve(package, repo) == [("install", "b", "2.1")]

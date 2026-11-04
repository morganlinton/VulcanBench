"""Suites routes: the unlock screen's password path, tooling worktree, suite and task CRUD.

Every ``vulcanbench suite`` call is answered by the fake CLI in
``tests/runner_fixtures.py`` (``FAKE_SUITE_CLI``), which records its argv and
the names of its environment variables. The password tests assert the secret
reaches the engine on stdin only and comes back in no response, log line or
file.
"""

from __future__ import annotations

import json
import logging
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from backend import app as appmod
from backend.runner import launch, suites
from backend.runner.state import State
from tests.runner_fixtures import (
    BUILDER_TAG,
    FAKE_PASSWORD,
    BuilderEnv,
    builder_env,
    fake_builder_task,
    fake_repo,
    fake_suite_calls,
    fake_worktree,
    make_config,
    set_fake_suite,
)

API = "/api/runner/suites"


Env = BuilderEnv


@pytest.fixture
def env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Env:
    return builder_env(tmp_path, monkeypatch)


def _client() -> TestClient:
    return TestClient(appmod.app, client=("127.0.0.1", 50000))


def _unlocked(env: Env) -> TestClient:
    set_fake_suite(env.wt, "unlocked")
    return _client()


# ------------------------------------------------------------- tooling --


def test_no_builder_tag_means_no_tooling(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo = fake_repo(tmp_path)
    monkeypatch.setattr(launch, "REPO_ROOT", repo)
    cfg = make_config(tmp_path)
    fake_worktree(repo, cfg)  # the old tag: no harness/suite_builder
    t = suites.tooling_worktree(cfg)
    assert t["ready"] is False and t["tag"] is None
    assert "no prepared bench tag has the suite builder" in t["problems"][0]
    named = suites.tooling_worktree(cfg, "bench/2026-10-29-test")
    assert named["ready"] is False and "predates the suite builder" in named["problems"][-1]


def test_tooling_is_the_newest_prepared_builder_tag(env: Env) -> None:
    t = suites.tooling_worktree(env.cfg)
    assert t == {
        "tag": BUILDER_TAG,
        "worktree": str(env.wt),
        "tag_commit": t["tag_commit"],
        "ready": True,
        "problems": [],
    }
    (env.wt / "harness" / "cli.py").write_text("# edited\n")  # dirty scoring path
    assert suites.tooling_worktree(env.cfg)["ready"] is False


# -------------------------------------------------------------- access --


def test_access_reports_locked_then_unlocked_and_lock_again(env: Env) -> None:
    c = _client()
    r = c.get(f"{API}/access").json()
    assert r["tooling"]["tag"] == BUILDER_TAG
    assert r["access"] == {"configured": True, "unlocked": False, "unlocked_at": None}
    ok = c.post(f"{API}/unlock", json={"password": FAKE_PASSWORD})
    assert ok.status_code == 200 and ok.json()["access"]["unlocked"] is True
    assert c.get(f"{API}/access").json()["access"]["unlocked"] is True
    locked = c.post(f"{API}/lock")
    assert locked.status_code == 200 and locked.json()["access"]["unlocked"] is False


def test_password_goes_on_stdin_only_and_is_never_echoed(
    env: Env, caplog: pytest.LogCaptureFixture
) -> None:
    caplog.set_level(logging.DEBUG)
    c = _client()
    wrong = c.post(f"{API}/unlock", json={"password": "not the password at all"})
    assert wrong.status_code == 403
    assert wrong.json()["detail"]["code"] == "wrong_password"
    assert "not the password at all" not in wrong.text
    good = c.post(f"{API}/unlock", json={"password": FAKE_PASSWORD})
    assert good.status_code == 200
    assert FAKE_PASSWORD not in good.text
    assert (env.wt / ".fake-suite-stdin-ok").read_text() == "yes"
    calls = [x for x in fake_suite_calls(env.wt) if x["argv"][:2] == ["suite", "unlock"]]
    assert len(calls) == 2
    for call in calls:
        assert call["argv"] == ["suite", "unlock", "--password-stdin", "--json"]
        assert not any("PASSWORD" in name for name in call["env"])
    assert FAKE_PASSWORD not in caplog.text and "not the password at all" not in caplog.text
    # Nothing on disk outside the fake's own stdin check holds it either.
    for root in (env.cfg.home, env.wt):
        for f in root.rglob("*"):
            if f.is_file() and f.name != ".fake-suite-password":
                assert FAKE_PASSWORD not in f.read_text(errors="replace"), f


def test_a_cli_that_prints_the_password_is_masked(env: Env) -> None:
    set_fake_suite(env.wt, "echo")
    r = _client().post(f"{API}/unlock", json={"password": FAKE_PASSWORD})
    assert r.status_code == 502
    assert FAKE_PASSWORD not in r.text and "[REDACTED]" in r.text


@pytest.mark.parametrize(
    ("body", "status"),
    [
        (b"password=hunter2hunter2", 422),
        (b'{"password": "two\\nlines here"}', 422),
        (b'{"password": ""}', 422),
        (b'{"pass": "hunter2hunter2"}', 422),
        (b'["hunter2hunter2"]', 422),
        (b'{"password": "hunter2hunter2", "tag": 5}', 422),
        (b'{"password": "' + b"x" * 5000 + b'"}', 413),
    ],
)
def test_malformed_unlock_bodies_are_refused_without_quoting_them(
    env: Env, body: bytes, status: int
) -> None:
    r = _client().post(f"{API}/unlock", content=body, headers={"content-type": "application/json"})
    assert r.status_code == status
    assert "hunter2" not in r.text and "two" not in r.text and "xxxxx" not in r.text
    assert not [x for x in fake_suite_calls(env.wt) if x["argv"][:2] == ["suite", "unlock"]]


def test_unlock_from_a_foreign_page_never_reaches_the_cli(env: Env) -> None:
    r = _client().post(
        f"{API}/unlock",
        json={"password": FAKE_PASSWORD},
        headers={"Origin": "https://evil.example"},
    )
    assert r.status_code == 403
    assert fake_suite_calls(env.wt) == []


def test_wrong_passwords_back_off(env: Env) -> None:
    c = _client()
    for _ in range(suites.UNLOCK_FAILURES_ALLOWED):
        assert c.post(f"{API}/unlock", json={"password": "wrong wrong wrong"}).status_code == 403
    n_calls = len(fake_suite_calls(env.wt))
    r = c.post(f"{API}/unlock", json={"password": FAKE_PASSWORD})
    assert r.status_code == 429 and r.json()["detail"]["code"] == "too_many_attempts"
    assert len(fake_suite_calls(env.wt)) == n_calls  # refused before the CLI


def test_not_configured_is_its_own_answer(env: Env) -> None:
    set_fake_suite(env.wt, "not-configured")
    c = _client()
    assert c.get(f"{API}/access").json()["access"]["configured"] is False
    r = c.post(f"{API}/unlock", json={"password": FAKE_PASSWORD})
    assert r.status_code == 403 and r.json()["detail"]["code"] == "builder_not_configured"


def test_access_without_tooling_says_so(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo = fake_repo(tmp_path)
    monkeypatch.setattr(launch, "REPO_ROOT", repo)
    cfg = make_config(tmp_path)
    monkeypatch.setenv("VULCANBENCH_RUNNER", "1")
    monkeypatch.setenv("VULCANBENCH_RUNNER_HOME", str(cfg.home))
    monkeypatch.setenv("VULCANBENCH_RUNNER_RUN_WORKTREES", str(cfg.run_worktrees))
    r = _client().get(f"{API}/access").json()
    assert r["access"] is None and r["tooling"]["ready"] is False
    unlock = _client().post(f"{API}/unlock", json={"password": FAKE_PASSWORD})
    assert unlock.status_code == 409 and unlock.json()["detail"]["code"] == "tooling_not_ready"


# -------------------------------------------------------------- suites --


def test_create_suite_needs_the_unlock_then_lists_it(env: Env) -> None:
    c = _client()
    body = {"id": "acme-api", "display_name": "Acme API", "repo": "acme/api"}
    locked = c.post(API, json=body)
    assert locked.status_code == 403 and locked.json()["detail"]["code"] == "builder_locked"
    set_fake_suite(env.wt, "unlocked")
    made = c.post(API, json=body)
    assert made.status_code == 201
    assert made.json()["id"] == "acme-api" and made.json()["status"] == "draft"
    init = [x for x in fake_suite_calls(env.wt) if x["argv"][1] == "init"][-1]["argv"]
    assert init[init.index("--suites-home") + 1] == str(env.cfg.suites_home)
    assert c.post(API, json=body).status_code == 409
    listing = c.get(API).json()
    assert [s["id"] for s in listing["suites"]] == ["acme-api"]
    assert listing["tooling"]["ready"] is True
    assert listing["suites_home"] == str(env.cfg.suites_home)


@pytest.mark.parametrize(
    ("body", "status"),
    [
        ({"id": "demo-suite", "display_name": "x", "repo": "a/b"}, 409),  # built-in collision
        ({"id": "Bad_Id", "display_name": "x", "repo": "a/b"}, 422),
        ({"id": "access", "display_name": "x", "repo": "a/b"}, 422),
        ({"id": "good-id", "display_name": "x", "repo": "not a repo"}, 422),
    ],
)
def test_create_suite_refusals(env: Env, body: dict[str, str], status: int) -> None:
    assert _unlocked(env).post(API, json=body).status_code == status


def test_prs_are_cached_refreshable_and_name_scaffolded_tasks(env: Env) -> None:
    fake_builder_task(env.cfg, "acme-api", "pr-40-fix", pr=40)
    c = _unlocked(env)
    first = c.get(f"{API}/acme-api/prs", params={"since": "2026-08-01", "limit": 10})
    assert first.status_code == 200
    prs = first.json()["prs"]
    assert [p["number"] for p in prs] == [40, 41, 42]
    assert prs[0]["task_id"] == "pr-40-fix" and prs[1]["task_id"] is None
    n = len(fake_suite_calls(env.wt))
    again = c.get(f"{API}/acme-api/prs", params={"since": "2026-08-01", "limit": 10})
    assert again.json()["prs"] == prs and len(fake_suite_calls(env.wt)) == n
    c.get(f"{API}/acme-api/prs", params={"since": "2026-08-01", "limit": 10, "refresh": True})
    assert len(fake_suite_calls(env.wt)) == n + 1
    argv = fake_suite_calls(env.wt)[-1]["argv"]
    assert argv[:6] == ["suite", "prs", "list", "--suite", "acme-api", "--limit"]
    assert "--since" in argv and "--suites-home" in argv
    assert c.get(f"{API}/acme-api/prs", params={"since": "Oct 1"}).status_code == 422


def test_prs_relay_gh_problems(env: Env) -> None:
    fake_builder_task(env.cfg, "acme-api", "pr-40-fix", pr=40)
    c = _unlocked(env)
    set_fake_suite(env.wt, "prs-mode", "gh_auth")
    r = c.get(f"{API}/acme-api/prs")
    assert r.status_code == 409
    assert r.json()["detail"]["code"] == "gh_auth" and "gh auth login" in r.json()["detail"]["hint"]
    set_fake_suite(env.wt, "prs-mode", "network")
    assert c.get(f"{API}/acme-api/prs", params={"refresh": True}).status_code == 502
    assert c.get(f"{API}/no-such-suite/prs").status_code == 404


def test_scaffold_then_refuse_duplicates_and_frozen(env: Env) -> None:
    c = _unlocked(env)
    c.post(API, json={"id": "acme-api", "display_name": "Acme", "repo": "acme/api"})
    r = c.post(f"{API}/acme-api/tasks", json={"pr": 41})
    assert r.status_code == 201
    row = r.json()
    assert row["id"] == "pr-41-fix-thing" and row["stage"] == "scaffolded" and row["pr"] == 41
    assert row["title"] == "Fix thing"
    dup = c.post(f"{API}/acme-api/tasks", json={"pr": 41})
    assert dup.status_code == 409 and "already has task" in dup.json()["detail"]["message"]
    set_fake_suite(env.wt, "scaffold-mode", "network")
    assert c.post(f"{API}/acme-api/tasks", json={"pr": 42}).status_code == 502
    (env.cfg.suites_home / "acme-api" / "suite.lock.json").write_text("{}")
    frozen = c.post(f"{API}/acme-api/tasks", json={"pr": 43})
    assert frozen.status_code == 409 and frozen.json()["detail"]["code"] == "suite_frozen"


def test_stages_come_from_disk(env: Env) -> None:
    cfg = env.cfg
    fake_builder_task(cfg, "acme-api", "t-scaffolded", pr=1)
    fake_builder_task(cfg, "acme-api", "t-authored", pr=2, author="ok")
    fake_builder_task(cfg, "acme-api", "t-author-failed", pr=3, author="failed")
    fake_builder_task(cfg, "acme-api", "t-validated", pr=4, author="ok", validate="PASS")
    fake_builder_task(cfg, "acme-api", "t-failed", pr=5, author="ok", validate="FAIL")
    stale = fake_builder_task(cfg, "acme-api", "t-stale", pr=6, author="ok", validate="PASS")
    a = json.loads((stale / "build" / "author.json").read_text())
    a["finished_at"] = "2026-11-04T13:00:00+00:00"  # authored again after the validation
    (stale / "build" / "author.json").write_text(json.dumps(a))
    fake_builder_task(
        cfg, "acme-api", "t-accepted", pr=7, author="ok", validate="WARN", accepted=True
    )
    detail = _client().get(f"{API}/acme-api").json()
    stages = {t["id"]: t["stage"] for t in detail["tasks"]}
    assert stages == {
        "t-accepted": "validated",
        "t-author-failed": "author_failed",
        "t-authored": "authored",
        "t-failed": "validation_failed",
        "t-scaffolded": "scaffolded",
        "t-stale": "authored",
        "t-validated": "validated",
    }
    rows = {t["id"]: t for t in detail["tasks"]}
    assert rows["t-accepted"]["accepted"] is True
    assert rows["t-authored"]["author"]["cost_usd"] == 1.25
    assert rows["t-failed"]["validate"]["reasons"] == ["gold does not solve"]
    assert detail["freeze_ready"]["ok"] is True
    assert "ultra" not in detail["options"]["efforts"]
    assert "vulcan" not in detail["options"]["harnesses"]
    assert detail["suite"]["stages"]["validated"] == 2


def test_active_jobs_set_the_stage(env: Env) -> None:
    fake_builder_task(env.cfg, "acme-api", "t-one", pr=1, author="ok")
    state = State(env.cfg.home / "runner.db")
    job = state.create_job("validate", "validate-t-one", {"suite": "acme-api", "task": "t-one"})
    detail = _client().get(f"{API}/acme-api").json()
    row = detail["tasks"][0]
    assert row["stage"] == "validating" and row["active_job"]["id"] == job.id
    assert row["active_job"]["run_id"] == f"job:{job.id}"
    assert detail["freeze_ready"]["ok"] is False


def test_task_detail_caps_redacts_and_refuses_traversal(env: Env) -> None:
    t = fake_builder_task(env.cfg, "acme-api", "t-one", pr=1, author="ok")
    (t / "tests" / "big.py").write_text("x" * (suites.TEST_FILE_CAP + 10))
    (t / "tests" / "blob.bin").write_bytes(b"\x00\x01" * 10)
    (t / "issue.md").write_text("token: sk-ant-api03-" + "a" * 90 + "\n")
    c = _client()
    d = c.get(f"{API}/acme-api/tasks/t-one").json()
    files = {f["path"]: f for f in d["tests"]}
    assert files["tests/big.py"]["truncated"] is True
    assert len(files["tests/big.py"]["text"]) == suites.TEST_FILE_CAP
    assert files["tests/blob.bin"]["binary"] is True and files["tests/blob.bin"]["text"] is None
    assert "sk-ant-api03" not in d["issue_md"]["text"]
    assert d["metadata"]["tests"]["fail_to_pass"] == ["tests/vb_test_fix.py"]
    assert d["gold_patch"]["text"].startswith("--- a/main.py")
    for bad in ("..%2F..%2Fetc", ".hidden", "no-such-task"):
        assert c.get(f"{API}/acme-api/tasks/{bad}").status_code == 404
    assert c.get(f"{API}/Not_An_Id").status_code == 404


def test_accept_and_drop(env: Env) -> None:
    cfg = env.cfg
    fake_builder_task(cfg, "acme-api", "t-ok", pr=1, author="ok", validate="PASS")
    fake_builder_task(cfg, "acme-api", "t-bad", pr=2, author="ok", validate="FAIL")
    c = _client()
    assert c.post(f"{API}/acme-api/tasks/t-ok/accept").status_code == 403  # locked
    set_fake_suite(env.wt, "unlocked")
    bad = c.post(f"{API}/acme-api/tasks/t-bad/accept")
    assert bad.status_code == 409 and bad.json()["detail"]["code"] == "not_validated"
    ok = c.post(f"{API}/acme-api/tasks/t-ok/accept")
    assert ok.status_code == 200 and ok.json()["accepted"] is True
    manifest = json.loads((cfg.suites_home / "acme-api" / "suite.json").read_text())
    assert manifest["tasks"] == ["t-ok"]
    dropped = c.post(f"{API}/acme-api/tasks/t-ok/drop", json={"reason": "tests too weak"})
    assert dropped.status_code == 200
    assert dropped.json()["dropped"] is True and dropped.json()["accepted"] is False
    assert dropped.json()["dropped_reason"] == "tests too weak"
    assert (cfg.suites_home / "acme-api" / "t-ok").is_dir()
    assert c.post(f"{API}/acme-api/tasks/t-ok/accept").status_code == 409


def test_drop_needs_the_unlock_even_for_a_task_not_in_the_list(env: Env) -> None:
    fake_builder_task(env.cfg, "acme-api", "t-one", pr=1)
    r = _client().post(f"{API}/acme-api/tasks/t-one/drop")
    assert r.status_code == 403
    assert not (env.cfg.suites_home / "acme-api" / "t-one" / "build" / "dropped.json").exists()


def test_freeze_checks_first_then_shows_up_in_the_launch_picker(env: Env) -> None:
    cfg = env.cfg
    fake_builder_task(cfg, "acme-api", "t-ok", pr=1, author="ok", validate="PASS")
    c = _unlocked(env)
    early = c.post(f"{API}/acme-api/freeze")
    assert early.status_code == 409
    assert "accept at least one validated task" in early.json()["detail"]["problems"]
    c.post(f"{API}/acme-api/tasks/t-ok/accept")
    frozen = c.post(f"{API}/acme-api/freeze")
    assert frozen.status_code == 201
    assert frozen.json()["n_tasks"] == 1 and frozen.json()["lock_digest"]
    detail = c.get(f"{API}/acme-api").json()
    assert detail["suite"]["status"] == "frozen" and detail["freeze_ready"]["ok"] is False
    opts = c.get("/api/runner/options", params={"tag": BUILDER_TAG}).json()
    custom = [s for s in opts["suites"] if s.get("custom")]
    assert [s["id"] for s in custom] == ["acme-api"]
    assert c.post(f"{API}/acme-api/tasks/t-ok/drop").status_code == 409

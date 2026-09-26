"""incident-root-cause (Verdict v2): construction, re-derivation, balance, shortcuts, determinism.

The answer is re-derived here without the generator's propagation code: from
the stored causal onsets (every symptom traces back over real dependency
edges to exactly one causeless, earliest service), and from the rendered log
text itself (parse every line in all four formats, undo each host's clock
offset using only the ntp lines printed in the logs, resolve addresses and
routes using only the header and the access lines, then follow each victim's
first complaint to the service it points at until the chain stops).
"""

import json
import re
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path

import pytest

from harness.verdict.v2.families import ops_gen
from harness.verdict.v2.items import MAX_STATE_CHARS, labels_for, validate
from harness.verdict.v2.registry import BY_ID, BuildContext, builder_for

REPO = Path(__file__).resolve().parents[1]
FAMILY = "incident-root-cause"
FULL = 250


def _ctx(seed, per_family):
    return BuildContext(seed=seed, repo=REPO, per_family=per_family)


@pytest.fixture(scope="module")
def full():
    return ops_gen.generate(_ctx(11, FULL))


# --------------------------------------------------------------------------
# An independent log parser
# --------------------------------------------------------------------------

MONTHS = {
    m: i + 1
    for i, m in enumerate(
        ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")
    )
}
PLAIN = re.compile(
    r"^(\d{4})-(\d\d)-(\d\d) (\d\d):(\d\d):(\d\d)\.(\d{3}) (DEBUG|INFO|WARN|ERROR) +\[([a-z]+)\] (.*)$"
)
SYSLOG = re.compile(
    r"^([A-Z][a-z]{2}) +(\d{1,2}) (\d\d):(\d\d):(\d\d)\.(\d{3}) \S+ ([a-z]+)\[\d+\]: "
    r"(DEBUG|INFO|WARN|ERROR) (.*)$"
)
KV = re.compile(r'^ts=(\S+)Z level=([a-z]+) svc=([a-z]+) msg="([^"]*)" host=\S+$')
NTP = re.compile(r"local clock is (?:now )?([\d.]+)s (behind|ahead of) reference time")


def _iso(text):
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%S.%f").replace(tzinfo=UTC)


def parse_line(line, year):
    """(timestamp, service, LEVEL, message) from a rendered line in any format."""
    if line.startswith("{"):
        rec = json.loads(line)
        return _iso(rec["ts"].rstrip("Z")), rec["service"], rec["level"].upper(), rec["msg"]
    if m := PLAIN.match(line):
        y, mo, d, h, mi, s, ms = (int(g) for g in m.groups()[:7])
        return datetime(y, mo, d, h, mi, s, ms * 1000, tzinfo=UTC), m[9], m[8], m[10]
    if m := KV.match(line):
        return _iso(m[1]), m[3], m[2].upper(), m[4]
    m = SYSLOG.match(line)
    assert m, line
    h, mi, s, ms = (int(g) for g in m.groups()[2:6])
    stamp = datetime(year, MONTHS[m[1]], int(m[2]), h, mi, s, ms * 1000, tzinfo=UTC)
    return stamp, m[7], m[8], m[9]


def parse_state(state):
    """(service names, {ip: name}, rows) from a rendered state."""
    head, logs = state.split("\nLogs:\n")
    year = int(re.search(r"services, (\d{4})-", head)[1])
    listed = re.search(r"Services \(address\): (.*)\.\n", head)[1]
    pairs = re.findall(r"([a-z]+) \((\d+\.\d+\.\d+\.\d+)\)", listed)
    rows = [parse_line(line, year) for line in logs.splitlines()]
    return [name for name, _ in pairs], {ip: name for name, ip in pairs}, rows


def corrected_ms(rows, base):
    """Each row's true time in ms after base, undoing offsets the ntp lines disclose."""
    shifts = {}  # service -> (displayed ms from which the offset applies, offset ms)
    disp = [round((ts - base).total_seconds() * 1000) for ts, *_ in rows]
    for d, (_, service, _, msg) in zip(disp, rows, strict=True):
        if m := NTP.search(msg):
            off = round(float(m[1]) * 1000) * (-1 if m[2] == "behind" else 1)
            start = d if "clock stepped" in msg else -(10**12)
            shifts[service] = (start, off)
    out = []
    for d, (_, service, _, _) in zip(disp, rows, strict=True):
        start, off = shifts.get(service, (0, 0))
        out.append(d - off if d >= start else d)
    return out


def names_in(msg, known, skip):
    return [tok for tok in re.findall(r"[a-z]+", msg) if tok in known and tok != skip]


ACCESS = re.compile(r"^(?:GET|POST|PUT) (/v1/[a-z]+) 200 \d+ms$")


def route_owners(rows):
    """Route -> the service whose plain access lines serve it, read from the logs."""
    owners = {}
    for _, service, _, msg in rows:
        if m := ACCESS.match(msg):
            owners.setdefault(m[1], set()).add(service)
    assert all(len(v) == 1 for v in owners.values()), owners
    return {route: next(iter(v)) for route, v in owners.items()}


def referenced(msg, known, by_ip, routes, skip):
    """Services a line refers to: by name, by IP and port, or by a route it serves."""
    named = names_in(msg, known, skip)
    named += [by_ip[ip] for ip in re.findall(r"\b(\d+\.\d+\.\d+\.\d+):\d+", msg) if ip in by_ip]
    named += [routes[r] for r in re.findall(r"(/v1/[a-z]+)", msg) if r in routes]
    return [n for n in named if n != skip]


# --------------------------------------------------------------------------
# Registry, validity, determinism
# --------------------------------------------------------------------------


def test_builder_is_registered():
    family = BY_ID[FAMILY]
    assert family.module == "ops_gen"
    assert family.reference == "generator"
    assert family.question_type == "choice"
    assert builder_for(FAMILY) is ops_gen.BUILDERS[FAMILY]


def test_items_validate_and_fit(full):
    assert len(full) == FULL
    assert len({item.item_id for item, _ in full}) == FULL
    for item, draft in full:
        validate(item)
        assert item.family == FAMILY
        assert item.reference == "generator"
        assert len(item.state) < MAX_STATE_CHARS
        q = item.question
        assert q["instructions"] == "Which service is the root cause of this incident?"
        topo = draft.spec["topology"]
        assert sorted(q["options"]) == sorted(topo.names)
        assert ops_gen.MIN_SERVICES <= len(q["options"]) <= ops_gen.MAX_SERVICES
        assert item.answer == draft.spec["root"]
        services, by_ip, rows = parse_state(item.state)
        assert services == sorted(topo.names)
        assert by_ip == {topo.service(n).ip: n for n in topo.names}
        assert ops_gen.MIN_LINES <= len(rows) <= ops_gen.MAX_LINES
        assert len({topo.service(n).fmt for n in topo.names}) >= 2
        assert set(item.shortcuts) == set(ops_gen.SHORTCUTS)


def test_fifty_seed_groups_of_five(full):
    units = Counter(item.source_unit for item, _ in full)
    assert len(units) == 50
    assert set(units.values()) == {5}
    assert all(re.fullmatch(rf"{FAMILY}-g\d\d", unit) for unit in units)
    assert {item.split for item, _ in full} == {"dev", "test"}
    by_unit = {}
    for item, _ in full:
        assert by_unit.setdefault(item.source_unit, item.split) == item.split
        assert item.source["seed_group"] == int(item.source_unit[-2:])
    # One topology per seed group.
    topo_of = {}
    for item, draft in full:
        assert (
            topo_of.setdefault(item.source_unit, draft.spec["topology"]) is draft.spec["topology"]
        )


def test_no_dashes_in_any_text(full):
    banned = re.compile("[" + chr(0x2013) + chr(0x2014) + "]")  # en and em dash
    for item, _ in full:
        assert not banned.search(item.state)
        assert not banned.search(item.question["instructions"])
        assert not any(banned.search(o) for o in item.question["options"])


def test_builds_are_deterministic():
    build = ops_gen.BUILDERS[FAMILY]
    a = [i.to_json() for i in build(_ctx(5, 10))]
    b = [i.to_json() for i in build(_ctx(5, 10))]
    c = [i.to_json() for i in build(_ctx(6, 10))]
    assert a == b
    assert a != c


def test_small_builds_still_work():
    items = ops_gen.BUILDERS[FAMILY](_ctx(9, 7))
    assert len(items) == 7
    for item in items:
        validate(item)


# --------------------------------------------------------------------------
# Ground truth, re-derived
# --------------------------------------------------------------------------

ROLES_FOR = {
    "pool_exhaustion": {"api", "worker"},
    "bad_config": {"api", "worker", "gateway", "scheduler"},
    "disk_full": {"db", "queue", "cache", "worker"},
    "cert_expiry": {"api", "db", "cache", "queue", "gateway"},
    "oom": {"api", "worker", "cache", "queue", "db"},
    "clock_skew": {"gateway", "api", "scheduler"},
    "slow_dependency": {"db", "cache", "api", "queue"},
    "poison_message": {"worker"},
    "dns_failure": {"api", "worker", "gateway"},
}
FIRST_ANOMALY = {
    "pool_exhaustion": r"^connection pool \S+: \d+/\d+ in use",
    "bad_config": r"^deploying version ",
    "disk_full": r"^disk usage on \S+ at 9\d%",
    "cert_expiry": r"^tls: serving certificate .* expires in \d+s",
    "oom": r"^heap usage \d+% of \d+MiB",
    "clock_skew": r"^ntp: clock stepped",
    "slow_dependency": r"VACUUM FULL|fork took|lock contention|partition reassignment",
    "poison_message": r"^handler failed on message",
    "dns_failure": r"^resolv\.conf reloaded",
}
# When the root is generic it shows only plain errors or latency, no mechanism.
GENERIC_ANOMALY = (
    r"internal error|internal broker error|^slow query|latency p99|^p99 latency"
    r"|exited unexpectedly|bad certificate"
)


def test_root_rederived_from_the_simulation(full):
    """One causeless onset; every symptom chains back to it over real edges, in time order."""
    for item, draft in full:
        spec = draft.spec
        topo, root, kind = spec["topology"], spec["root"], spec["kind"]
        assert topo.role(root) in ROLES_FOR[kind]
        onsets = {o.service: o for o in spec["onsets"]}
        assert onsets, item.item_id
        assert root not in onsets
        assert all(o.cause is not None for o in onsets.values())
        adjacent = {(e.src, e.dst) for e in topo.edges} | {(e.dst, e.src) for e in topo.edges}
        start = {**{s: o.t for s, o in onsets.items()}, root: spec["t_f"]}
        for o in onsets.values():
            assert (o.service, o.cause) in adjacent
            assert start[o.cause] < o.t
            seen, node = set(), o.service
            while node != root:
                assert node not in seen
                seen.add(node)
                node = onsets[node].cause
        assert min(start, key=start.get) == root


def test_rendered_logs_recover_the_root(full):
    """Undo skew from the ntp lines, check the root's first anomaly leads, follow the blame."""
    for item, draft in full:
        spec = draft.spec
        root, records = spec["root"], spec["records"]
        services, by_ip, rows = parse_state(item.state)
        known = set(services)
        assert len(rows) == len(records)
        stamps = [ts for ts, *_ in rows]
        assert stamps == sorted(stamps)
        fixed = corrected_ms(rows, spec["base"])
        for row, rec, t in zip(rows, records, fixed, strict=True):
            assert (row[1], row[2], row[3]) == (rec["service"], rec["level"], rec["msg"])
            assert t == rec["t_ms"], item.item_id
        firsts = [i for i, r in enumerate(records) if r["cat"] == "root_first"]
        assert len(firsts) == 1
        lead = firsts[0]
        assert rows[lead][1] == root
        explicit = item.source["root_explicit"]
        pattern = FIRST_ANOMALY[spec["kind"]] if explicit else GENERIC_ANOMALY
        assert re.search(pattern, rows[lead][3]), rows[lead][3]
        symptom_times = [t for t, r in zip(fixed, records, strict=True) if r["cat"] == "symptom"]
        assert symptom_times and fixed[lead] < min(symptom_times)
        # Nothing on the root looks wrong before its first anomaly (a disclosed clock
        # offset aside).
        for t, r in zip(fixed, records, strict=True):
            if r["service"] == root and t < fixed[lead]:
                assert r["cat"] == "ntp" or (
                    r["level"] in ("DEBUG", "INFO") and r["cat"] == "noise"
                )
        # Each victim fails, then its first complaint that points anywhere points at one
        # service; following those complaints always ends at the root.
        routes = route_owners(rows)
        first_symptom, first_complaint = {}, {}
        for t, row, r in sorted(zip(fixed, rows, records, strict=True), key=lambda x: x[0]):
            if r["cat"] != "symptom":
                continue
            first_symptom.setdefault(row[1], t)
            named = referenced(row[3], known, by_ip, routes, row[1])
            if named and row[1] not in first_complaint:
                assert len(set(named)) == 1, row[3]
                first_complaint[row[1]] = named[0]
        assert set(first_complaint) == set(first_symptom) == {o.service for o in spec["onsets"]}
        for service in first_complaint:
            node, hops = service, 0
            while node in first_complaint:
                blamed = first_complaint[node]
                cause_start = fixed[lead] if blamed == root else first_symptom[blamed]
                assert cause_start < first_symptom[node]
                node, hops = blamed, hops + 1
                assert hops <= len(services)
            assert node == root


def test_every_skewed_host_is_disclosed(full):
    for item, draft in full:
        spec = draft.spec
        _, _, rows = parse_state(item.state)
        disclosed = {service for _, service, _, msg in rows if NTP.search(msg)}
        assert disclosed == set(spec["skews"])
        herring = [s for s in spec["skews"] if s != spec["root"]]
        if spec["kind"] == "clock_skew":
            assert set(spec["skews"]) == {spec["root"]}
        assert len(herring) <= 1


# --------------------------------------------------------------------------
# Balance and shortcuts
# --------------------------------------------------------------------------


def test_answer_positions_are_balanced_per_option_count(full):
    by_k = {}
    for item, _ in full:
        options = item.question["options"]
        by_k.setdefault(len(options), Counter())[options.index(item.answer)] += 1
    for k, counts in by_k.items():
        low = min(counts.get(p, 0) for p in range(k))
        assert max(counts.values()) - low <= 1, (k, counts)


def test_fault_kinds_roles_and_knobs_are_spread(full):
    kinds = Counter(d.source["fault_kind"] for _, d in full)
    assert set(kinds) == set(ops_gen.KINDS)
    assert max(kinds.values()) - min(kinds.values()) <= 4
    roles = Counter(d.source["root_role"] for _, d in full)
    assert len(roles) == 7
    assert max(roles.values()) / FULL <= 0.5
    shown = sum(d.source["show_map"] for _, d in full)
    assert 0.4 * FULL <= shown <= 0.6 * FULL
    assert sum(d.source["skew_herring"] != "none" for _, d in full) >= 0.3 * FULL
    assert sum(d.source["decoy_deploy"] for _, d in full) >= 0.3 * FULL
    assert sum(d.source["contained_fault"] for _, d in full) >= 0.3 * FULL
    addressing = Counter(d.source["addressing"] for _, d in full)
    assert set(addressing) == {"name", "ip", "path"}
    assert min(addressing.values()) >= 0.15 * FULL
    assert sum(d.source["skew_herring"] == "root" for _, d in full) >= 0.05 * FULL
    assert 0.3 * FULL <= sum(not d.source["root_explicit"] for _, d in full) <= 0.7 * FULL
    lines = [d.source["lines"] for _, d in full]
    assert min(lines) < 150 and max(lines) > 300


def test_every_shortcut_stays_near_chance(full):
    chance = sum(1 / len(labels_for(item.question)) for item, _ in full) / FULL
    for name in ops_gen.SHORTCUTS:
        hits = sum(item.shortcuts[name] == item.answer for item, _ in full) / FULL
        assert abs(hits - chance) <= 0.10, (name, hits, chance)


def test_log_shortcuts_recompute_from_the_rendered_state(full):
    for item, _ in full:
        services, _, rows = parse_state(item.state)
        known = set(services)
        errors = [s for _, s, level, _ in rows if level == "ERROR"]
        err_count = Counter(errors)
        line_count = Counter(s for _, s, _, _ in rows)
        blamed = Counter()
        for _, s, level, msg in rows:
            if level in ("WARN", "ERROR"):
                blamed.update(names_in(msg, known, s))
        hit = next(
            ((s, msg) for _, s, _, msg in rows if re.search(r"\broot\b|caused", msg, re.I)), None
        )
        mentioned = names_in(hit[1], known, hit[0]) if hit else []
        expect = {
            "first_error_service": errors[0] if errors else rows[0][1],
            "last_error_service": errors[-1] if errors else rows[-1][1],
            "most_errors_service": max(services, key=lambda n: err_count[n]),
            "most_log_lines_service": max(services, key=lambda n: line_count[n]),
            "most_blamed_service": max(services, key=lambda n: blamed[n]),
            "root_or_caused_line": (mentioned[0] if mentioned else hit[0]) if hit else rows[0][1],
            "first_listed_option": item.question["options"][0],
        }
        for name, guess in expect.items():
            assert item.shortcuts[name] == guess, (item.item_id, name)


def test_topology_shortcuts_recompute(full):
    for item, draft in full:
        topo = draft.spec["topology"]
        names = sorted(topo.names)
        indegree = Counter(e.dst for e in topo.edges)
        assert item.shortcuts["most_depended_on"] == max(names, key=lambda n: indegree[n])
        calls = {
            n: [e.dst for e in topo.edges if e.src == n and e.kind != "consume"] for n in names
        }
        tops = [n for n in names if not indegree[n]]
        depth = dict.fromkeys(names, 0)
        for top in tops:
            frontier = [(top, 0)]
            while frontier:
                node, d = frontier.pop()
                depth[node] = max(depth[node], d)
                frontier += [(c, d + 1) for c in calls[node]]
        leaves = [n for n in names if not calls[n]]
        assert item.shortcuts["deepest_leaf"] == max(leaves, key=lambda n: depth[n])

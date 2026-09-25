"""Operations family for Verdict v2: which service caused this incident?

``incident-root-cause`` simulates a small system of 5 to 12 services with a
generated dependency graph (nonsense names; gateway, APIs, workers, queues,
caches, databases and a scheduler), injects exactly one root fault into one
service and propagates its symptoms along the graph with realistic delays:
timeouts, retries, circuit breakers opening, 5xx at callers, queue backlog,
backpressure and health check flaps. The logs of every service are rendered
in its own format (plain, syslog, key=value or JSON lines), merged by the
timestamp each host recorded, and padded with benign noise and red herrings:
an unrelated WARN storm (often complaining about a client), a routine deploy
elsewhere, cause-like warnings on healthy services, a dramatic but contained
fault that recovers on a service the incident never reaches, a service that
fails a background task all day, a victim that logs the most errors or fails
dramatically itself once the incident reaches it, a healthy dependency one
caller keeps calling slow, a transient error before the incident and, unless
the fault is clock skew, one host (sometimes the root's) whose clock is off;
it says so in an ntp line, so the reader can correct it.

Difficulty knobs, per item: whether the root names its own mechanism or only
shows plain errors or latency (then victims' errors do not leak it either);
whether victims refer to upstreams by name, by IP and port (the header maps
addresses to services) or by route (every route appears in its owner's
access log before the incident); whether victims first fail generically and
only then say where; whether the dependency map is shown; line count,
service count, root log volume and level.

The answer is the faulted service, known by construction. The trace is
recoverable by careful reading: the root's own first anomaly precedes every
downstream symptom once timestamps are corrected, and each victim's first
complaint that points anywhere points (by name, address or route) at the
service that caused its failure.

Balancing: the answer sits at a cycling option position per option count,
and for each item the builder makes several candidates (different root,
fault kind, herrings and volumes on the seed group's topology) and keeps the
one that holds every recorded shortcut closest to chance.

``generate(ctx)`` returns each item with the structured simulation it came
from, so the tests can re-derive the answer independently. ``BUILDERS`` is
what the registry imports.
"""

from __future__ import annotations

import heapq
import json
import math
import random
import re
from collections import Counter
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from datetime import UTC, datetime, timedelta
from typing import Any

from harness.verdict.v2.items import (
    MAX_STATE_CHARS,
    Item,
    answer_label,
    choice_question,
    labels_for,
    make_item,
)
from harness.verdict.v2.registry import BuildContext

FAMILY = "incident-root-cause"
REFERENCE = "generator"
ITEMS_PER_UNIT = 5
CANDIDATES = 20
MIN_LINES, MAX_LINES = 80, 400
MIN_SERVICES, MAX_SERVICES = 5, 12
QUESTION = "Which service is the root cause of this incident?"
KINDS = (
    "pool_exhaustion",
    "bad_config",
    "disk_full",
    "cert_expiry",
    "oom",
    "clock_skew",
    "slow_dependency",
    "poison_message",
    "dns_failure",
)
SHORTCUTS = (
    "first_error_service",
    "most_errors_service",
    "most_log_lines_service",
    "last_error_service",
    "most_depended_on",
    "deepest_leaf",
    "first_listed_option",
    "root_or_caused_line",
    "most_blamed_service",
)
SYNC = frozenset({"http", "grpc", "sql", "cache"})
# Modes in which a service fails or slows the requests it serves, so the
# services that depend on it see symptoms of their own.
IMPAIRED = frozenset({"errors", "flap", "slow", "tls", "load", "throttled", "overload"})
LEVELS = ("DEBUG", "INFO", "WARN", "ERROR")
HORIZON = 600.0  # seconds after the first anomaly that propagation may reach
MONTHS = ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")
_ROOT_WORD = re.compile(r"\broot\b|caused", re.IGNORECASE)
_TOKEN = re.compile(r"[a-z]+")


# --------------------------------------------------------------------------
# Names and topology
# --------------------------------------------------------------------------

# Service names start with onsets English words do not use, so a name never
# collides with a word in a log template and a token match means a mention.
_NAME_ONSETS = ("vr", "zh", "kv", "tz", "q", "dv", "gv", "zv", "pf", "xh")
_ONSETS = (
    "b", "d", "f", "g", "k", "l", "m", "n", "p", "r", "s", "t", "v",
    "br", "dr", "gr", "kl", "pl", "st", "tr", "sk",
)  # fmt: skip
_VOWELS = ("a", "e", "i", "o", "u", "ai", "ei", "ou")
_CODAS = ("", "", "", "n", "r", "l", "m", "k", "x", "s")


def _word(rng: random.Random, first: Sequence[str], syllables: int) -> str:
    parts = [rng.choice(first) + rng.choice(_VOWELS)]
    parts += [rng.choice(_ONSETS) + rng.choice(_VOWELS) for _ in range(syllables - 1)]
    return "".join(parts) + rng.choice(_CODAS)


def _service_names(rng: random.Random, n: int) -> list[str]:
    names: list[str] = []
    while len(names) < n:
        name = _word(rng, _NAME_ONSETS, rng.choice((2, 2, 2, 3)))
        if not 5 <= len(name) <= 10:
            continue
        if any(name in other or other in name for other in names):
            continue
        names.append(name)
    return names


def _plain_words(rng: random.Random, n: int, avoid: set[str]) -> list[str]:
    words: list[str] = []
    while len(words) < n:
        word = _word(rng, _ONSETS, 2)
        if 4 <= len(word) <= 9 and word not in avoid and word not in words:
            words.append(word)
    return words


@dataclass(frozen=True)
class Service:
    name: str
    role: str
    fmt: str
    host: str
    pid: int

    @property
    def ip(self) -> str:
        return self.host.removeprefix("ip-").replace("-", ".")

    @property
    def port(self) -> int:
        return {"db": 5432, "cache": 6379, "queue": 9092}.get(self.role, 8080)


@dataclass(frozen=True)
class Edge:
    """``src`` depends on ``dst``: calls it, publishes to it or consumes from it."""

    src: str
    dst: str
    kind: str


@dataclass
class Topology:
    services: list[Service]
    edges: list[Edge]
    paths: dict[str, list[str]]
    tables: dict[str, list[str]]
    topics: dict[str, str]
    jobs: list[str]
    words: list[str]
    _by_name: dict[str, Service] = field(default_factory=dict, repr=False)

    def __post_init__(self) -> None:
        self._by_name = {s.name: s for s in self.services}

    @property
    def names(self) -> list[str]:
        return [s.name for s in self.services]

    def service(self, name: str) -> Service:
        return self._by_name[name]

    def role(self, name: str) -> str:
        return self._by_name[name].role

    def deps(self, name: str) -> list[Edge]:
        return [e for e in self.edges if e.src == name]

    def dependents(self, name: str) -> list[Edge]:
        return [e for e in self.edges if e.dst == name]

    def of_role(self, *roles: str) -> list[str]:
        return [s.name for s in self.services if s.role in roles]

    def edge(self, src: str, dst: str) -> Edge | None:
        return next((e for e in self.edges if e.src == src and e.dst == dst), None)


def _role_counts(rng: random.Random, n: int) -> dict[str, int]:
    counts = {"gateway": 1, "db": 1, "cache": 0, "queue": 0, "worker": 0, "scheduler": 0}
    if n == 5:
        pick = rng.random()
        if pick < 0.4:
            counts["queue"] = counts["worker"] = 1
        elif pick < 0.7:
            counts["cache"] = 1
    else:
        if rng.random() < (0.7 if n < 10 else 0.95):
            counts["queue"] = 1 + int(n >= 11 and rng.random() < 0.4)
            counts["worker"] = counts["queue"] + int(n >= 10 and rng.random() < 0.4)
        counts["cache"] = int(rng.random() < 0.55) + int(n >= 11 and rng.random() < 0.3)
        counts["scheduler"] = int(n >= 7 and rng.random() < 0.5)
        counts["db"] += int(n >= 8 and rng.random() < 0.5) + int(n >= 11 and rng.random() < 0.3)
    min_api = 1 if n == 5 else 2
    for role in ("scheduler", "cache", "db", "worker"):
        while n - sum(counts.values()) < min_api and counts[role] > (1 if role == "db" else 0):
            if role == "worker" and counts["worker"] <= counts["queue"]:
                break
            counts[role] -= 1
    counts["api"] = n - sum(counts.values())
    return counts


class _EdgeSet:
    def __init__(self) -> None:
        self.edges: list[Edge] = []

    def add(self, src: str, dst: str, kind: str) -> None:
        if src != dst and not any(e.src == src and e.dst == dst for e in self.edges):
            self.edges.append(Edge(src, dst, kind))

    def has_out(self, src: str, kinds: frozenset[str] | None = None) -> bool:
        return any(e.src == src and (kinds is None or e.kind in kinds) for e in self.edges)


def _wire(rng: random.Random, by_role: dict[str, list[str]]) -> list[Edge]:  # noqa: PLR0912, one block per role
    es = _EdgeSet()
    gw, apis = by_role["gateway"][0], by_role["api"]
    dbs, caches, queues = by_role["db"], by_role["cache"], by_role["queue"]
    workers, scheds = by_role["worker"], by_role["scheduler"]
    es.add(gw, apis[0], "http")
    for i, api in enumerate(apis[1:], start=1):
        caller = gw if rng.random() < 0.5 else rng.choice(apis[:i])
        es.add(caller, api, "http" if caller == gw else rng.choice(("http", "grpc")))
    if len(apis) > 1 and rng.random() < 0.3:
        es.add(gw, rng.choice(apis[1:]), "http")
    for db in dbs:
        for caller in rng.sample(apis + workers, rng.randint(1, min(3, len(apis + workers)))):
            es.add(caller, db, "sql")
    for cache in caches:
        for caller in rng.sample(apis, rng.randint(1, min(2, len(apis)))):
            es.add(caller, cache, "cache")
    for q in queues:
        for producer in rng.sample(apis, rng.randint(1, min(2, len(apis)))):
            es.add(producer, q, "publish")
    for i, worker in enumerate(workers):
        es.add(worker, queues[i % len(queues)], "consume")
        if not es.has_out(worker, frozenset({"sql", "cache"})):
            es.add(worker, rng.choice(dbs), "sql")
    for api in apis:
        if not es.has_out(api):
            target = rng.choice(dbs + caches)
            es.add(api, target, "cache" if target in caches else "sql")
    for sched in scheds:
        if queues and rng.random() < 0.6:
            es.add(sched, rng.choice(queues), "publish")
        else:
            es.add(sched, rng.choice(apis), "http")
    return es.edges


def make_topology(rng: random.Random) -> Topology:
    n = rng.randint(MIN_SERVICES, MAX_SERVICES)
    counts = _role_counts(rng, n)
    names = _service_names(rng, n)
    order = ("gateway", "api", "worker", "queue", "cache", "db", "scheduler")
    by_role: dict[str, list[str]] = {}
    roles: list[str] = []
    for role in order:
        by_role[role] = [names[len(roles) + i] for i in range(counts[role])]
        roles += [role] * counts[role]
    fmts = ["plain", "syslog", "kv", "json"]
    rng.shuffle(fmts)
    wanted = fmts[: 2 if n < 7 else 3]
    services: list[Service] = []
    for i, (name, role) in enumerate(zip(names, roles, strict=True)):
        fmt = wanted[i] if i < len(wanted) else rng.choice(wanted)
        host = ""
        while not host or any(s.host == host for s in services):
            host = f"ip-10-{rng.randint(0, 9)}-{rng.randint(0, 31)}-{rng.randint(2, 250)}"
        services.append(Service(name, role, fmt, host, rng.randint(200, 9999)))
    rng.shuffle(services)
    edges = _wire(rng, by_role)
    words = _plain_words(rng, 40, set(names))
    pool = iter(words)
    paths = {a: [f"/v1/{next(pool)}" for _ in range(rng.randint(2, 3))] for a in by_role["api"]}
    tables = {
        d: [f"{next(pool)}_{rng.choice(('entries', 'items', 'events'))}"] for d in by_role["db"]
    }
    topics = {q: f"{next(pool)}.events" for q in by_role["queue"]}
    jobs = [f"{next(pool)}_sync", f"{next(pool)}_rollup"]
    return Topology(services, edges, paths, tables, topics, jobs, list(pool))


def most_depended_on(topo: Topology) -> str:
    return max(sorted(topo.names), key=lambda n: len(topo.dependents(n)))


def deepest_leaf(topo: Topology) -> str:
    level: dict[str, int] = {}
    tops = [n for n in topo.names if not topo.dependents(n)]
    for top in sorted(tops):
        stack = [(top, 0)]
        while stack:
            name, d = stack.pop()
            if level.get(name, -1) >= d:
                continue
            level[name] = d
            stack += [(e.dst, d + 1) for e in topo.deps(name) if e.kind != "consume"]
    leaves = [n for n in topo.names if not any(e.kind != "consume" for e in topo.deps(n))]
    return max(sorted(leaves), key=lambda n: level.get(n, 0))


# --------------------------------------------------------------------------
# The simulation
# --------------------------------------------------------------------------


@dataclass
class Onset:
    service: str
    t: float
    cause: str | None
    mode: str


@dataclass
class Line:
    t: float
    service: str
    level: str
    msg: str
    cat: str  # noise, herring, ntp, root_first, root, symptom
    keep: bool = False


Template = tuple[str, Callable[[], str]]


@dataclass
class Case:
    rng: random.Random
    topo: Topology
    kind: str
    root: str
    t_f: float = 0.0
    t_imp: float = 0.0
    t_err: float = 0.0
    t_end: float = 0.0
    root_mode: str = "errors"
    detail: dict[str, Any] = field(default_factory=dict)
    fallback: frozenset[tuple[str, str]] = frozenset()
    onsets: dict[str, Onset] = field(default_factory=dict)
    skews: dict[str, list[tuple[float, float]]] = field(default_factory=dict)
    lines: list[Line] = field(default_factory=list)
    knobs: dict[str, Any] = field(default_factory=dict)
    # Services that log their failures at WARN rather than ERROR.
    warn_only: frozenset[str] = frozenset()

    def u(self, lo: float, hi: float) -> float:
        return self.rng.uniform(lo, hi)

    def mode_of(self, name: str) -> str:
        return self.root_mode if name == self.root else self.onsets[name].mode

    def emit(self, t: float, service: str, level: str, msg: str, cat: str, keep: bool) -> None:
        if level == "ERROR" and cat in ("root", "symptom") and service in self.warn_only:
            level = "WARN"
        self.lines.append(Line(t, service, level, msg, cat, keep))


def eligible_roots(topo: Topology, kind: str) -> list[str]:
    out = []
    for name in topo.names:
        role, deps = topo.role(name), topo.deps(name)
        kinds = {e.kind for e in deps}
        api_callee = any(e.kind in ("http", "grpc") and topo.role(e.dst) == "api" for e in deps)
        ok = {
            "pool_exhaustion": role in ("api", "worker") and bool(kinds & SYNC),
            "bad_config": role in ("api", "worker", "gateway", "scheduler"),
            "disk_full": role in ("db", "queue", "cache", "worker"),
            "cert_expiry": role in ("api", "db", "cache", "queue", "gateway"),
            "oom": role in ("api", "worker", "cache", "queue", "db"),
            "clock_skew": role in ("gateway", "api", "scheduler") and api_callee,
            "slow_dependency": role in ("db", "cache", "api", "queue"),
            "poison_message": role == "worker",
            "dns_failure": role in ("api", "worker", "gateway") and bool(kinds - {"consume"}),
        }[kind]
        if ok:
            out.append(name)
    return out


def _setup(case: Case) -> None:
    """Fault timing, the mode the root serves in, and kind details."""
    rng, topo, root = case.rng, case.topo, case.root
    role = topo.role(root)
    sync_deps = [e.dst for e in topo.deps(root) if e.kind in SYNC]
    sql_deps = [e.dst for e in topo.deps(root) if e.kind == "sql"]
    kind = case.kind
    gap = {
        "pool_exhaustion": (15, 60),
        "bad_config": (4, 20),
        "disk_full": (30, 90),
        "cert_expiry": (30, 120),
        "oom": (40, 120),
        "clock_skew": (0, 0),
        "slow_dependency": (5, 25),
        "poison_message": (1, 3),
        "dns_failure": (5, 14),
    }[kind]
    case.t_imp = case.t_f + case.u(*gap)
    case.root_mode = {"cert_expiry": "tls", "oom": "flap", "slow_dependency": "slow"}.get(
        kind, "errors"
    )
    if kind == "pool_exhaustion":
        case.detail["dep"] = rng.choice(sql_deps or sync_deps)
        case.detail["max"] = rng.choice((20, 32, 50, 64))
    elif kind == "bad_config":
        if role == "scheduler":
            case.root_mode = "storm"
            case.detail["variant"] = "storm"
            case.t_imp = case.t_f + case.u(3, 8)
        elif role == "gateway":
            case.detail["variant"] = "routes"
        else:
            variants = ["timeout"] if sync_deps else []
            variants += ["strict"] if role == "api" else []
            variants += ["poolsize"] if sql_deps else []
            case.detail["variant"] = rng.choice(variants or ["strict"])
            case.detail["dep"] = rng.choice(sql_deps or sync_deps or [""])
    elif kind == "clock_skew":
        sign = 1 if rng.random() < 0.5 else -1
        case.detail["offset"] = sign * round(case.u(40, 300), 1)
        callees = [
            e.dst
            for e in topo.deps(root)
            if e.kind in ("http", "grpc") and topo.role(e.dst) == "api"
        ]
        case.detail["callees"] = callees
    elif kind == "poison_message":
        case.detail["queue"] = next(e.dst for e in topo.deps(root) if e.kind == "consume")
        case.detail["offset"] = rng.randint(10_000, 900_000)
    elif kind == "dns_failure":
        case.detail["ns"] = f"10.0.{rng.randint(0, 9)}.{rng.randint(2, 250)}"


class _Propagator:
    def __init__(self, case: Case) -> None:
        self.case = case
        self.heap: list[tuple[float, int, str, str, str]] = []
        self.seq = 0

    def push(self, service: str, t: float, cause: str, mode: str) -> None:
        if service == self.case.root or t > self.case.t_f + HORIZON:
            return
        self.seq += 1
        heapq.heappush(self.heap, (t, self.seq, service, cause, mode))

    def run(self) -> None:
        case = self.case
        self._seed()
        self.spread(case.root, case.t_imp, case.root_mode)
        while self.heap:
            t, _, service, cause, mode = heapq.heappop(self.heap)
            if service in case.onsets:
                continue
            case.onsets[service] = Onset(service, t, cause, mode)
            self.spread(service, t, mode)

    def _seed(self) -> None:
        """Kind-specific symptoms that do not follow the dependents rule."""
        case, topo, root = self.case, self.case.topo, self.case.root
        explicit = case.knobs.get("root_explicit")
        if case.kind == "pool_exhaustion" and explicit and topo.role(case.detail["dep"]) == "db":
            self.push(case.detail["dep"], case.t_imp + case.u(5, 30), root, "client_hold")
        if case.kind == "clock_skew":
            for callee in case.detail["callees"]:
                self.push(callee, case.t_f + case.u(1.0, 3.0), root, "reject")
        elif topo.role(root) == "gateway":
            for e in topo.deps(root):
                self.push(e.dst, case.t_imp + case.u(20, 60), root, "traffic_drop")

    def spread(self, service: str, t: float, mode: str) -> None:  # noqa: PLR0912, one rule per mode
        case, topo = self.case, self.case.topo
        role = topo.role(service)
        if mode in IMPAIRED and not (mode == "overload" and role == "worker"):
            for e in topo.dependents(service):
                self._hit_dependent(e, t, mode)
            if role == "worker":
                for e in topo.deps(service):
                    if e.kind == "consume":
                        self.push(e.dst, t + case.u(20, 80), service, "backlog")
        if mode == "backlog" and case.rng.random() < 0.8:
            for e in topo.dependents(service):
                if e.kind == "publish":
                    self.push(e.src, t + case.u(30, 90), service, "throttled")
        elif mode == "storm":
            for e in topo.deps(service):
                target_mode = "flood" if e.kind == "publish" else "overload"
                self.push(e.dst, t + case.u(8, 25), service, target_mode)
        elif mode == "flood":
            for e in topo.dependents(service):
                if e.kind == "consume":
                    self.push(e.src, t + case.u(10, 40), service, "overload")
        elif mode == "overload" and role == "worker":
            for e in topo.deps(service):
                if e.kind == "sql":
                    self.push(e.dst, t + case.u(15, 45), service, "load")

    def _hit_dependent(self, e: Edge, t: float, mode: str) -> None:
        case = self.case
        extra = case.u(2, 8) if mode in ("slow", "load", "overload", "throttled") else 0.0
        if e.kind == "consume":
            self.push(e.src, t + case.u(3, 15), e.dst, "idle")
        elif e.kind == "cache" or (e.src, e.dst) in case.fallback:
            slow = e.kind == "cache" and case.rng.random() < 0.4
            self.push(e.src, t + case.u(2, 12) + extra, e.dst, "slow" if slow else "degraded")
        elif mode == "throttled":
            self.push(e.src, t + case.u(4, 20), e.dst, "slow")
        else:
            self.push(e.src, t + case.u(2, 15) + extra, e.dst, "errors")


# --------------------------------------------------------------------------
# Log text
# --------------------------------------------------------------------------


def _rid(rng: random.Random) -> str:
    return f"{rng.getrandbits(32):08x}"


def _path(case: Case, service: str) -> str:
    topo = case.topo
    own = topo.paths.get(service)
    if own:
        return case.rng.choice(own)
    callees = [p for e in topo.deps(service) for p in topo.paths.get(e.dst, [])]
    return case.rng.choice(callees or [f"/v1/{case.rng.choice(topo.words)}"])


def _method(rng: random.Random) -> str:
    return rng.choice(("GET", "GET", "POST", "PUT"))


def _err_seen(case: Case, u: str, kind: str) -> str:  # noqa: PLR0911, one return per symptom
    """What a service that depends on ``u`` over ``kind`` sees when ``u`` misbehaves."""
    rng, mode, role = case.rng, case.mode_of(u), case.topo.role(u)
    if u == case.root and case.kind == "disk_full" and case.knobs.get("root_explicit"):
        return {
            "db": "ERROR: could not extend file: No space left on device",
            "queue": "broker error NOT_ENOUGH_SPACE: log directory full",
            "cache": "MISCONF background save failed, writes disabled",
        }.get(role, "503 Service Unavailable")
    if mode == "tls":
        return "x509: certificate has expired or is not yet valid"
    if mode == "flap":
        return rng.choice(("connection refused", "connection reset by peer"))
    if mode in ("slow", "load", "overload", "throttled"):
        if kind == "sql":
            return "canceling statement due to statement timeout"
        return f"context deadline exceeded after {rng.choice((1000, 2000, 3000, 5000))}ms"
    if kind == "sql":
        return "server closed the connection unexpectedly"
    if kind == "grpc":
        return rng.choice(("rpc error: code = Unavailable", "rpc error: code = Internal"))
    if kind == "publish":
        return "broker returned 503"
    return rng.choice(("503 Service Unavailable", "500 Internal Server Error", "502 Bad Gateway"))


def _served(case: Case, service: str, status: int, cause: str | None) -> Callable[[], str]:
    """A failed request this service answered (a gateway's lines carry the upstream)."""
    rng = case.rng
    gateway = case.topo.role(service) == "gateway" and cause is not None

    def make() -> str:
        ms = rng.randint(900, 5100) if status in (504, 503) else rng.randint(3, 900)
        path = _path(case, cause if gateway and cause else service)
        text = f"{_method(rng)} {path} {status} {ms}ms"
        if gateway and cause and case.knobs.get("addressing") != "path":
            return f"{text} upstream={_addr(case, cause)}"
        return text

    return make


def _addr(case: Case, name: str, kind: str = "") -> str:
    """How a victim's network errors refer to ``name``: its name, IP and port, or a route.

    Routes are used only for HTTP and gRPC calls to APIs, whose access logs show
    which routes they serve; everything else falls back to IP and port.
    """
    addressing = case.knobs.get("addressing", "name")
    s = case.topo.service(name)
    if addressing == "path" and kind in ("http", "grpc") and case.topo.paths.get(name):
        return case.rng.choice(case.topo.paths[name])
    if addressing == "name":
        return name
    return f"{s.ip}:{s.port}"


def _url(addr: str, path: str) -> str:
    if addr.startswith("/"):
        return addr
    return f"http://{addr}{path}" if ":" in addr else f"http://{addr}:8080{path}"


def _has_clients(case: Case, service: str) -> bool:
    topo = case.topo
    return topo.role(service) == "gateway" or any(
        e.kind in SYNC or e.kind == "publish" for e in topo.dependents(service)
    )


# Symptom handlers: for a victim's onset, the lines it keeps (the first one
# names the cause) and the templates it repeats until the window ends.
SymptomPlan = tuple[list[tuple[float, str, str]], list[Template]]


def _sym_errors(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    edge = case.topo.edge(o.service, u)
    kind = edge.kind if edge else "http"
    err = _err_seen(case, u, kind)
    a = _addr(case, u, kind)
    keep: list[tuple[float, str, str]] = []
    reps: list[Template] = []
    if kind == "sql":
        keep.append((o.t, "ERROR", f"query on {a} failed: {err}"))
        reps += [
            ("ERROR", lambda: f"query on {a} failed: {err}"),
            ("WARN", lambda: f"retrying transaction on {a} (attempt {rng.randint(2, 3)}/3)"),
        ]
    elif kind == "publish":
        topic = case.topo.topics.get(u, "events")
        keep.append((o.t, "ERROR", f"publish to {a} topic {topic} failed: {err}"))
        reps += [
            ("ERROR", lambda: f"publish to {a} topic {topic} failed: {err}"),
            ("WARN", lambda: f"buffering {rng.randint(50, 4000)} events for {a}"),
        ]
    else:
        path = _path(case, u)
        keep.append((o.t, "WARN", f"retrying {_method(rng)} {_url(a, path)} (attempt 1/3): {err}"))
        reps += [
            ("ERROR", lambda: f"call to {a} failed after 3 attempts: {err}"),
            (
                "ERROR",
                lambda: f"request {_rid(rng)} failed: upstream error from {a}; caused by: {err}",
            ),
            ("WARN", lambda: f"retrying call to {a} (attempt {rng.randint(2, 3)}/3): {err}"),
        ]
        opened = o.t + case.u(10, 40)
        if opened < case.t_end:
            pct = rng.randint(55, 100)
            keep.append(
                (opened, "ERROR", f"circuit breaker for {a} opened: {pct}% of last 20 calls failed")
            )
            reps.append(("WARN", lambda: f"circuit open for {a}, failing fast"))
    if case.mode_of(u) == "flap":
        reps += [
            ("WARN", lambda: f"health check for {a} failed 3 times, marking unhealthy"),
            ("INFO", lambda: f"health check for {a} passed, marking healthy"),
        ]
    if _has_clients(case, o.service):
        status = 504 if "deadline" in err or "timeout" in err else rng.choice((500, 502, 503))
        reps += [("ERROR", _served(case, o.service, status, u))] * 2
    return keep, reps


def _sym_slow(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    edge = case.topo.edge(o.service, u)
    a = _addr(case, u, edge.kind if edge else "")
    # A gateway has no routes of its own: its slow answers are on the upstream's routes.
    route_of = u if case.topo.role(o.service) == "gateway" else o.service
    first = f"call to {a} took {rng.randint(1200, 4800)}ms (budget 800ms)"
    reps: list[Template] = [
        ("WARN", lambda: f"call to {a} took {rng.randint(900, 5200)}ms (budget 800ms)")
    ]
    if _has_clients(case, o.service):
        reps.append(
            (
                "WARN",
                lambda: (
                    f"slow request {_method(rng)} {_path(case, route_of)} 200 {rng.randint(1500, 6000)}ms"
                ),
            )
        )
    return [(o.t, "WARN", first)], reps


def _sym_degraded(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    edge = case.topo.edge(o.service, u)
    if edge and edge.kind == "cache":
        msg = f"cache {_addr(case, u)} timeout after 250ms, falling back to origin"
    else:
        kind = edge.kind if edge else "http"
        msg = f"{_addr(case, u, kind)} unavailable ({_err_seen(case, u, kind)}), serving fallback"
    reps: list[Template] = [
        ("WARN", lambda: msg),
        ("INFO", lambda: f"fallback served for {_path(case, o.service)} in {rng.randint(5, 90)}ms"),
    ]
    return [(o.t, "WARN", msg)], reps


def _sym_idle(case: Case, o: Onset) -> SymptomPlan:
    u = o.cause or ""
    msg = f"fetch from {_addr(case, u)} failed: {_err_seen(case, u, 'consume')}; consumer idle"
    return [(o.t, "WARN", msg)], [
        ("WARN", lambda: msg),
        ("INFO", lambda: "no messages processed in the last 60s"),
    ]


def _sym_backlog(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    topic = case.topo.topics.get(o.service, "events")
    stuck = ""
    if case.kind == "poison_message" and u == case.root:
        stuck = f", partition {rng.randint(0, 11)} stuck at offset {case.detail['offset']}"
    lag = [rng.randint(800, 2000)]

    def make() -> str:
        lag[0] += rng.randint(300, 3000)
        return f"consumer group {u} lag {lag[0]} and rising on {topic}{stuck}"

    first = f"consumer group {u} lag {lag[0]} and rising on {topic} (acks 0/s){stuck}"
    return [(o.t, "WARN", first)], [("WARN", make)]


def _sym_throttled(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    a = _addr(case, u)

    def make() -> str:
        return f"publish to {a} throttled: broker backpressure, {rng.randint(500, 9000)} messages buffered"

    reps: list[Template] = [("WARN", make)]
    if _has_clients(case, o.service):
        reps.append(("ERROR", _served(case, o.service, 504, None)))
    return [(o.t, "WARN", make())], reps


def _sym_traffic_drop(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    base = rng.randint(20, 90)

    def make() -> str:
        return f"request rate from {u} dropped to {rng.uniform(0, 2):.1f}/s (baseline {base}/s)"

    return [(o.t, "WARN", make())], [
        ("WARN", make),
        ("INFO", lambda: "no requests received in the last 30s"),
    ]


def _sym_reject(case: Case, o: Onset) -> SymptomPlan:
    u, off = o.cause or "", int(case.detail["offset"])
    if off > 0:
        msg = f"rejected request from {u}: token issued {off}s in the future (allowed skew 30s)"
    else:
        msg = f"rejected request from {u}: token already expired, issued {-off}s in the past"
    return [(o.t, "WARN", msg)], [("WARN", lambda: msg)]


def _sym_client_hold(case: Case, o: Onset) -> SymptomPlan:
    rng, u, top = case.rng, o.cause or "", int(case.detail["max"])

    def make() -> str:
        idle = rng.randint(top - 6, top - 1)
        return f"client {u} holds {top} connections, {idle} idle in transaction for over 60s"

    return [(o.t, "WARN", make())], [("WARN", make)]


def _sym_overload(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    if case.topo.role(o.service) == "worker":

        def make() -> str:
            return (
                f"all {rng.choice((8, 16, 32))} workers busy, {rng.randint(900, 20000)} jobs "
                f"waiting, ingress from {u} at {rng.uniform(8, 60):.0f}x baseline"
            )

        return [(o.t, "WARN", make())], [("WARN", make)]

    def shed() -> str:
        return f"shedding load: 429 for {rng.randint(20, 70)}% of requests, calls from {u} at {rng.uniform(8, 60):.0f}x baseline"

    reps: list[Template] = [("WARN", shed), ("WARN", _served(case, o.service, 429, None))]
    return [(o.t, "WARN", shed())], reps


def _sym_load(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    table = rng.choice(case.topo.tables.get(o.service, ["jobs"]))

    def make() -> str:
        return f"slow query {rng.randint(1500, 6500)}ms on {table}; query rate {rng.uniform(3, 9):.1f}x baseline, top client {u}"

    return [(o.t, "WARN", make())], [
        ("WARN", make),
        ("WARN", lambda: f"connections {rng.randint(170, 200)}/200, top client {u}"),
    ]


def _sym_flood(case: Case, o: Onset) -> SymptomPlan:
    rng, u = case.rng, o.cause or ""
    topic = case.topo.topics.get(o.service, "events")

    def make() -> str:
        return f"ingress from producer {u} at {rng.uniform(40, 300):.0f}x baseline on {topic}, lag rising"

    return [(o.t, "WARN", make())], [("WARN", make)]


SYMPTOMS: dict[str, Callable[[Case, Onset], SymptomPlan]] = {
    "errors": _sym_errors,
    "slow": _sym_slow,
    "degraded": _sym_degraded,
    "idle": _sym_idle,
    "backlog": _sym_backlog,
    "throttled": _sym_throttled,
    "traffic_drop": _sym_traffic_drop,
    "reject": _sym_reject,
    "client_hold": _sym_client_hold,
    "overload": _sym_overload,
    "load": _sym_load,
    "flood": _sym_flood,
}


# Root sequences: the lines the root keeps (the first is its first anomaly),
# the templates it repeats, and any extra timed lines.
RootPlan = tuple[list[tuple[float, str, str]], list[Template], list[tuple[float, str, str]]]


def _root_served(case: Case, status: int) -> list[Template]:
    """HTTP answers the root gives its callers (only services that speak HTTP)."""
    if case.topo.role(case.root) not in ("api", "gateway") or not _has_clients(case, case.root):
        return []
    return [("ERROR", _served(case, case.root, status, None))]


def _root_pool(case: Case) -> RootPlan:
    rng, x, top = case.rng, case.detail["dep"], case.detail["max"]
    mid = (case.t_f + case.t_imp) / 2
    keep = [
        (case.t_f, "WARN", f"connection pool {x}: {int(top * 0.9)}/{top} in use, 0 idle"),
        (mid, "WARN", f"connection pool {x}: {top}/{top} in use, {rng.randint(2, 9)} waiting"),
        (case.t_err, "ERROR", f"timed out after 3000ms waiting for a connection from pool {x}"),
        (
            case.t_imp + case.u(20, 60),
            "WARN",
            f"leak detector: {top - rng.randint(1, 5)} connections to {x} checked out for over 60s with no activity",
        ),
    ]
    reps: list[Template] = [
        ("ERROR", lambda: f"timed out after 3000ms waiting for a connection from pool {x}"),
        ("WARN", lambda: f"connection pool {x}: {top}/{top} in use, {rng.randint(10, 90)} waiting"),
        *_root_served(case, 503),
    ]
    return keep, reps, []


def _root_bad_config(case: Case) -> RootPlan:
    rng, variant, dep = case.rng, case.detail["variant"], case.detail.get("dep", "")
    ver = f"{rng.randint(1, 9)}.{rng.randint(0, 30)}.{rng.randint(0, 9)}"
    first = (case.t_f, "INFO", f"deploying version {ver} (build {_rid(rng)[:7]})")
    reload_t = case.t_f + case.u(1, 3)
    job = case.rng.choice(case.topo.jobs)
    if variant == "storm":
        change = f"schedule reloaded: job {job} every 1s (was every 1h)"
        keep = [first, (reload_t, "INFO", change)]
        reps: list[Template] = [
            ("INFO", lambda: f"job {job} fired, enqueued batch of {rng.randint(200, 900)}"),
            (
                "WARN",
                lambda: (
                    f"job {job} overlaps previous run ({rng.randint(3, 60)} running), starting anyway"
                ),
            ),
        ]
        return keep, reps, []
    field_name = rng.choice(case.topo.words)
    table: dict[str, tuple[str, int, Callable[[], str]]] = {
        "routes": (
            f"route table reloaded: {rng.randint(1, 3)} routes (was {rng.randint(12, 30)})",
            502,
            lambda: f"no route for {_method(rng)} {_path(case, case.root)}, returning 502",
        ),
        "timeout": (
            "config reloaded: upstream_timeout_ms=5 (was 5000)",
            500,
            lambda: f"call to {dep} cancelled: deadline of 5ms exceeded",
        ),
        "poolsize": (
            "config reloaded: db_pool_max=1 (was 40)",
            503,
            lambda: f"timed out waiting for a connection to {dep} (pool size 1)",
        ),
        "strict": (
            "config reloaded: strict_schema=true (was false)",
            500,
            lambda: f"rejecting payload: unknown field '{field_name}' (strict_schema=true)",
        ),
    }
    change, status, failure = table[variant]
    keep = [first, (reload_t, "INFO", change), (case.t_err, "ERROR", failure())]
    return keep, [("ERROR", failure), *_root_served(case, status)], []


def _root_disk(case: Case) -> RootPlan:
    rng, role = case.rng, case.topo.role(case.root)
    mount = {"db": "/var/lib/data", "queue": "/var/lib/broker", "cache": "/var/lib/cache"}.get(
        role, "/var/spool/work"
    )
    topic = next(iter(case.topo.topics.values()), "events")

    def nospace() -> str:
        return {
            "db": f"could not extend file base/{rng.randint(10000, 99999)}/{rng.randint(1000, 9999)}: No space left on device",
            "queue": f"failed to append to {topic} segment: No space left on device",
            "cache": "background save failed: No space left on device, rejecting writes",
        }.get(role, f"failed to write spool file for job {_rid(rng)}: No space left on device")

    keep = [
        (case.t_f, "WARN", f"disk usage on {mount} at {rng.randint(91, 93)}%"),
        (
            case.t_f + 0.6 * (case.t_imp - case.t_f),
            "WARN",
            f"disk usage on {mount} at {rng.randint(97, 99)}%",
        ),
        (case.t_err, "ERROR", nospace()),
    ]
    reps: list[Template] = [
        ("ERROR", nospace),
        ("WARN", lambda: f"disk usage on {mount} at 100%"),
        *_root_served(case, 500),
    ]
    return keep, reps, []


def _root_cert(case: Case) -> RootPlan:
    rng, root = case.rng, case.root
    secs = int(case.t_imp - case.t_f)
    keep = [
        (
            case.t_f,
            "WARN",
            f"tls: serving certificate CN={root}.internal expires in {secs}s, renewal has not run",
        ),
        (case.t_imp, "WARN", f"tls: serving certificate CN={root}.internal has expired"),
    ]

    def handshake() -> str:
        return (
            f"tls: handshake error from 10.{rng.randint(0, 9)}.{rng.randint(0, 31)}."
            f"{rng.randint(2, 250)}:{rng.randint(30000, 60000)}: remote error: tls: bad certificate"
        )

    return keep, [(rng.choice(("WARN", "ERROR")), handshake)], []


def _root_oom(case: Case) -> RootPlan:
    rng, root = case.rng, case.root
    mem = rng.choice((512, 1024, 2048, 4096))
    ver = f"{rng.randint(1, 9)}.{rng.randint(0, 30)}.{rng.randint(0, 9)}"
    keep = [(case.t_f, "WARN", f"heap usage {rng.randint(76, 80)}% of {mem}MiB after full GC")]
    step = (case.t_imp - case.t_f) / 3
    keep += [
        (
            case.t_f + step,
            "WARN",
            f"heap usage {rng.randint(84, 89)}% of {mem}MiB, GC pause {rng.uniform(0.6, 1.4):.1f}s",
        ),
        (
            case.t_f + 2 * step,
            "WARN",
            f"heap usage {rng.randint(93, 97)}% of {mem}MiB, GC overhead limit approaching",
        ),
        (case.t_imp, "ERROR", "process killed by the OOM killer (exit code 137), restart 1"),
    ]
    extra: list[tuple[float, str, str]] = []
    t, restart = case.t_imp, 1
    while t < case.t_end:
        extra.append((t + case.u(3, 8), "INFO", f"starting {root} version {ver}"))
        extra.append((t + case.u(10, 16), "INFO", "ready, listening on :8080"))
        t += case.u(45, 110)
        restart += 1
        if t < case.t_end:
            extra.append(
                (t, "ERROR", f"process killed by the OOM killer (exit code 137), restart {restart}")
            )
    reps: list[Template] = [
        (
            "WARN",
            lambda: (
                f"heap usage {rng.randint(85, 98)}% of {mem}MiB, GC pause {rng.uniform(0.5, 2.5):.1f}s"
            ),
        ),
    ]
    return keep, reps, extra


def _root_clock(case: Case) -> RootPlan:
    rng, callees = case.rng, case.detail["callees"]
    off = case.detail["offset"]
    job = rng.choice(case.topo.jobs)
    scheduler = case.topo.role(case.root) == "scheduler"
    why = "token not yet valid" if off > 0 else "token expired"

    def failure() -> str:
        c = rng.choice(callees)
        if scheduler:
            return f"job {job} failed: {c} returned 401 Unauthorized ({why})"
        return f"call to {c} failed: 401 Unauthorized ({why})"

    keep = [
        (
            case.t_f,
            "WARN",
            f"ntp: clock stepped, local clock is now {abs(off):.1f}s "
            f"{'ahead of' if off > 0 else 'behind'} reference time",
        ),
        (case.t_f + case.u(1, 3), "ERROR", failure()),
    ]
    return keep, [("ERROR", failure), *_root_served(case, 502)], []


def _root_slow(case: Case) -> RootPlan:
    rng, topo, role = case.rng, case.topo, case.topo.role(case.root)
    table = rng.choice(topo.tables.get(case.root, ["jobs"]))
    topic = topo.topics.get(case.root, "events")
    word = rng.choice(topo.words)
    by_role: dict[str, tuple[str, Callable[[], str]]] = {
        "db": (
            f"autovacuum: VACUUM FULL started on table {table}, taking ACCESS EXCLUSIVE lock",
            lambda: f"slow query {rng.randint(2500, 9000)}ms on {table}: waiting for lock",
        ),
        "cache": (
            f"background save started, fork took {rng.randint(1800, 4200)}ms",
            lambda: (
                f"command latency p99 {rng.randint(900, 4000)}ms, {rng.randint(20, 300)} clients blocked"
            ),
        ),
        "api": (
            f"lock contention: {rng.randint(30, 60)} threads blocked on {word} registry mutex",
            lambda: (
                f"request queue {rng.randint(100, 900)} deep, p99 latency {rng.randint(2000, 9000)}ms"
            ),
        ),
        "queue": (
            f"partition reassignment started for {rng.randint(24, 96)} partitions on {topic}",
            lambda: f"produce latency p99 {rng.randint(1500, 6000)}ms during reassignment",
        ),
    }
    trigger, slow = by_role[role]
    level = "WARN" if role == "api" else "INFO"
    keep = [(case.t_f, level, trigger), (case.t_imp, "WARN", slow())]
    served = [("WARN", _served(case, case.root, 504, None))] if role == "api" else []
    return keep, [("WARN", slow), *served], []


def _root_poison(case: Case) -> RootPlan:
    rng, topo = case.rng, case.topo
    off, topic = case.detail["offset"], topo.topics.get(case.detail["queue"], "events")
    key = f"{rng.choice(topo.words)}-{rng.randint(1000, 99999)}"
    fld = rng.choice(topo.words)
    panic = f"handler panic on message offset={off} key={key} source=partner-feed: unexpected type for field '{fld}'"
    attempt = [1]

    def redeliver() -> str:
        attempt[0] += 1
        return f"redelivering message offset={off} (attempt {attempt[0]}), no dead letter queue configured"

    failed = f"handler failed on message offset={off} key={key} source=partner-feed (attempt 1): unexpected type for field '{fld}'"
    keep = [(case.t_f, "WARN", failed), (case.t_err + case.u(2, 6), "ERROR", panic)]
    reps: list[Template] = [
        ("ERROR", lambda: panic),
        ("WARN", redeliver),
        ("INFO", lambda: f"consumer restarted, resuming {topic} at offset {off}"),
    ]
    return keep, reps, []


def _root_dns(case: Case) -> RootPlan:
    rng, ns = case.rng, case.detail["ns"]
    deps = [e.dst for e in case.topo.deps(case.root) if e.kind != "consume"]

    def lookup() -> str:
        return f"dial tcp: lookup {rng.choice(deps)}.svc.internal on {ns}:53: no such host"

    keep = [
        (case.t_f, "INFO", f"resolv.conf reloaded: nameserver {ns}"),
        (case.t_f + case.u(1, 4), "WARN", f"dns: resolver {ns}:53 timed out after 5000ms"),
        (case.t_err, "ERROR", lookup()),
    ]
    return keep, [("ERROR", lookup), *_root_served(case, 502)], []


def _generic_failure(case: Case) -> Callable[[], str]:
    rng, topo, root = case.rng, case.topo, case.root
    role = topo.role(root)
    topic = topo.topics.get(root, "events")
    table = rng.choice(topo.tables.get(root, ["jobs"]))
    by_role: dict[str, Callable[[], str]] = {
        "api": lambda: (
            f"unhandled error in request handler for {_path(case, root)}: internal error"
        ),
        "gateway": lambda: f"unhandled error proxying {_path(case, root)}: internal error",
        "worker": lambda: f"job {_rid(rng)} failed: internal error",
        "db": lambda: f"could not complete statement on {table}: internal error",
        "queue": lambda: f"produce request on {topic} failed: internal broker error",
        "cache": lambda: "command failed: internal error",
    }
    slow: dict[str, Callable[[], str]] = {
        "db": lambda: f"slow query {rng.randint(1500, 9000)}ms on {table}",
        "cache": lambda: f"command latency p99 {rng.randint(900, 4000)}ms",
        "queue": lambda: f"produce latency p99 {rng.randint(1500, 6000)}ms",
        "api": lambda: f"p99 latency {rng.randint(1500, 9000)}ms over the last 60s (target 250ms)",
    }
    if case.root_mode == "slow":
        return slow[role]
    if case.root_mode == "tls":
        return lambda: (
            f"tls: handshake error from 10.{rng.randint(0, 9)}.{rng.randint(0, 31)}."
            f"{rng.randint(2, 250)}:{rng.randint(30000, 60000)}: remote error: tls: bad certificate"
        )
    return by_role[role]


def _root_generic(case: Case) -> RootPlan:
    """The root only shows plain errors or latency; its mechanism is never named.

    Its first anomaly lands just after it becomes impaired, still before any
    victim's first symptom (the fastest propagation takes two seconds).
    """
    rng, root = case.rng, case.root
    if case.root_mode == "flap":
        case.t_f = case.t_imp
        extra: list[tuple[float, str, str]] = []
        t, restart = case.t_imp, 1
        while t < case.t_end:
            extra.append((t + case.u(3, 8), "INFO", f"starting {root}"))
            extra.append((t + case.u(10, 16), "INFO", "ready, listening on :8080"))
            t += case.u(45, 110)
            restart += 1
            if t < case.t_end:
                extra.append(
                    (t, "ERROR", f"process exited unexpectedly, restarting (restart {restart})")
                )
        keep = [(case.t_f, "ERROR", "process exited unexpectedly, restarting (restart 1)")]
        return keep, [], extra
    case.t_f = case.t_imp + case.u(0.1, 1.0)
    case.t_err = max(case.t_err, case.t_f + case.u(0.5, 3))
    failure = _generic_failure(case)
    level = "WARN" if case.root_mode == "slow" else rng.choice(("WARN", "ERROR"))
    keep = [
        (case.t_f, level, failure()),
        (case.t_err, "WARN" if case.root_mode == "slow" else "ERROR", failure()),
    ]
    status = 504 if case.root_mode == "slow" else 500
    return (
        keep,
        [("WARN" if case.root_mode == "slow" else "ERROR", failure), *_root_served(case, status)],
        [],
    )


ROOTS: dict[str, Callable[[Case], RootPlan]] = {
    "pool_exhaustion": _root_pool,
    "bad_config": _root_bad_config,
    "disk_full": _root_disk,
    "cert_expiry": _root_cert,
    "oom": _root_oom,
    "clock_skew": _root_clock,
    "slow_dependency": _root_slow,
    "poison_message": _root_poison,
    "dns_failure": _root_dns,
}


def _generic_symptom(case: Case, o: Onset) -> tuple[str, str]:
    rng = case.rng
    if case.topo.role(o.service) == "api" and rng.random() < 0.5:
        return "ERROR", _served(case, o.service, 500 if o.mode == "errors" else 504, None)()
    if o.mode == "slow":
        return "WARN", f"p99 latency {rng.randint(1500, 6000)}ms over the last 60s (target 250ms)"
    return rng.choice(("ERROR", "WARN")), f"request {_rid(rng)} failed: internal error"


def _route_samples(case: Case) -> None:
    """One ordinary access line per route before the incident, so routes map to services."""
    rng = case.rng
    stepped_back = case.kind == "clock_skew" and case.detail["offset"] < 0
    for api, paths in case.topo.paths.items():
        for path in paths:
            if api == case.root and stepped_back:
                # Before the step these would display among the replayed lines.
                t = rng.uniform(case.t_f + 1, case.t_end)
            else:
                t = rng.uniform(1, max(2.0, case.t_f - 5))
            msg = f"{_method(rng)} {path} 200 {rng.randint(3, 180)}ms"
            case.emit(t, api, "INFO", msg, "noise", True)


def _repeat(
    case: Case,
    service: str,
    start: float,
    period: float,
    reps: Sequence[Template],
    cat: str,
    until: float | None = None,
) -> None:
    stop = case.t_end if until is None else min(until, case.t_end)
    t = start + case.u(0.3, 1.0) * period
    while t < stop and reps:
        level, make = case.rng.choice(reps)
        case.emit(t, service, level, make(), cat, False)
        t += period * case.u(0.4, 1.6)


# --------------------------------------------------------------------------
# Noise and red herrings
# --------------------------------------------------------------------------


def _noise_line(case: Case, service: str, t: float) -> tuple[str, str]:  # noqa: PLR0911, one return per role
    rng, topo = case.rng, case.topo
    role = topo.role(service)
    pick = rng.random()
    deps = [e.dst for e in topo.deps(service) if e.kind in SYNC]
    if pick > 0.94 and deps:
        return "WARN", f"call to {rng.choice(deps)} took {rng.randint(810, 1400)}ms (budget 800ms)"
    if pick < 0.08:
        return rng.choice(
            (
                ("INFO", "health check ok"),
                ("DEBUG", "config watcher: no changes"),
                ("INFO", f"gc: pause {rng.randint(2, 30)}ms, heap {rng.randint(30, 60)}%"),
                ("DEBUG", f"trace exported: root span {_rid(rng)} with {rng.randint(3, 40)} spans"),
            )
        )
    if role == "gateway":
        callees = [e.dst for e in topo.deps(service)]
        up = rng.choice(callees)
        status = rng.choice((200, 200, 200, 201, 204, 304))
        return (
            "INFO",
            f"{_method(rng)} {_path(case, up)} {status} {rng.randint(3, 250)}ms upstream={up}",
        )
    if role == "api":
        if pick < 0.2 and any(e.kind == "publish" for e in topo.deps(service)):
            q = next(e.dst for e in topo.deps(service) if e.kind == "publish")
            return "INFO", f"published {rng.randint(1, 40)} events to {topo.topics[q]}"
        return "INFO", f"{_method(rng)} {_path(case, service)} 200 {rng.randint(2, 180)}ms"
    if role == "worker":
        q = next((e.dst for e in topo.deps(service) if e.kind == "consume"), "")
        topic = topo.topics.get(q, "events")
        if pick < 0.4:
            return "INFO", f"committed offset {rng.randint(10_000, 900_000)} on {topic}"
        return "INFO", f"processed job {_rid(rng)} from {topic} in {rng.randint(20, 900)}ms"
    if role == "queue":
        topic = topo.topics[service]
        consumer = next((e.src for e in topo.dependents(service) if e.kind == "consume"), "")
        onset = case.onsets.get(service)
        if pick < 0.4 and consumer and (onset is None or t < onset.t):
            return "INFO", f"consumer group {consumer} lag {rng.randint(0, 40)} on {topic}"
        return (
            "INFO",
            f"partition {rng.randint(0, 11)} of {topic} leader=broker-{rng.randint(1, 3)} isr=3",
        )
    if role == "cache":
        return (
            "INFO",
            f"keyspace hits={rng.randint(1000, 90000)} misses={rng.randint(10, 900)} evicted_keys={rng.randint(0, 50)}",
        )
    if role == "db":
        if pick < 0.4:
            return (
                "INFO",
                f"checkpoint complete: wrote {rng.randint(100, 5000)} buffers ({rng.uniform(0.1, 4):.1f}%)",
            )
        return "INFO", f"connections: {rng.randint(10, 60)} active, {rng.randint(5, 40)} idle"
    job = rng.choice(topo.jobs)
    return "INFO", f"job {job} completed in {rng.uniform(0.2, 30):.1f}s"


_ROLE_WEIGHT = {
    "gateway": 3.0,
    "api": 2.0,
    "worker": 1.5,
    "db": 1.5,
    "queue": 1.0,
    "cache": 1.0,
    "scheduler": 0.6,
}


@dataclass(frozen=True)
class _Span:
    lo: float
    hi: float

    def __contains__(self, t: object) -> bool:
        return isinstance(t, float) and self.lo <= t < self.hi


def _noise(case: Case, count: int) -> list[Line]:
    rng, names = case.rng, case.topo.names
    weights = [_ROLE_WEIGHT[case.topo.role(n)] * rng.uniform(0.5, 1.5) for n in names]
    # The root keeps its benign lines at INFO and below. After a backward clock
    # step its later lines would display among its earlier ones, so it stays
    # quiet in the stretch the step replays.
    off = case.detail.get("offset", 0.0) if case.kind == "clock_skew" else 0.0
    blackout = _Span(case.t_f + min(0.0, off), case.t_f)
    out = []
    for _ in range(count):
        service = rng.choices(names, weights)[0]
        t = rng.uniform(0, case.t_end)
        onset = case.onsets.get(service)
        down = t > case.t_imp if service == case.root else bool(onset and t > onset.t)
        if down and rng.random() < 0.5:
            continue
        level, msg = _noise_line(case, service, t)
        if service == case.root and (level == "WARN" or t in blackout):
            continue
        out.append(Line(t, service, level, msg, "noise"))
    return out


def _decoy(case: Case, kind: str, service: str) -> tuple[str, str]:
    rng, topo = case.rng, case.topo
    other = rng.choice([e.dst for e in topo.deps(service)] or [service])
    return {
        "pool_exhaustion": ("WARN", f"connection pool {other}: {rng.randint(60, 72)}% in use"),
        "bad_config": (
            "INFO",
            f"config reloaded: log_level=info (was {rng.choice(('debug', 'warn'))})",
        ),
        "disk_full": ("WARN", f"disk usage on /var/log at {rng.randint(80, 86)}%"),
        "cert_expiry": (
            "WARN",
            f"tls: serving certificate CN={service}.internal expires in {rng.randint(9, 30)} days",
        ),
        "oom": ("WARN", f"GC pause {rng.randint(300, 650)}ms, heap {rng.randint(64, 74)}%"),
        "clock_skew": ("INFO", f"ntp: local clock offset {rng.randint(2, 40)}ms, within tolerance"),
        "slow_dependency": ("WARN", f"request p99 {rng.randint(700, 950)}ms above target 600ms"),
        "poison_message": (
            "WARN",
            f"skipped malformed record {_rid(rng)} (sent to dead letter queue)",
        ),
        "dns_failure": (
            "WARN",
            f"dns: slow lookup for {other}.svc.internal ({rng.randint(150, 400)}ms)",
        ),
    }[kind]


def _herring_decoys(case: Case, others: list[str]) -> int:
    """Cause-like warnings on healthy services, often of the same kind as the fault."""
    rng = case.rng
    count = rng.randint(1, 3)
    for i in range(count):
        kind = case.kind if i == 0 and rng.random() < 0.6 else rng.choice(KINDS)
        service = rng.choice(others)
        level, msg = _decoy(case, kind, service)
        case.emit(rng.uniform(0, case.t_end), service, level, msg, "herring", True)
    return count


def _contained(case: Case, kind: str, service: str) -> list[tuple[float, str, str]]:
    """A real but self-limiting fault: it recovers and nothing depends on it failing."""
    rng, topo = case.rng, case.topo
    ver = f"{rng.randint(1, 9)}.{rng.randint(0, 30)}.{rng.randint(0, 9)}"
    table = rng.choice([t for ts in topo.tables.values() for t in ts] or ["jobs"])
    off = rng.randint(10_000, 900_000)
    key = f"{rng.choice(topo.words)}-{rng.randint(1000, 99999)}"
    ns = f"10.0.{rng.randint(0, 9)}.{rng.randint(2, 250)}"
    events: dict[str, list[tuple[float, str, str]]] = {
        "oom": [
            (0, "ERROR", "process killed by the OOM killer (exit code 137), restart 1"),
            (rng.uniform(3, 8), "INFO", f"starting {service} version {ver}"),
            (rng.uniform(10, 16), "INFO", "ready, listening on :8080"),
        ],
        "disk_full": [
            (0, "WARN", f"disk usage on /var/log at {rng.randint(95, 97)}%"),
            (rng.uniform(15, 40), "ERROR", "failed to write access log: No space left on device"),
            (
                rng.uniform(45, 80),
                "INFO",
                f"log rotation freed {rng.uniform(4, 20):.1f}GB, disk usage on /var/log at {rng.randint(30, 50)}%",
            ),
        ],
        "cert_expiry": [
            (
                0,
                "ERROR",
                f"tls: certificate CN=admin.{service}.internal for admin listener :9443 has expired",
            ),
            (rng.uniform(2, 6), "WARN", "admin listener :9443 stopped until certificate renewal"),
        ],
        "bad_config": [
            (0, "INFO", f"deploying version {ver} (build {_rid(rng)[:7]})"),
            (rng.uniform(1, 3), "INFO", "config reloaded: export_batch_size=0 (was 500)"),
            (
                rng.uniform(5, 10),
                "ERROR",
                "metrics export failed: export_batch_size must be positive",
            ),
            (
                rng.uniform(40, 80),
                "INFO",
                "rolled back to previous version, metrics export resumed",
            ),
        ],
        "slow_dependency": [
            (0, "INFO", f"autovacuum: VACUUM started on table {table}"),
            (rng.uniform(5, 15), "WARN", f"slow query {rng.randint(1500, 3000)}ms on {table}"),
            (rng.uniform(40, 90), "INFO", f"autovacuum finished on table {table}"),
        ],
        "poison_message": [
            (
                0,
                "ERROR",
                f"handler failed on message offset={off} key={key}: unexpected type for field '{rng.choice(topo.words)}'",
            ),
            (
                rng.uniform(1, 4),
                "WARN",
                f"message offset={off} sent to dead letter queue after 3 attempts",
            ),
        ],
        "dns_failure": [
            (0, "WARN", f"dns: resolver {ns}:53 timed out after 5000ms"),
            (rng.uniform(1, 3), "INFO", "dns: fell back to secondary resolver, lookups healthy"),
        ],
        "clock_skew": [
            (0, "WARN", "ntp: lost sync with all sources, holding last good frequency"),
            (rng.uniform(30, 90), "INFO", "ntp: resynchronized with 3 sources"),
        ],
        "pool_exhaustion": [
            (
                0,
                "WARN",
                f"worker pool saturated: {rng.choice((16, 32))} of {rng.choice((16, 32))} busy on batch job",
            ),
            (rng.uniform(20, 60), "INFO", "batch job finished, worker pool idle"),
        ],
    }
    return events[kind]


def _herring_contained(case: Case, others: list[str]) -> bool:
    """A dramatic but contained fault on a service the incident never reaches."""
    rng = case.rng
    if rng.random() >= 0.7:
        return False
    t = max(5.0, case.t_f + rng.uniform(-150, 120))
    spots = [n for n in others if n not in case.onsets or case.onsets[n].t > t + 110]
    if not spots:
        return False
    service = rng.choice(spots)
    kind = case.kind if rng.random() < 0.5 else rng.choice(KINDS)
    for dt, level, msg in _contained(case, kind, service):
        case.emit(t + dt, service, level, msg, "herring", True)
    return True


def _secondary(case: Case) -> bool:
    """A victim that fails dramatically itself once the incident has reached it."""
    rng = case.rng
    pool = [o for o in case.onsets.values() if o.mode == "errors" and o.t + 150 < case.t_end]
    if not pool or rng.random() >= 0.5:
        return False
    o = rng.choice(pool)
    t = o.t + rng.uniform(40, 110)
    if rng.random() < 0.5:
        n = rng.randint(8_000, 60_000)
        lines = [
            (
                t,
                "ERROR",
                f"process killed by the OOM killer (exit code 137): retry buffer held {n} requests",
            ),
            (t + rng.uniform(3, 8), "INFO", f"starting {o.service}"),
        ]
    else:
        k = rng.choice((32, 64, 128))
        lines = [
            (
                t,
                "ERROR",
                f"worker thread pool exhausted: {k}/{k} threads blocked, rejecting new requests",
            )
        ]
    for when, level, msg in lines:
        case.emit(when, o.service, level, msg, "symptom", True)
    return True


def _herring_deploy(case: Case, others: list[str], victims: list[str]) -> bool:
    """A routine deploy elsewhere, close to the incident."""
    rng = case.rng
    if rng.random() >= (0.85 if case.kind == "bad_config" else 0.55):
        return False
    service = rng.choice(victims if victims and rng.random() < 0.5 else others)
    t = max(5.0, case.t_f + rng.uniform(-240, 60))
    ver = f"{rng.randint(1, 9)}.{rng.randint(0, 30)}.{rng.randint(0, 9)}"
    key, old, new = rng.choice(
        (
            ("log_level", "debug", "info"),
            ("cache_ttl_s", "120", "300"),
            ("feature.new_dashboard", "off", "on"),
        )
    )
    steps = (
        (0.0, f"deploying version {ver} (build {_rid(rng)[:7]})"),
        (rng.uniform(1, 3), f"config reloaded: {key}={new} (was {old})"),
        (rng.uniform(20, 60), "rollout complete, 3/3 instances ready"),
    )
    for dt, msg in steps:
        case.emit(t + dt, service, "INFO", msg, "herring", True)
    return True


def _herring_storm(case: Case, others: list[str], n_target: int) -> bool:
    """A noisy service with an unrelated WARN storm, often complaining about a client."""
    rng, topo = case.rng, case.topo
    if rng.random() >= 0.75:
        return False
    service = rng.choice(others)
    client = rng.choice([n for n in topo.names if n != service])
    endpoint = _path(case, service)
    msg = rng.choice(
        (
            f"client {client} is calling deprecated endpoint {endpoint}; please upgrade",
            f"client {client} is calling deprecated endpoint {endpoint}; please upgrade",
            f"client {client} exceeded soft rate limit, allowing burst",
            "deprecated config key 'metrics.legacy_port' is set; ignoring",
            f"rate limit at {rng.randint(88, 97)}% for client key k{rng.randint(1000, 9999)}",
            f"log shipper slow, dropped {rng.randint(10, 400)} lines",
            "container running as root; set runAsUser to drop privileges",
        )
    )
    for _ in range(rng.randint(8, max(8, min(30, n_target // 8)))):
        case.emit(rng.uniform(0, case.t_end), service, "WARN", msg, "herring", True)
    return True


def _herring_slow(case: Case, others: list[str], n_target: int) -> bool:
    """A healthy but chronically slow dependency that one caller keeps warning about."""
    rng, topo = case.rng, case.topo
    if rng.random() >= 0.55:
        return False
    target = rng.choice(others)
    callers = [e.src for e in topo.dependents(target) if e.kind in SYNC and e.src != case.root]
    caller = rng.choice(callers or [n for n in others if n != target])
    budget = rng.choice((300, 500, 800))
    for _ in range(rng.randint(8, max(8, min(25, n_target // 10)))):
        text = f"call to {target} took {rng.randint(budget + 50, budget * 2)}ms (budget {budget}ms)"
        case.emit(rng.uniform(0, case.t_end), caller, "WARN", text, "herring", True)
    return True


def _herring_chronic(case: Case, others: list[str]) -> bool:
    """A service that fails some background task all day, before and after the incident."""
    rng, topo = case.rng, case.topo
    if rng.random() >= 0.5:
        return False
    service = rng.choice(others)
    word = rng.choice(topo.words)
    msg = rng.choice(
        (
            f"scheduled export {rng.choice(topo.jobs)} failed: access denied writing to bucket {word}-exports",
            f"failed to renew lease {word}: conflict, another holder is active",
            f"webhook delivery to partner endpoint failed: 410 Gone ({word})",
        )
    )
    t = rng.uniform(5, 60)
    while t < case.t_end:
        case.emit(t, service, "ERROR", msg, "herring", True)
        t += rng.uniform(40, 110)
    return True


def _herring_pre_error(case: Case, others: list[str]) -> bool:
    """A transient error on some service well before the incident."""
    rng = case.rng
    if rng.random() >= 0.6 or case.t_f <= 40:
        return False
    service = rng.choice(others)
    t = rng.uniform(5, case.t_f - 20)
    text = "failed to flush metrics to collector: connection reset by peer"
    case.emit(t, service, "ERROR", text, "herring", True)
    case.emit(t + rng.uniform(5, 15), service, "INFO", "metrics flush recovered", "herring", True)
    return True


def _herring_skew(case: Case, others: list[str], victims: list[str]) -> str:
    """One host whose clock is off (never when the fault itself is clock skew)."""
    rng = case.rng
    if case.kind == "clock_skew" or rng.random() >= 0.6:
        return "none"
    pick = rng.random()
    if pick < 0.3:
        service = case.root
        off = round(rng.uniform(30, 180), 1) * (1 if rng.random() < 0.75 else -1)
    else:
        service = rng.choice(victims if victims and pick < 0.8 else others)
        off = round(rng.uniform(30, 180), 1) * (-1 if rng.random() < 0.75 else 1)
    case.skews[service] = [(-math.inf, off)]
    where = "behind" if off < 0 else "ahead of"
    text = (
        f"ntp: local clock is {abs(off):.1f}s {where} reference time; "
        "step refused (makestep limit), offset persists"
    )
    case.emit(rng.uniform(0, case.t_end * 0.6), service, "WARN", text, "ntp", True)
    if service == case.root:
        return "root"
    return "victim" if service in victims else "bystander"


def _herrings(case: Case, n_target: int) -> None:
    others = [n for n in case.topo.names if n != case.root]
    victims = [n for n in case.onsets if n != case.root]
    case.knobs |= {
        "decoys": _herring_decoys(case, others),
        "contained_fault": _herring_contained(case, others),
        "decoy_deploy": _herring_deploy(case, others, victims),
        "warn_storm": _herring_storm(case, others, n_target),
        "slow_herring": _herring_slow(case, others, n_target),
        "pre_incident_error": _herring_pre_error(case, others),
        "chronic_errors": _herring_chronic(case, others),
        "skew_herring": _herring_skew(case, others, victims),
    }


# --------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------


def offset_at(skews: dict[str, list[tuple[float, float]]], service: str, t: float) -> float:
    return sum(off for start, off in skews.get(service, []) if t >= start)


def _render(
    topo: Topology, base: datetime, service: str, disp_ms: int, level: str, msg: str
) -> str:
    s = topo.service(service)
    ts = base + timedelta(milliseconds=disp_ms)
    frac = f"{ts.microsecond // 1000:03d}"
    if s.fmt == "plain":
        return f"{ts:%Y-%m-%d %H:%M:%S}.{frac} {level:<5} [{service}] {msg}"
    if s.fmt == "syslog":
        stamp = f"{MONTHS[ts.month - 1]} {ts.day:2d} {ts:%H:%M:%S}.{frac}"
        return f"{stamp} {s.host} {service}[{s.pid}]: {level} {msg}"
    iso = f"{ts:%Y-%m-%dT%H:%M:%S}.{frac}Z"
    if s.fmt == "kv":
        return f'ts={iso} level={level.lower()} svc={service} msg="{msg}" host={s.host}'
    record = {"ts": iso, "level": level.lower(), "service": service, "msg": msg}
    return json.dumps(record, separators=(",", ":"))


@dataclass
class Draft:
    state: str
    question: dict[str, Any]
    answer: str
    shortcuts: dict[str, str]
    source: dict[str, Any]
    spec: dict[str, Any]


def _thin(case: Case, n_target: int) -> list[Line]:
    rng = case.rng
    keep = [ln for ln in case.lines if ln.keep and ln.t <= case.t_end]
    reps = [ln for ln in case.lines if not ln.keep and ln.t <= case.t_end]
    room = max(0, n_target - len(keep))
    noise = _noise(case, int(room * 1.4) + 20)
    n_noise = min(len(noise), round(room * case.knobs["noise_share"]))
    n_reps = min(len(reps), room - n_noise)
    n_noise = min(len(noise), room - n_reps)
    return keep + rng.sample(reps, n_reps) + rng.sample(noise, n_noise)


def _names_in(msg: str, names: set[str], skip: str) -> list[str]:
    return [tok for tok in _TOKEN.findall(msg) if tok in names and tok != skip]


def log_shortcuts(rows: Sequence[tuple[str, str, str]], names: Sequence[str]) -> dict[str, str]:
    """Surface-feature guesses from (service, level, message) rows in rendered order."""
    known = set(names)
    alpha = sorted(names)
    errors = [svc for svc, level, _ in rows if level == "ERROR"]
    err_count = Counter(errors)
    line_count = Counter(svc for svc, _, _ in rows)
    blamed: Counter[str] = Counter()
    for svc, level, msg in rows:
        if level in ("WARN", "ERROR"):
            blamed.update(_names_in(msg, known, svc))
    rooted = next(((svc, msg) for svc, _, msg in rows if _ROOT_WORD.search(msg)), (rows[0][0], ""))
    mentioned = _names_in(rooted[1], known, rooted[0])
    return {
        "first_error_service": errors[0] if errors else rows[0][0],
        "most_errors_service": max(alpha, key=lambda n: err_count[n]),
        "most_log_lines_service": max(alpha, key=lambda n: line_count[n]),
        "last_error_service": errors[-1] if errors else rows[-1][0],
        "root_or_caused_line": mentioned[0] if mentioned else rooted[0],
        "most_blamed_service": max(alpha, key=lambda n: blamed[n]),
    }


def _state(
    case: Case, rows: Sequence[str], show_map: bool, first_ms: int, last_ms: int, base: datetime
) -> str:
    topo = case.topo
    start = base + timedelta(milliseconds=first_ms)
    end = base + timedelta(milliseconds=last_ms)
    head = [
        f"Incident review: merged logs from {len(topo.names)} services, "
        f"{start:%Y-%m-%d %H:%M:%S} to {end:%H:%M:%S} UTC.",
        "Lines are merged by the timestamp each host recorded.",
        "Services (address): "
        + ", ".join(f"{n} ({topo.service(n).ip})" for n in sorted(topo.names))
        + ".",
    ]
    if show_map:
        head += ["", "Dependency map (each line reads: service depends on service):"]
        phrase = {"publish": "publishes to", "consume": "consumes from"}
        for e in sorted(topo.edges, key=lambda e: (e.src, e.dst)):
            if e.kind in phrase:
                head.append(f"  {e.src} {phrase[e.kind]} {e.dst}")
            else:
                head.append(f"  {e.src} calls {e.dst} over {e.kind}")
    return "\n".join([*head, "", "Logs:", *rows]) + "\n"


def _candidate(
    rng: random.Random, topo: Topology, kind: str, root: str, position: int
) -> Draft | None:
    case = Case(rng, topo, kind, root)
    case.t_f = rng.uniform(90, 360)
    _setup(case)
    case.fallback = frozenset(
        (e.src, e.dst) for e in topo.edges if e.kind in ("http", "grpc") and rng.random() < 0.12
    )
    explicit = kind == "clock_skew" or case.detail.get("variant") == "storm"
    case.knobs = {
        "root_explicit": explicit or rng.random() < 0.4,
        "addressing": rng.choice(("name", "name", "ip", "path")),
    }
    _Propagator(case).run()
    if not case.onsets:
        return None
    last = max(o.t for o in case.onsets.values())
    case.t_end = max(case.t_imp + 150, last + rng.uniform(60, 200))
    n_target = rng.randint(MIN_LINES, MAX_LINES)
    impaired = [o for o in case.onsets.values() if o.mode in IMPAIRED]
    far = [o.service for o in impaired if o.cause != root]
    near = [o.service for o in impaired if o.cause == root]
    heavy = rng.choice(far if far and rng.random() < 0.7 else near or far or [""])
    case.knobs |= {
        "root_volume": rng.choice(("high", "low", "low", "low")),
        "noise_share": rng.uniform(0.35, 0.6),
        "error_heavy_victim": bool(heavy),
        "root_logs_failures_at_warn": rng.random() < 0.45,
        "root_error_delay": rng.random() < 0.5,
        "root_quiet_tail": rng.random() < 0.5,
        "generic_first_symptoms": rng.random() < 0.5,
    }
    case.t_err = case.t_imp + (case.u(5, 30) if case.knobs["root_error_delay"] else 0.0)
    quiet = [n for n in case.onsets if rng.random() < 0.3]
    if case.knobs["root_logs_failures_at_warn"]:
        quiet.append(root)
    case.warn_only = frozenset(quiet)
    if kind == "clock_skew":
        case.skews[root] = [(case.t_f, case.detail["offset"])]
    keep, reps, extra = (ROOTS[kind] if case.knobs["root_explicit"] else _root_generic)(case)
    for i, (t, level, msg) in enumerate(keep):
        case.emit(t, root, level, msg, "root_first" if i == 0 else "root", True)
    for t, level, msg in extra:
        case.emit(t, root, level, msg, "root", False)
    root_period = case.u(1.5, 4) if case.knobs["root_volume"] == "high" else case.u(15, 45)
    tail = case.t_err + case.u(0.3, 0.9) * (case.t_end - case.t_err)
    _repeat(
        case,
        root,
        case.t_err,
        root_period,
        reps,
        "root",
        tail if case.knobs["root_quiet_tail"] else None,
    )
    for o in sorted(case.onsets.values(), key=lambda o: o.t):
        sym_keep, sym_reps = SYMPTOMS[o.mode](case, o)
        lag = 0.0
        if (
            case.knobs["generic_first_symptoms"]
            and o.mode in ("errors", "slow")
            and rng.random() < 0.6
        ):
            # The victim first fails without saying why; its complaint follows.
            level, text = _generic_symptom(case, o)
            case.emit(o.t, o.service, level, text, "symptom", True)
            lag = case.u(2, 12)
        for t, level, msg in sym_keep:
            case.emit(t + lag, o.service, level, msg, "symptom", True)
        period = case.u(4, 15) / (3 if o.service == heavy else 1)
        _repeat(case, o.service, o.t, period, sym_reps, "symptom")
    case.knobs["secondary_failure"] = _secondary(case)
    if case.knobs["addressing"] == "path":
        _route_samples(case)
    _herrings(case, n_target)
    n_keep = sum(ln.keep for ln in case.lines if ln.t <= case.t_end)
    if n_keep + 20 > MAX_LINES:
        return None
    lines = _thin(case, max(n_target, n_keep + 20))
    return _finish(case, lines, position)


def _finish(case: Case, lines: list[Line], position: int) -> Draft | None:
    rng, topo, root = case.rng, case.topo, case.root
    stamped = []
    for seq, ln in enumerate(lines):
        t_ms = round(ln.t * 1000)
        disp_ms = t_ms + round(offset_at(case.skews, ln.service, ln.t) * 1000)
        stamped.append((disp_ms, seq, t_ms, ln))
    stamped.sort(key=lambda row: (row[0], row[1]))
    if not MIN_LINES <= len(stamped) <= MAX_LINES:
        return None
    base = datetime(
        2026,
        rng.randint(6, 9),
        rng.randint(1, 28),
        rng.randint(0, 23),
        rng.randint(0, 59),
        tzinfo=UTC,
    )
    rendered = [_render(topo, base, ln.service, d, ln.level, ln.msg) for d, _, _, ln in stamped]
    show_map = rng.random() < 0.5
    state = _state(case, rendered, show_map, stamped[0][0], stamped[-1][0], base)
    if len(state) > MAX_STATE_CHARS:
        return None
    others = [n for n in topo.names if n != root]
    rng.shuffle(others)
    options = [*others[:position], root, *others[position:]]
    shortcuts = log_shortcuts(
        [(ln.service, ln.level, ln.msg) for _, _, _, ln in stamped], topo.names
    )
    shortcuts |= {
        "most_depended_on": most_depended_on(topo),
        "deepest_leaf": deepest_leaf(topo),
        "first_listed_option": options[0],
    }
    depth = {root: 0}
    for o in sorted(case.onsets.values(), key=lambda o: o.t):
        depth[o.service] = depth.get(o.cause or root, 0) + 1
    source = {
        "fault_kind": case.kind,
        "root_role": topo.role(root),
        "services": len(topo.names),
        "lines": len(stamped),
        "show_map": show_map,
        "symptomatic_services": len(case.onsets),
        "chain_depth": max(depth.values()),
        "formats": len({topo.service(n).fmt for n in topo.names}),
        "fallback_edges": len(case.fallback),
        **{k: v for k, v in case.knobs.items() if k != "noise_share"},
    }
    spec = {
        "topology": topo,
        "root": root,
        "kind": case.kind,
        "t_f": case.t_f,
        "t_imp": case.t_imp,
        "detail": dict(case.detail),
        "onsets": list(case.onsets.values()),
        "skews": dict(case.skews),
        "base": base,
        "records": [
            {
                "service": ln.service,
                "t_ms": t_ms,
                "disp_ms": d,
                "level": ln.level,
                "cat": ln.cat,
                "msg": ln.msg,
            }
            for d, _, t_ms, ln in stamped
        ],
    }
    question = choice_question(QUESTION, dict.fromkeys(options))
    return Draft(state, question, root, shortcuts, source, spec)


# --------------------------------------------------------------------------
# Selection and the builder
# --------------------------------------------------------------------------


def unit_plan(per_family: int) -> list[int]:
    """Seed group of each item: five items per group (50 groups for 250 items)."""
    n_units = max(1, math.ceil(per_family / ITEMS_PER_UNIT))
    return [i * n_units // per_family for i in range(per_family)]


class _Balancer:
    """Keeps every shortcut's running hit count as close to chance as it can."""

    def __init__(self) -> None:
        self.hits: Counter[str] = Counter()
        self.expected = 0.0

    def cost(self, draft: Draft) -> float:
        truth = answer_label(draft.question, draft.answer)
        target = self.expected + 1 / len(labels_for(draft.question))
        return sum(
            abs(self.hits[name] + (guess == truth) - target)
            for name, guess in draft.shortcuts.items()
        )

    def commit(self, draft: Draft) -> None:
        truth = answer_label(draft.question, draft.answer)
        for name, guess in draft.shortcuts.items():
            self.hits[name] += int(guess == truth)
        self.expected += 1 / len(labels_for(draft.question))


def _draw(rng: random.Random, topo: Topology, kinds: Counter[str], position: int) -> Draft | None:
    viable = [k for k in KINDS if eligible_roots(topo, k)]
    low = min(kinds[k] for k in viable)
    kind = rng.choice([k for k in viable if kinds[k] <= low + 1])
    root = rng.choice(eligible_roots(topo, kind))
    return _candidate(rng, topo, kind, root, position)


def generate(ctx: BuildContext) -> list[tuple[Item, Draft]]:
    rng = random.Random(f"verdict-v2:{ctx.seed}:{FAMILY}")
    balancer = _Balancer()
    kinds: Counter[str] = Counter()
    positions: Counter[int] = Counter()
    topologies: dict[int, Topology] = {}
    roots_used: dict[int, Counter[str]] = {}
    out = []
    for i, g in enumerate(unit_plan(ctx.per_family)):
        topo = topologies.setdefault(g, make_topology(rng))
        used = roots_used.setdefault(g, Counter())
        k = len(topo.names)
        position = positions[k] % k
        drafts: list[Draft] = []
        for _ in range(CANDIDATES * 20):
            draft = _draw(rng, topo, kinds, position)
            if draft is not None:
                drafts.append(draft)
            if len(drafts) >= CANDIDATES:
                break
        if not drafts:
            raise RuntimeError(f"no valid incident for group {g}")
        chosen = min(drafts, key=lambda d: balancer.cost(d) + 1.5 * used[d.answer])
        balancer.commit(chosen)
        kinds[chosen.source["fault_kind"]] += 1
        positions[k] += 1
        used[chosen.answer] += 1
        item = make_item(
            family=FAMILY,
            key=f"{ctx.seed}:{FAMILY}:{i}",
            source_unit=f"{FAMILY}-g{g:02d}",
            state=chosen.state,
            question=chosen.question,
            answer=chosen.answer,
            reference=REFERENCE,
            shortcuts=chosen.shortcuts,
            source={**chosen.source, "seed_group": g},
        )
        out.append((item, chosen))
    return out


def build(ctx: BuildContext) -> list[Item]:
    return [item for item, _ in generate(ctx)]


BUILDERS: dict[str, Callable[[BuildContext], list[Item]]] = {FAMILY: build}

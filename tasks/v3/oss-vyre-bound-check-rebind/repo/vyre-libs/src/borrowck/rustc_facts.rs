//! Producer: load rustc's `-Znll-facts` dump and run the Polonius "Naive"
//! borrow-check ruleset on the real MIR-level facts, so the borrow checker
//! applies to real Rust functions, not only the nano-subset our own front-end
//! parses, and reaches the *same* verdict rustc does.
//!
//! rustc (nightly) writes one directory of tab-separated `.facts` files per
//! function: the Polonius input relations the compiler's own borrow checker
//! consumes. We intern their string atoms (`"Start(bb0[5])"`, `"bw0"`, `"'?2"`,
//! `"_1"`) to dense per-category `u32` ids and evaluate the Naive ruleset:
//!
//! ```text
//! var_live_on_entry(V,P)    :- var_used_at(V,P)
//! var_live_on_entry(V,P)    :- var_live_on_entry(V,Q), cfg_edge(P,Q), !var_defined_at(V,P)
//! var_drop_live(V,P)        :- var_dropped_at(V,P)
//! var_drop_live(V,P)        :- var_drop_live(V,Q), cfg_edge(P,Q), !var_defined_at(V,P)
//! region_live_at(O,P)       :- use_of_var_derefs_origin(V,O),  var_live_on_entry(V,P)
//! region_live_at(O,P)       :- drop_of_var_derefs_origin(V,O), var_drop_live(V,P)
//! subset(O1,O2,P)           :- subset_base(O1,O2,P)
//! subset(O1,O3,P)           :- subset(O1,O2,P), subset(O2,O3,P)
//! subset(O1,O2,Q)           :- subset(O1,O2,P), cfg_edge(P,Q), region_live_at(O1,Q), region_live_at(O2,Q)
//! requires(O,L,P)           :- loan_issued_at(O,L,P)
//! requires(O2,L,P)          :- requires(O1,L,P), subset(O1,O2,P)
//! requires(O,L,Q)           :- requires(O,L,P), !loan_killed_at(L,P), cfg_edge(P,Q), region_live_at(O,Q)
//! loan_live_at(L,P)         :- requires(O,L,P), region_live_at(O,P)
//! errors(L,P)               :- loan_invalidated_at(P,L), loan_live_at(L,P)
//! ```
//!
//! `region_live_at` depends only on variable liveness, `subset` only on it, and
//! `requires` on both, so the relations evaluate in stages with no global mutual
//! recursion. Each stage is a monotone fixpoint - the same shape [`super::gpu`]
//! batches on the device; this module is the reference CPU verdict first.

use std::collections::{HashMap, HashSet};

use crate::dataflow::{SharedFactHeader, SharedFactKind, Soundness};

/// A program point id (interned from a rustc point atom).
pub type Point = u32;
/// A loan id (interned from a rustc loan atom such as `bw0`).
pub type Loan = u32;
/// An origin/region id (interned from a rustc origin atom such as `'?2`).
pub type Origin = u32;
/// A MIR variable id (interned from a rustc variable atom such as `_1`).
pub type Var = u32;

/// rustc NLL input facts for one function, interned to dense per-category ids.
#[derive(Debug, Default, Clone)]
pub struct RustcNllFacts {
    /// Number of distinct interned program points.
    pub point_count: u32,
    /// Number of distinct interned origins (regions).
    pub origin_count: u32,
    /// Number of distinct interned MIR variables.
    pub var_count: u32,
    /// Number of distinct interned loans.
    pub loan_count: u32,
    /// Control-flow successor edges `(from, to)`.
    pub cfg_edge: Vec<(Point, Point)>,
    /// `(origin, loan, point)`: loan created at point, placed in origin.
    pub loan_issued_at: Vec<(Origin, Loan, Point)>,
    /// `(point, loan)`: an access at the point invalidates the loan.
    pub loan_invalidated_at: Vec<(Point, Loan)>,
    /// `(loan, point)`: the loan's referent is overwritten at the point.
    pub loan_killed_at: Vec<(Loan, Point)>,
    /// `(o1, o2, point)`: base region-subset relation `o1 <= o2` at the point.
    pub subset_base: Vec<(Origin, Origin, Point)>,
    /// `(var, point)`: the variable is used at the point.
    pub var_used_at: Vec<(Var, Point)>,
    /// `(var, point)`: the variable is (re)defined at the point.
    pub var_defined_at: Vec<(Var, Point)>,
    /// `(var, point)`: the variable is dropped at the point.
    pub var_dropped_at: Vec<(Var, Point)>,
    /// `(var, origin)`: a use of the variable dereferences the origin.
    pub use_of_var_derefs_origin: Vec<(Var, Origin)>,
    /// `(var, origin)`: a drop of the variable dereferences the origin.
    pub drop_of_var_derefs_origin: Vec<(Var, Origin)>,
    /// `(origin, loan)`: origin is a placeholder (a universal/free region).
    pub placeholder: Vec<(Origin, Loan)>,
    /// `(o1, o2)`: the subset `o1 <= o2` between placeholders is known/declared.
    pub known_placeholder_subset: Vec<(Origin, Origin)>,
    /// Origins that are universal (free) regions, live at every point.
    pub universal_region: Vec<Origin>,
    /// Interned point atom for each point id (for diagnostics).
    pub point_names: Vec<String>,
    /// Interned loan atom for each loan id (for diagnostics).
    pub loan_names: Vec<String>,
}

/// An NLL borrow-check error: a loan invalidated while live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NllError {
    /// The loan that was live when invalidated.
    pub loan: Loan,
    /// The point at which the invalidating access occurred.
    pub point: Point,
}

#[derive(Default)]
struct Interner {
    ids: HashMap<String, u32>,
    names: Vec<String>,
}

impl Interner {
    fn intern(&mut self, atom: &str) -> u32 {
        if let Some(&id) = self.ids.get(atom) {
            return id;
        }
        let id = self.names.len() as u32;
        self.ids.insert(atom.to_string(), id);
        self.names.push(atom.to_string());
        id
    }
}

fn unquote(field: &str) -> &str {
    let field = field.trim();
    field
        .strip_prefix('"')
        .and_then(|f| f.strip_suffix('"'))
        .unwrap_or(field)
}

fn fields(line: &str) -> Vec<&str> {
    line.split('\t').map(unquote).collect()
}

/// Load the relations from one function's facts directory. `read(name)` returns
/// the raw text of `<name>.facts` (or "" if rustc did not emit it).
#[must_use]
pub fn load_facts(read: impl Fn(&str) -> String) -> RustcNllFacts {
    let mut points = Interner::default();
    let mut origins = Interner::default();
    let mut vars = Interner::default();
    let mut loans = Interner::default();

    let for_each = |name: &str, mut f: Box<dyn FnMut(Vec<&str>) + '_>| {
        let text = read(name);
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            f(fields(line));
        }
    };

    let mut cfg_edge = Vec::new();
    for_each(
        "cfg_edge",
        Box::new(|f| {
            if f.len() == 2 {
                cfg_edge.push((points.intern(f[0]), points.intern(f[1])));
            }
        }),
    );
    let mut loan_issued_at = Vec::new();
    for_each(
        "loan_issued_at",
        Box::new(|f| {
            if f.len() == 3 {
                loan_issued_at.push((
                    origins.intern(f[0]),
                    loans.intern(f[1]),
                    points.intern(f[2]),
                ));
            }
        }),
    );
    let mut loan_invalidated_at = Vec::new();
    for_each(
        "loan_invalidated_at",
        Box::new(|f| {
            if f.len() == 2 {
                loan_invalidated_at.push((points.intern(f[0]), loans.intern(f[1])));
            }
        }),
    );
    let mut loan_killed_at = Vec::new();
    for_each(
        "loan_killed_at",
        Box::new(|f| {
            if f.len() == 2 {
                loan_killed_at.push((loans.intern(f[0]), points.intern(f[1])));
            }
        }),
    );
    let mut subset_base = Vec::new();
    for_each(
        "subset_base",
        Box::new(|f| {
            if f.len() == 3 {
                subset_base.push((
                    origins.intern(f[0]),
                    origins.intern(f[1]),
                    points.intern(f[2]),
                ));
            }
        }),
    );
    let mut var_used_at = Vec::new();
    for_each(
        "var_used_at",
        Box::new(|f| {
            if f.len() == 2 {
                var_used_at.push((vars.intern(f[0]), points.intern(f[1])));
            }
        }),
    );
    let mut var_defined_at = Vec::new();
    for_each(
        "var_defined_at",
        Box::new(|f| {
            if f.len() == 2 {
                var_defined_at.push((vars.intern(f[0]), points.intern(f[1])));
            }
        }),
    );
    let mut var_dropped_at = Vec::new();
    for_each(
        "var_dropped_at",
        Box::new(|f| {
            if f.len() == 2 {
                var_dropped_at.push((vars.intern(f[0]), points.intern(f[1])));
            }
        }),
    );
    let mut use_of_var_derefs_origin = Vec::new();
    for_each(
        "use_of_var_derefs_origin",
        Box::new(|f| {
            if f.len() == 2 {
                use_of_var_derefs_origin.push((vars.intern(f[0]), origins.intern(f[1])));
            }
        }),
    );
    let mut drop_of_var_derefs_origin = Vec::new();
    for_each(
        "drop_of_var_derefs_origin",
        Box::new(|f| {
            if f.len() == 2 {
                drop_of_var_derefs_origin.push((vars.intern(f[0]), origins.intern(f[1])));
            }
        }),
    );
    let mut placeholder = Vec::new();
    for_each(
        "placeholder",
        Box::new(|f| {
            if f.len() == 2 {
                placeholder.push((origins.intern(f[0]), loans.intern(f[1])));
            }
        }),
    );
    let mut known_placeholder_subset = Vec::new();
    for_each(
        "known_placeholder_subset",
        Box::new(|f| {
            if f.len() == 2 {
                known_placeholder_subset.push((origins.intern(f[0]), origins.intern(f[1])));
            }
        }),
    );
    let mut universal_region = Vec::new();
    for_each(
        "universal_region",
        Box::new(|f| {
            if f.len() == 1 {
                universal_region.push(origins.intern(f[0]));
            }
        }),
    );

    RustcNllFacts {
        point_count: points.names.len() as u32,
        origin_count: origins.names.len() as u32,
        var_count: vars.names.len() as u32,
        loan_count: loans.names.len() as u32,
        cfg_edge,
        loan_issued_at,
        loan_invalidated_at,
        loan_killed_at,
        subset_base,
        var_used_at,
        var_defined_at,
        var_dropped_at,
        use_of_var_derefs_origin,
        drop_of_var_derefs_origin,
        placeholder,
        known_placeholder_subset,
        universal_region,
        point_names: points.names,
        loan_names: loans.names,
    }
}

impl RustcNllFacts {
    /// Convert rustc NLL relations into shared fact headers.
    #[must_use]
    pub fn shared_fact_headers(&self, producer: &str) -> Vec<SharedFactHeader> {
        let mut headers = Vec::new();
        let mut next_id = 1u64;
        for &(from, to) in &self.cfg_edge {
            headers.push(
                SharedFactHeader::new(
                    producer,
                    SharedFactKind::GraphEdge,
                    next_id,
                    u64::from(from),
                    Soundness::Exact,
                )
                .with_object(u64::from(to)),
            );
            next_id += 1;
        }
        for &(origin, loan, point) in &self.loan_issued_at {
            headers.push(
                SharedFactHeader::new(
                    producer,
                    SharedFactKind::BorrowLoan,
                    next_id,
                    u64::from(loan),
                    Soundness::Exact,
                )
                .with_object(u64::from(origin))
                .with_aux(u64::from(point)),
            );
            next_id += 1;
        }
        for &(origin, loan) in &self.placeholder {
            headers.push(
                SharedFactHeader::new(
                    producer,
                    SharedFactKind::BorrowOrigin,
                    next_id,
                    u64::from(origin),
                    Soundness::Exact,
                )
                .with_object(u64::from(loan)),
            );
            next_id += 1;
        }
        for &(left, right) in &self.known_placeholder_subset {
            headers.push(
                SharedFactHeader::new(
                    producer,
                    SharedFactKind::BorrowSubset,
                    next_id,
                    u64::from(left),
                    Soundness::Exact,
                )
                .with_object(u64::from(right)),
            );
            next_id += 1;
        }
        for &origin in &self.universal_region {
            headers.push(SharedFactHeader::new(
                producer,
                SharedFactKind::BorrowOrigin,
                next_id,
                u64::from(origin),
                Soundness::Exact,
            ));
            next_id += 1;
        }
        headers
    }

    /// Backward variable liveness: `live(V,P)` if `V` is used at `P`, or live at
    /// a CFG successor and not defined at `P`.
    fn var_liveness(&self, seeds: &[(Var, Point)]) -> HashSet<(Var, Point)> {
        let defined: HashSet<(Var, Point)> = self.var_defined_at.iter().copied().collect();
        // preds[Q] = points P with cfg_edge(P, Q)
        let mut preds: HashMap<Point, Vec<Point>> = HashMap::new();
        for &(p, q) in &self.cfg_edge {
            preds.entry(q).or_default().push(p);
        }
        let mut live: HashSet<(Var, Point)> = seeds.iter().copied().collect();
        let mut work: Vec<(Var, Point)> = live.iter().copied().collect();
        while let Some((v, q)) = work.pop() {
            if let Some(ps) = preds.get(&q) {
                for &p in ps {
                    if !defined.contains(&(v, p)) && live.insert((v, p)) {
                        work.push((v, p));
                    }
                }
            }
        }
        live
    }

    /// `region_live_at`: an origin is live where a variable that derefs it is
    /// (use- or drop-) live; universal (free) regions are live at every point.
    fn region_live_at(&self) -> HashSet<(Origin, Point)> {
        let var_live = self.var_liveness(&self.var_used_at);
        let drop_live = self.var_liveness(&self.var_dropped_at);
        // Index liveness by variable so each deref fact is an O(1) lookup of the
        // points where its variable is live, not a linear scan of the whole
        // liveness set per deref fact.
        let index_by_var = |live: &HashSet<(Var, Point)>| {
            let mut m: HashMap<Var, Vec<Point>> = HashMap::new();
            for &(v, p) in live {
                m.entry(v).or_default().push(p);
            }
            m
        };
        let use_by_var = index_by_var(&var_live);
        let drop_by_var = index_by_var(&drop_live);
        let mut region = HashSet::new();
        for &(v, o) in &self.use_of_var_derefs_origin {
            if let Some(ps) = use_by_var.get(&v) {
                for &p in ps {
                    region.insert((o, p));
                }
            }
        }
        for &(v, o) in &self.drop_of_var_derefs_origin {
            if let Some(ps) = drop_by_var.get(&v) {
                for &p in ps {
                    region.insert((o, p));
                }
            }
        }
        // Universal regions are live throughout the function.
        for &o in &self.universal_region {
            for p in 0..self.point_count {
                region.insert((o, p));
            }
        }
        region
    }

    /// `subset(O1,O2,P)`: base subsets, their transitive closure, and forward
    /// CFG propagation where both origins are live.
    ///
    /// Evaluated semi-naively: each round joins only the previous round's newly
    /// derived tuples (the delta) against the relation, instead of re-scanning
    /// the entire `subset` set and rebuilding its index every round. The two
    /// per-point indexes (`idx_first[(a,p)] = {b}`, `idx_second[(b,p)] = {a}`)
    /// are maintained incrementally as tuples are added. This is the identical
    /// least fixpoint as a naive evaluation, computed without the O(n^2)-per-
    /// round re-scan.
    fn subset_closure(
        &self,
        region_live: &HashSet<(Origin, Point)>,
        succ: &HashMap<Point, Vec<Point>>,
    ) -> HashSet<(Origin, Origin, Point)> {
        let mut subset: HashSet<(Origin, Origin, Point)> = HashSet::new();
        // Incrementally-maintained indexes over `subset` (the full relation):
        // `idx_first[(a,p)]` lists every `b` with `(a,b,p)`; `idx_second[(b,p)]`
        // lists every `a` with `(a,b,p)`.
        let mut idx_first: HashMap<(Origin, Point), Vec<Origin>> = HashMap::new();
        let mut idx_second: HashMap<(Origin, Point), Vec<Origin>> = HashMap::new();
        let mut delta: Vec<(Origin, Origin, Point)> = Vec::new();

        let admit = |subset: &mut HashSet<(Origin, Origin, Point)>,
                     idx_first: &mut HashMap<(Origin, Point), Vec<Origin>>,
                     idx_second: &mut HashMap<(Origin, Point), Vec<Origin>>,
                     delta: &mut Vec<(Origin, Origin, Point)>,
                     t: (Origin, Origin, Point)| {
            if subset.insert(t) {
                let (a, b, p) = t;
                idx_first.entry((a, p)).or_default().push(b);
                idx_second.entry((b, p)).or_default().push(a);
                delta.push(t);
            }
        };

        for &t in &self.subset_base {
            admit(&mut subset, &mut idx_first, &mut idx_second, &mut delta, t);
        }

        while !delta.is_empty() {
            // Derive against the relation as it stood at the start of the round;
            // indexes are only mutated in the commit phase below.
            let mut derived: Vec<(Origin, Origin, Point)> = Vec::new();
            for &(x, y, p) in &delta {
                // Transitivity, delta as the left premise: (x,y,p) & (y,z,p).
                if let Some(zs) = idx_first.get(&(y, p)) {
                    for &z in zs {
                        if !subset.contains(&(x, z, p)) {
                            derived.push((x, z, p));
                        }
                    }
                }
                // Transitivity, delta as the right premise: (w,x,p) & (x,y,p).
                if let Some(ws) = idx_second.get(&(x, p)) {
                    for &w in ws {
                        if !subset.contains(&(w, y, p)) {
                            derived.push((w, y, p));
                        }
                    }
                }
                // CFG propagation where both origins remain live at the successor.
                if let Some(qs) = succ.get(&p) {
                    for &q in qs {
                        if region_live.contains(&(x, q))
                            && region_live.contains(&(y, q))
                            && !subset.contains(&(x, y, q))
                        {
                            derived.push((x, y, q));
                        }
                    }
                }
            }
            delta.clear();
            for t in derived {
                admit(&mut subset, &mut idx_first, &mut idx_second, &mut delta, t);
            }
        }
        subset
    }

    /// CFG successor adjacency `succ[p] = {q : edge(p,q)}`.
    fn succ_edges(&self) -> HashMap<Point, Vec<Point>> {
        let mut m: HashMap<Point, Vec<Point>> = HashMap::new();
        for &(p, q) in &self.cfg_edge {
            m.entry(p).or_default().push(q);
        }
        m
    }

    /// The shared analysis base used by every error relation: region liveness,
    /// the CFG successor map, and the `subset` closure. Computed once so
    /// [`accepts`](Self::accepts) does not derive it twice (once per error kind).
    fn region_subset(
        &self,
    ) -> (
        HashSet<(Origin, Point)>,
        HashMap<Point, Vec<Point>>,
        HashSet<(Origin, Origin, Point)>,
    ) {
        let region_live = self.region_live_at();
        let succ = self.succ_edges();
        let subset = self.subset_closure(&region_live, &succ);
        (region_live, succ, subset)
    }

    /// Illegal-subset (region-outlives) errors from a precomputed `subset`: a
    /// derived subset between two placeholder origins that is not a
    /// known/declared placeholder subset.
    fn subset_errors_from(
        &self,
        subset: &HashSet<(Origin, Origin, Point)>,
    ) -> Vec<(Origin, Origin)> {
        let placeholders: HashSet<Origin> = self.placeholder.iter().map(|&(o, _)| o).collect();
        if placeholders.is_empty() {
            return Vec::new();
        }
        let known: HashSet<(Origin, Origin)> =
            self.known_placeholder_subset.iter().copied().collect();
        let mut errors: Vec<(Origin, Origin)> = subset
            .iter()
            .filter(|&&(a, b, _)| {
                a != b
                    && placeholders.contains(&a)
                    && placeholders.contains(&b)
                    && !known.contains(&(a, b))
            })
            .map(|&(a, b, _)| (a, b))
            .collect();
        errors.sort_unstable();
        errors.dedup();
        errors
    }

    /// Illegal-subset (region-outlives) errors: a derived subset between two
    /// placeholder origins that is not a known/declared placeholder subset.
    /// These are the escape/lifetime errors (rustc E0515 / E0521 and kin) that
    /// carry no loan invalidation.
    #[must_use]
    pub fn subset_errors(&self) -> Vec<(Origin, Origin)> {
        if self.point_count == 0 || self.universal_region.is_empty() {
            return Vec::new();
        }
        let (_, _, subset) = self.region_subset();
        self.subset_errors_from(&subset)
    }

    /// Polonius "Naive" loan-invalidation errors from a precomputed base.
    fn nll_errors_from(
        &self,
        region_live_set: &HashSet<(Origin, Point)>,
        succ: &HashMap<Point, Vec<Point>>,
        subset: &HashSet<(Origin, Origin, Point)>,
    ) -> Vec<NllError> {
        // requires(O,L,P): issued, via subset, and forward while live + not
        // killed. Evaluated semi-naively: only the previous round's newly
        // derived `requires` tuples (the delta) drive new derivations.
        // `subset_by_first`, `killed`, `region_live_set`, and `succ` are static
        // here, so no index needs rebuilding per round.
        let killed: HashSet<(Loan, Point)> = self.loan_killed_at.iter().copied().collect();
        let subset_by_first: HashMap<(Origin, Point), Vec<Origin>> = {
            let mut m: HashMap<(Origin, Point), Vec<Origin>> = HashMap::new();
            for &(a, b, p) in subset {
                m.entry((a, p)).or_default().push(b);
            }
            m
        };
        let mut requires: HashSet<(Origin, Loan, Point)> = HashSet::new();
        let mut delta: Vec<(Origin, Loan, Point)> = Vec::new();
        for &(o, l, p) in &self.loan_issued_at {
            if requires.insert((o, l, p)) {
                delta.push((o, l, p));
            }
        }
        while !delta.is_empty() {
            let mut derived: Vec<(Origin, Loan, Point)> = Vec::new();
            for &(o, l, p) in &delta {
                if let Some(o2s) = subset_by_first.get(&(o, p)) {
                    for &o2 in o2s {
                        if !requires.contains(&(o2, l, p)) {
                            derived.push((o2, l, p));
                        }
                    }
                }
                if !killed.contains(&(l, p)) {
                    if let Some(qs) = succ.get(&p) {
                        for &q in qs {
                            if region_live_set.contains(&(o, q)) && !requires.contains(&(o, l, q)) {
                                derived.push((o, l, q));
                            }
                        }
                    }
                }
            }
            delta.clear();
            for t in derived {
                if requires.insert(t) {
                    delta.push(t);
                }
            }
        }

        // loan_live_at(L,P): some origin requiring L is live at P.
        let mut loan_live: HashSet<(Loan, Point)> = HashSet::new();
        for &(o, l, p) in &requires {
            if region_live_set.contains(&(o, p)) {
                loan_live.insert((l, p));
            }
        }

        // errors: invalidated while live.
        let mut errors: Vec<NllError> = self
            .loan_invalidated_at
            .iter()
            .filter(|&&(p, l)| loan_live.contains(&(l, p)))
            .map(|&(p, l)| NllError { loan: l, point: p })
            .collect();
        errors.sort_unstable_by_key(|e| (e.loan, e.point));
        errors.dedup();
        errors
    }

    /// Compute the Polonius "Naive" borrow-check errors for this function.
    #[must_use]
    pub fn nll_errors(&self) -> Vec<NllError> {
        if self.point_count == 0 || self.loan_count == 0 {
            return Vec::new();
        }
        let (region_live, succ, subset) = self.region_subset();
        self.nll_errors_from(&region_live, &succ, &subset)
    }

    /// Whether the function borrow-checks: no conflicting-borrow (loan
    /// invalidation) errors and no illegal-subset (region-outlives) errors.
    /// This is the verdict rustc's own borrow checker reaches on these facts.
    /// The shared region/subset base is derived once for both error kinds.
    #[must_use]
    pub fn accepts(&self) -> bool {
        let need_nll = self.point_count != 0 && self.loan_count != 0;
        let need_subset = self.point_count != 0 && !self.universal_region.is_empty();
        if !need_nll && !need_subset {
            return true;
        }
        let (region_live, succ, subset) = self.region_subset();
        if need_nll
            && !self
                .nll_errors_from(&region_live, &succ, &subset)
                .is_empty()
        {
            return false;
        }
        if need_subset && !self.subset_errors_from(&subset).is_empty() {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod semi_naive_differential {
    //! Differential oracle: the production semi-naive `nll_errors` /
    //! `subset_errors` must equal an intentionally brute-force naive reference
    //! over thousands of randomly generated fact sets. The reference re-derives
    //! every relation independently (repeat-scan-until-stable, no indexes), so it
    //! shares no code with the production fixpoints it checks. This needs no
    //! rustc; the rustc differential suite in `tests/rustc_nll_facts.rs` checks
    //! the verdict end-to-end on top of this.
    use super::*;

    fn naive_var_liveness(f: &RustcNllFacts, seeds: &[(Var, Point)]) -> HashSet<(Var, Point)> {
        let defined: HashSet<(Var, Point)> = f.var_defined_at.iter().copied().collect();
        let mut live: HashSet<(Var, Point)> = seeds.iter().copied().collect();
        loop {
            let mut added = Vec::new();
            let snapshot: Vec<(Var, Point)> = live.iter().copied().collect();
            for &(p, q) in &f.cfg_edge {
                for &(v, lp) in &snapshot {
                    if lp == q && !defined.contains(&(v, p)) && !live.contains(&(v, p)) {
                        added.push((v, p));
                    }
                }
            }
            if added.is_empty() {
                break;
            }
            for t in added {
                live.insert(t);
            }
        }
        live
    }

    fn naive_region_live(f: &RustcNllFacts) -> HashSet<(Origin, Point)> {
        let var_live = naive_var_liveness(f, &f.var_used_at);
        let drop_live = naive_var_liveness(f, &f.var_dropped_at);
        let mut region = HashSet::new();
        for &(v, o) in &f.use_of_var_derefs_origin {
            for &(lv, p) in &var_live {
                if lv == v {
                    region.insert((o, p));
                }
            }
        }
        for &(v, o) in &f.drop_of_var_derefs_origin {
            for &(lv, p) in &drop_live {
                if lv == v {
                    region.insert((o, p));
                }
            }
        }
        for &o in &f.universal_region {
            for p in 0..f.point_count {
                region.insert((o, p));
            }
        }
        region
    }

    fn succ_map(f: &RustcNllFacts) -> HashMap<Point, Vec<Point>> {
        let mut m: HashMap<Point, Vec<Point>> = HashMap::new();
        for &(p, q) in &f.cfg_edge {
            m.entry(p).or_default().push(q);
        }
        m
    }

    fn naive_subset(
        f: &RustcNllFacts,
        region_live: &HashSet<(Origin, Point)>,
    ) -> HashSet<(Origin, Origin, Point)> {
        let succ = succ_map(f);
        let mut subset: HashSet<(Origin, Origin, Point)> = f.subset_base.iter().copied().collect();
        loop {
            let mut added = Vec::new();
            let snapshot: Vec<(Origin, Origin, Point)> = subset.iter().copied().collect();
            for &(a, b, p) in &snapshot {
                for &(b2, c, p2) in &snapshot {
                    if b2 == b && p2 == p && !subset.contains(&(a, c, p)) {
                        added.push((a, c, p));
                    }
                }
                if let Some(qs) = succ.get(&p) {
                    for &q in qs {
                        if region_live.contains(&(a, q))
                            && region_live.contains(&(b, q))
                            && !subset.contains(&(a, b, q))
                        {
                            added.push((a, b, q));
                        }
                    }
                }
            }
            if added.is_empty() {
                break;
            }
            for t in added {
                subset.insert(t);
            }
        }
        subset
    }

    fn naive_nll_errors(f: &RustcNllFacts) -> Vec<NllError> {
        if f.point_count == 0 || f.loan_count == 0 {
            return Vec::new();
        }
        let region_live = naive_region_live(f);
        let succ = succ_map(f);
        let subset = naive_subset(f, &region_live);
        let killed: HashSet<(Loan, Point)> = f.loan_killed_at.iter().copied().collect();
        let mut requires: HashSet<(Origin, Loan, Point)> = f
            .loan_issued_at
            .iter()
            .map(|&(o, l, p)| (o, l, p))
            .collect();
        loop {
            let mut added = Vec::new();
            let req_snapshot: Vec<(Origin, Loan, Point)> = requires.iter().copied().collect();
            let sub_snapshot: Vec<(Origin, Origin, Point)> = subset.iter().copied().collect();
            for &(o, l, p) in &req_snapshot {
                for &(a, b, sp) in &sub_snapshot {
                    if a == o && sp == p && !requires.contains(&(b, l, p)) {
                        added.push((b, l, p));
                    }
                }
                if !killed.contains(&(l, p)) {
                    if let Some(qs) = succ.get(&p) {
                        for &q in qs {
                            if region_live.contains(&(o, q)) && !requires.contains(&(o, l, q)) {
                                added.push((o, l, q));
                            }
                        }
                    }
                }
            }
            if added.is_empty() {
                break;
            }
            for t in added {
                requires.insert(t);
            }
        }
        let mut loan_live: HashSet<(Loan, Point)> = HashSet::new();
        for &(o, l, p) in &requires {
            if region_live.contains(&(o, p)) {
                loan_live.insert((l, p));
            }
        }
        let mut errors: Vec<NllError> = f
            .loan_invalidated_at
            .iter()
            .filter(|&&(p, l)| loan_live.contains(&(l, p)))
            .map(|&(p, l)| NllError { loan: l, point: p })
            .collect();
        errors.sort_unstable_by_key(|e| (e.loan, e.point));
        errors.dedup();
        errors
    }

    fn naive_subset_errors(f: &RustcNllFacts) -> Vec<(Origin, Origin)> {
        if f.point_count == 0 || f.universal_region.is_empty() {
            return Vec::new();
        }
        let placeholders: HashSet<Origin> = f.placeholder.iter().map(|&(o, _)| o).collect();
        if placeholders.is_empty() {
            return Vec::new();
        }
        let known: HashSet<(Origin, Origin)> = f.known_placeholder_subset.iter().copied().collect();
        let region_live = naive_region_live(f);
        let subset = naive_subset(f, &region_live);
        let mut errors: Vec<(Origin, Origin)> = subset
            .iter()
            .filter(|&&(a, b, _)| {
                a != b
                    && placeholders.contains(&a)
                    && placeholders.contains(&b)
                    && !known.contains(&(a, b))
            })
            .map(|&(a, b, _)| (a, b))
            .collect();
        errors.sort_unstable();
        errors.dedup();
        errors
    }

    /// Deterministic random fact set. Ids stay within their declared counts so
    /// both implementations interpret them identically; the CFG admits cycles,
    /// self-loops, and branches to exercise the fixpoint propagation.
    fn gen_facts(seed: u64) -> RustcNllFacts {
        let mut state = seed ^ 0xD1B5_4A32_D192_ED03;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as u32
        };
        let point_count = 1 + next() % 6;
        let origin_count = 1 + next() % 4;
        let var_count = 1 + next() % 4;
        let loan_count = 1 + next() % 3;
        let mut f = RustcNllFacts {
            point_count,
            origin_count,
            var_count,
            loan_count,
            ..Default::default()
        };
        for _ in 0..next() % (point_count * 2 + 1) {
            f.cfg_edge
                .push((next() % point_count, next() % point_count));
        }
        for _ in 0..next() % (loan_count + 1) {
            f.loan_issued_at.push((
                next() % origin_count,
                next() % loan_count,
                next() % point_count,
            ));
        }
        for _ in 0..next() % 4 {
            f.loan_invalidated_at
                .push((next() % point_count, next() % loan_count));
        }
        for _ in 0..next() % 3 {
            f.loan_killed_at
                .push((next() % loan_count, next() % point_count));
        }
        for _ in 0..next() % 5 {
            f.subset_base.push((
                next() % origin_count,
                next() % origin_count,
                next() % point_count,
            ));
        }
        for _ in 0..next() % 6 {
            f.var_used_at
                .push((next() % var_count, next() % point_count));
        }
        for _ in 0..next() % 4 {
            f.var_defined_at
                .push((next() % var_count, next() % point_count));
        }
        for _ in 0..next() % 3 {
            f.var_dropped_at
                .push((next() % var_count, next() % point_count));
        }
        for _ in 0..next() % 5 {
            f.use_of_var_derefs_origin
                .push((next() % var_count, next() % origin_count));
        }
        for _ in 0..next() % 3 {
            f.drop_of_var_derefs_origin
                .push((next() % var_count, next() % origin_count));
        }
        for _ in 0..next() % (origin_count + 1) {
            f.placeholder
                .push((next() % origin_count, next() % loan_count));
        }
        for _ in 0..next() % 3 {
            f.known_placeholder_subset
                .push((next() % origin_count, next() % origin_count));
        }
        for _ in 0..next() % (origin_count + 1) {
            f.universal_region.push(next() % origin_count);
        }
        f
    }

    #[test]
    fn semi_naive_matches_naive_reference_over_random_facts() {
        for seed in 0..5000u64 {
            let f = gen_facts(seed);
            assert_eq!(
                f.nll_errors(),
                naive_nll_errors(&f),
                "nll_errors diverged from naive reference at seed {seed}: {f:?}"
            );
            assert_eq!(
                f.subset_errors(),
                naive_subset_errors(&f),
                "subset_errors diverged from naive reference at seed {seed}: {f:?}"
            );
        }
    }
}

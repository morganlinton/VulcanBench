//! Handwritten oracle matrix for tenant registry policy decisions.
//!
//! Compares opcode allocation, global opcode mapping, quiesce backoff, and
//! concurrent-tenant selection against independent reference oracles (512+ cases).

#![forbid(unsafe_code)]

use vyre_runtime::tenant::{
    TenantRegistry, TenantSelectionScratch, OPCODE_RANGE_PER_TENANT, TENANT_OPCODE_BASE,
};

const POLICY_CASES: u32 = 512;

#[test]
fn tenant_global_opcode_oracle_matrix_matches_independent_range_policy() {
    let reg = TenantRegistry::new();
    let handle = reg
        .register("opcode-matrix")
        .expect("Fix: tenant registration must succeed for global_opcode oracle matrix.");
    let mut assertions = 0usize;
    for case in 0..POLICY_CASES {
        let local = case % (OPCODE_RANGE_PER_TENANT / 2);
        let expected = oracle_global_opcode(handle.base_opcode(), local, OPCODE_RANGE_PER_TENANT)
            .expect("in-range local opcode");
        assert_eq!(
            handle.global_opcode(local).expect("in-range local opcode"),
            expected,
            "Fix: tenant global_opcode case {case} local={local} must match the independent oracle."
        );
        assertions += 1;

        let out_of_range = OPCODE_RANGE_PER_TENANT + (case % 17);
        assert!(handle.global_opcode(out_of_range).is_err());
        assert_eq!(
            oracle_global_opcode(handle.base_opcode(), out_of_range, OPCODE_RANGE_PER_TENANT),
            None
        );
        assertions += 2;
    }
    assert_eq!(assertions, POLICY_CASES as usize * 3);
}

#[test]
fn tenant_registration_oracle_matrix_matches_independent_opcode_windows() {
    let reg = TenantRegistry::new();
    let mut assertions = 0usize;
    for case in 0..64 {
        let handle = reg
            .register(format!("window-{case}"))
            .expect("Fix: tenant registration must succeed for opcode-window oracle matrix.");
        let id = handle.id();
        let expected_base = oracle_tenant_base_opcode(id);
        assert_eq!(
            handle.base_opcode(),
            expected_base,
            "Fix: tenant {id} base_opcode case {case} must match the independent allocation oracle."
        );
        assertions += 1;

        if case > 0 {
            let prev = reg
                .lookup(id - 1)
                .expect("Fix: previous tenant must remain registered during oracle matrix.");
            assert!(
                prev.base_opcode() + OPCODE_RANGE_PER_TENANT <= handle.base_opcode(),
                "Fix: tenant opcode windows must not overlap for case {case}."
            );
            assertions += 1;
        }
    }
    assert_eq!(assertions, 64 + 63);
}

#[test]
fn tenant_backpressure_oracle_matrix_matches_independent_outstanding_cap() {
    let reg = TenantRegistry::new();
    let mut assertions = 0usize;
    for case in 0..POLICY_CASES {
        let requested_cap = u64::from(case % 17);
        let handle = reg
            .register_with_backpressure(format!("bounded-{case}"), requested_cap)
            .unwrap_or_else(|error| {
                panic!("Fix: bounded tenant registration case {case} must succeed: {error}")
            });
        let expected_cap = oracle_effective_outstanding_cap(requested_cap);
        assert_eq!(
            handle.max_outstanding_slots(),
            expected_cap,
            "Fix: tenant backpressure case {case} must clamp to max(1, requested cap)."
        );
        assertions += 1;
        assert_eq!(
            handle.runtime_counters().max_outstanding_slots,
            expected_cap,
            "Fix: tenant runtime_counters case {case} must expose the effective cap."
        );
        assertions += 1;
    }
    assert_eq!(assertions, POLICY_CASES as usize * 2);
}

#[test]
fn tenant_concurrent_selection_oracle_matrix_matches_independent_greedy_set() {
    let reg = TenantRegistry::new();
    for case in 0..8 {
        let _ = reg
            .register(format!("sel-{case}"))
            .expect("Fix: tenant selection oracle matrix requires active tenants.");
    }
    let active: Vec<u32> = reg.active_tenants().iter().map(|t| t.id()).collect();
    let n = active.len();
    let mut assertions = 0usize;

    for case in 0..POLICY_CASES {
        let conflicts = hostile_conflict_matrix(case, n);
        let selected = reg.select_concurrent_tenants(&conflicts);
        let expected = oracle_select_concurrent_tenants(&active, &conflicts);
        assert_eq!(
            selected, expected,
            "Fix: select_concurrent_tenants case {case} must match the independent greedy oracle."
        );
        assertions += 1;

        for window in selected.windows(2) {
            let i = active
                .iter()
                .position(|id| *id == window[0])
                .expect("tenant id");
            let j = active
                .iter()
                .position(|id| *id == window[1])
                .expect("tenant id");
            assert_eq!(conflicts[i * n + j], 0);
            assert_eq!(conflicts[j * n + i], 0);
        }
    }
    assert_eq!(assertions, POLICY_CASES as usize);
}

#[test]
fn tenant_selection_scratch_oracle_reuses_caller_storage() {
    let reg = TenantRegistry::new();
    let _a = reg.register("a").unwrap();
    let _b = reg.register("b").unwrap();
    let conflicts = vec![0_u32; 4];
    let mut out = Vec::with_capacity(2);
    let mut scratch = TenantSelectionScratch::new();
    reg.select_concurrent_tenants_into(&conflicts, &mut out, &mut scratch);
    let out_ptr = out.as_ptr();
    reg.select_concurrent_tenants_into(&conflicts, &mut out, &mut scratch);
    assert_eq!(out.as_ptr(), out_ptr);
    assert_eq!(out.len(), 2);
}

fn oracle_effective_outstanding_cap(requested: u64) -> u64 {
    requested.max(1)
}

fn oracle_tenant_base_opcode(id: u32) -> u32 {
    TENANT_OPCODE_BASE
        .checked_add(
            id.checked_mul(OPCODE_RANGE_PER_TENANT)
                .expect("tenant offset must fit u32"),
        )
        .expect("tenant base opcode must fit u32")
}

fn oracle_global_opcode(base: u32, local: u32, cap: u32) -> Option<u32> {
    if local >= cap {
        return None;
    }
    Some(base.checked_add(local).expect("global opcode must fit u32"))
}

fn oracle_select_concurrent_tenants(active: &[u32], conflict_adj: &[u32]) -> Vec<u32> {
    let n = active.len();
    if n == 0 {
        return Vec::new();
    }
    if conflict_adj.len() != n * n {
        return active.to_vec();
    }
    if conflict_adj.iter().all(|value| *value == 0) {
        return active.to_vec();
    }
    let mut selected = Vec::new();
    'candidate: for candidate_idx in 0..n {
        for &selected_idx in &selected {
            let i = active
                .iter()
                .position(|id| *id == selected_idx)
                .expect("selected tenant id");
            if conflict_adj[candidate_idx * n + i] != 0 || conflict_adj[i * n + candidate_idx] != 0
            {
                continue 'candidate;
            }
        }
        selected.push(active[candidate_idx]);
    }
    selected
}

fn hostile_conflict_matrix(seed: u32, n: usize) -> Vec<u32> {
    let mut matrix = vec![0_u32; n * n];
    let mut state = seed ^ 0xC0FF_EE00;
    for i in 0..n {
        for j in (i + 1)..n {
            state = state
                .wrapping_mul(1_103_515_245)
                .wrapping_add(i as u32 ^ j as u32);
            if state & 3 == 0 {
                matrix[i * n + j] = 1;
                matrix[j * n + i] = 1;
            }
        }
    }
    matrix
}

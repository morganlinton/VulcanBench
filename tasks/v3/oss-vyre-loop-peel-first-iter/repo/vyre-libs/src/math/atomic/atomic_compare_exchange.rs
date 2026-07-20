//! Cat-B `atomic_compare_exchange_u32`. CPU ref: for each i, if
//! `state == expected[i]`, replace state with `desired[i]`; always
//! emit the pre-op state into `trace[i]`.

use vyre::ir::Program;

use super::build_atomic_compare_exchange;

const OP_ID: &str = "vyre-libs::math::atomic::atomic_compare_exchange_u32";

/// Sequential compare-and-exchange over pairs `(expected[i], desired[i])`.
#[must_use]
pub fn atomic_compare_exchange_u32(
    expected: &str,
    desired: &str,
    state: &str,
    trace: &str,
    n: u32,
) -> Program {
    build_atomic_compare_exchange(OP_ID, expected, desired, state, trace, n)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || atomic_compare_exchange_u32("expected", "desired", "state", "trace", 4),
        test_inputs: Some(|| {
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[10u32, 99, 20, 30]),
                to_bytes(&[11u32, 88, 21, 31]),
                to_bytes(&[10u32]),
            ]]
        }),
        expected_output: Some(|| {
            // Serial CAS starting at state=10:
            //   i=0: exp=10, state matches → state=11. trace[0]=10.
            //   i=1: exp=99, no match. trace[1]=11.
            //   i=2: exp=20, no match. trace[2]=11.
            //   i=3: exp=30, no match. trace[3]=11.
            // Final state=11, trace=[10,11,11,11].
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![
                to_bytes(&[11u32]),
                to_bytes(&[10u32, 11, 11, 11]),
            ]]
        }),
        category: Some("math"),
    }
}

register_atomic_cas_op!(OP_ID, || atomic_compare_exchange_u32(
    "expected", "desired", "state", "trace", 4
));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::atomic::testutil::run_cas;

    #[test]
    fn swaps_when_expected_matches() {
        let expected = vec![10u32, 99, 20, 30];
        let desired = vec![11u32, 88, 21, 31];
        let initial = 10u32;
        let program = atomic_compare_exchange_u32(
            "expected",
            "desired",
            "state",
            "trace",
            expected.len() as u32,
        );
        let (final_state, trace) = run_cas(&program, &expected, &desired, initial);

        let mut cpu_state = initial;
        let mut cpu_trace = Vec::new();
        for (&e, &d) in expected.iter().zip(desired.iter()) {
            cpu_trace.push(cpu_state);
            if cpu_state == e {
                cpu_state = d;
            }
        }

        assert_eq!(final_state, cpu_state);
        assert_eq!(trace, cpu_trace);
    }
}

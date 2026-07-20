//! Regression for F-IR-33: a second `install_dialect_lookup` call with a
//! conflicting `provider_id` must error at install time rather than silently
//! dropping the installation and leaving the first provider in place. A
//! same-id reinstall must be a no-op so harnesses can defensively call the
//! installer at the top of every test without racing.
//!
//! Both sub-tests are order-independent under a shared process-wide
//! `OnceLock`: the idempotent case tolerates starting from either `None`
//! or an already-installed `ProviderA`, and the error case first
//! normalises the state to `ProviderA` before probing the conflict.

use std::sync::Arc;

use vyre_foundation::dialect_lookup::{
    dialect_lookup, install_dialect_lookup, private::Sealed, DialectLookup, InternedOpId, OpDef,
};

struct DynamicProvider {
    id: &'static str,
}

impl Sealed for DynamicProvider {}

impl DialectLookup for DynamicProvider {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    fn intern_op(&self, _name: &str) -> InternedOpId {
        InternedOpId(0)
    }

    fn lookup(&self, _id: InternedOpId) -> Option<&'static OpDef> {
        None
    }
}

fn active_provider_id() -> &'static str {
    if let Some(existing) = dialect_lookup() {
        existing.provider_id()
    } else {
        let _ = install_dialect_lookup(Arc::new(DynamicProvider {
            id: "test::ProviderA",
        }));
        "test::ProviderA"
    }
}

#[test]
fn same_provider_id_reinstall_is_idempotent() {
    let id = active_provider_id();
    install_dialect_lookup(Arc::new(DynamicProvider { id }))
        .expect("Fix: Provider install should succeed or be idempotent");
    install_dialect_lookup(Arc::new(DynamicProvider { id }))
        .expect("Fix: Provider install should succeed or be idempotent");
    install_dialect_lookup(Arc::new(DynamicProvider { id }))
        .expect("Fix: Provider install should succeed or be idempotent");
}

#[test]
fn same_provider_id_with_different_arc_allocation_is_idempotent() {
    // The provider identity is logical (by provider_id string), not by
    // Arc pointer equality. A fresh Arc with the same id must not panic.
    let id = active_provider_id();
    install_dialect_lookup(Arc::new(DynamicProvider { id }))
        .expect("Fix: Provider install should succeed or be idempotent");
    install_dialect_lookup(Arc::new(DynamicProvider { id }))
        .expect("Fix: Provider install should succeed or be idempotent");
}

#[test]
fn conflicting_provider_id_returns_error() {
    let id = active_provider_id();
    let conflict_id = if id == "test::ProviderB" {
        "test::ProviderA"
    } else {
        "test::ProviderB"
    };
    let err = install_dialect_lookup(Arc::new(DynamicProvider { id: conflict_id }))
        .expect_err("Fix: conflicting provider id must return an error");
    assert!(err.contains("dialect lookup already installed by provider"));
}

#[test]
fn conflicting_provider_id_error_has_actionable_hint() {
    // Adversarial: the error message MUST contain a Fix: hint so the
    // developer knows how to resolve the conflict. This test will fail
    // if the message is ever stripped or shortened.
    let id = active_provider_id();
    let conflict_id = if id == "test::ProviderB" {
        "test::ProviderA"
    } else {
        "test::ProviderB"
    };
    let err = install_dialect_lookup(Arc::new(DynamicProvider { id: conflict_id }))
        .expect_err("Fix: conflicting provider id must return an error");
    assert!(err.contains("Fix:"));
}

#[test]
fn concurrent_race_different_id_is_deterministic() {
    // Adversarial: two threads race install_dialect_lookup with DIFFERENT
    // provider ids. Exactly one must win; the other must return an error. The
    // winning provider_id must be readable and deterministic (no torn
    // state where the global is half-installed).
    use std::sync::Barrier;
    use std::thread;

    // If a provider is already installed from an earlier test, we can
    // only assert determinism, not the empty->set race.
    if let Some(existing) = dialect_lookup() {
        let id = existing.provider_id();
        // If it's one of our racers, the race already happened.
        assert!(
            id == "test::ProviderA"
                || id == "test::ProviderB"
                || id == "test::ProviderC"
                || id == "test::ProviderD"
                || id.is_empty(),
            "Fix: unexpected pre-installed provider {id}"
        );
        return;
    }

    let barrier = Arc::new(Barrier::new(2));

    let b1 = Arc::clone(&barrier);
    let t1 = thread::spawn(move || {
        b1.wait();
        install_dialect_lookup(Arc::new(DynamicProvider {
            id: "test::ProviderA",
        }))
    });

    let b2 = Arc::clone(&barrier);
    let t2 = thread::spawn(move || {
        b2.wait();
        install_dialect_lookup(Arc::new(DynamicProvider {
            id: "test::ProviderB",
        }))
    });

    let r1 = t1.join().expect("Fix: install thread must not panic");
    let r2 = t2.join().expect("Fix: install thread must not panic");
    let ok_count = usize::from(r1.is_ok()) + usize::from(r2.is_ok());
    let err_count = usize::from(r1.is_err()) + usize::from(r2.is_err());

    // Exactly one thread succeeds; the other returns an error because the ids differ.
    assert!(
        ok_count == 1 && err_count == 1,
        "Fix: concurrent install with different ids must have exactly one winner and one error; got ok={ok_count} err={err_count}"
    );

    let winner =
        dialect_lookup().expect("Fix: after a race, the winning provider must be installed");
    let winner_id = winner.provider_id();
    assert!(
        winner_id == "test::ProviderA" || winner_id == "test::ProviderB",
        "Fix: winner must be one of the raced providers, got {winner_id}"
    );
}

#[test]
fn concurrent_race_8_threads_4_ids_has_exactly_one_winner() {
    use std::sync::Barrier;
    use std::thread;

    // Skip if the global already carries a winner  -  the 2-thread test
    // above may have claimed it. Determinism assertion still applies.
    if let Some(existing) = dialect_lookup() {
        let id = existing.provider_id();
        assert!(
            matches!(
                id,
                "test::ProviderA" | "test::ProviderB" | "test::ProviderC" | "test::ProviderD" | ""
            ),
            "Fix: unexpected pre-installed provider {id}"
        );
        return;
    }

    const THREADS: usize = 8;
    let barrier = Arc::new(Barrier::new(THREADS));
    let mut handles = Vec::with_capacity(THREADS);

    for i in 0..THREADS {
        let b = Arc::clone(&barrier);
        let h = thread::spawn(move || {
            b.wait();
            // Rotate through 4 provider ids (A, B, C, D) so the race
            // has a realistic mix of equal-id and different-id pairs.
            match i % 4 {
                0 => install_dialect_lookup(Arc::new(DynamicProvider {
                    id: "test::ProviderA",
                })),
                1 => install_dialect_lookup(Arc::new(DynamicProvider {
                    id: "test::ProviderB",
                })),
                2 => install_dialect_lookup(Arc::new(DynamicProvider {
                    id: "test::ProviderC",
                })),
                _ => install_dialect_lookup(Arc::new(DynamicProvider {
                    id: "test::ProviderD",
                })),
            }
        });
        handles.push(h);
    }

    let mut ok_count = 0;
    let mut err_count = 0;
    for h in handles {
        let result = h.join().expect("Fix: install thread must not panic");
        if result.is_ok() {
            ok_count += 1;
        } else {
            err_count += 1;
        }
    }

    // With 4 distinct ids and 8 threads (2 threads per id), the winner's
    // id is shared by 2 threads. Both may succeed (same id = idempotent
    // no-op). The other 6 threads (3 groups of 2, each with a different
    // id) must return errors.
    //
    // Valid outcomes:
    // - 2 oks + 6 errors (winner id had 2 racers, both succeeded)
    // - 1 ok + 7 errors (winner id had only 1 racer hit the set, rest raced through conflict)
    //
    // The invariant: at least 1 ok, at most 2 oks; rest return errors.
    assert!(
        (1..=2).contains(&ok_count),
        "Fix: with 4 ids × 2 threads/id, expected 1 or 2 oks, got {ok_count} oks + {err_count} errors"
    );
    assert_eq!(
        ok_count + err_count,
        THREADS,
        "Fix: every thread must terminate (ok or error), got ok={ok_count} err={err_count}"
    );

    // The winning provider must be readable and match one of the 4 ids.
    let winner =
        dialect_lookup().expect("Fix: after the race, the winning provider must be installed");
    let winner_id = winner.provider_id();
    assert!(
        matches!(
            winner_id,
            "test::ProviderA" | "test::ProviderB" | "test::ProviderC" | "test::ProviderD"
        ),
        "Fix: winner must be one of the 4 raced providers, got {winner_id}"
    );
}

#[test]
fn empty_string_provider_id_is_not_special_cased() {
    // Adversarial: an empty-string provider_id must be treated exactly like
    // any other id  -  not rejected, not skipped, not defaulted. If it is
    // already installed, idempotent reinstall works. If a non-empty id is
    // installed, the empty id is treated as a conflict.
    if dialect_lookup().is_none() {
        match install_dialect_lookup(Arc::new(DynamicProvider { id: "" })) {
            Ok(()) => {}
            Err(error) => {
                assert!(
                    error.contains("Fix:"),
                    "Fix: empty provider install lost a race to a non-empty provider but returned a non-actionable error: {error}"
                );
                assert!(
                    dialect_lookup()
                        .map(|lookup| !lookup.provider_id().is_empty())
                        .unwrap_or(false),
                    "Fix: empty provider install may only fail if a non-empty provider won the race"
                );
                return;
            }
        }
    }

    // If EmptyIdProvider is the current winner, reinstalling it is fine.
    match dialect_lookup().map(|l| l.provider_id()).unwrap_or("x") {
        "" => install_dialect_lookup(Arc::new(DynamicProvider { id: "" }))
            .expect("Fix: empty provider reinstall should be idempotent"),
        _ => {
            let error = install_dialect_lookup(Arc::new(DynamicProvider { id: "" }))
                .expect_err("Fix: empty provider id must conflict with a non-empty winner");
            assert!(error.contains("Fix:"));
        }
    }
}

#[test]
fn empty_string_provider_id_conflicts_with_nonempty() {
    // Adversarial: empty-string id is not a wildcard; installing it after
    // a non-empty id must return the same Fix: hint as any conflict.
    let id = active_provider_id();
    if id.is_empty() {
        let err = install_dialect_lookup(Arc::new(DynamicProvider {
            id: "test::ProviderA",
        }))
        .expect_err("Fix: non-empty provider id must conflict with empty");
        assert!(err.contains("Fix:"));
    } else {
        let err = install_dialect_lookup(Arc::new(DynamicProvider { id: "" }))
            .expect_err("Fix: empty provider id must conflict with non-empty");
        assert!(err.contains("Fix:"));
    }
}

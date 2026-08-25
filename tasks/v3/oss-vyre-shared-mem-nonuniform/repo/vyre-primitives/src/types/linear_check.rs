//! Linear-type discipline checker primitive (P-PRIM-14).
//!
//! Given a use-count and a declared discipline, the primitive answers
//! "does the use-count satisfy the discipline?". The four disciplines
//! (Linear, Affine, Relevant, Unrestricted) cover the standard
//! substructural type system used by `vyre-foundation::validate` to
//! reject ill-typed programs before lowering.
//!
//! Pure scalar primitive  -  no allocation, no IR dependency. The
//! foundation's checker walks the program counting uses; this primitive
//! is the single decision per buffer.

/// Substructural-type discipline applied to one buffer or value.
///
/// Mirrors `vyre_foundation::ir::LinearType` but lives at the
/// primitive layer so external crates (the type-checker pass, future
/// effect-system frontends, external analyzer rule lowering) can refer to the
/// discipline without pulling in the IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LinearDiscipline {
    /// Exactly one use. `uses == 1` is the only legal count.
    Linear,
    /// At most one use. `uses <= 1`.
    Affine,
    /// At least one use. `uses >= 1`.
    Relevant,
    /// No discipline. Any `uses` count is permitted.
    Unrestricted,
}

impl LinearDiscipline {
    /// Whether this discipline forbids dropping a buffer without using
    /// it (`Linear` or `Relevant`).
    #[must_use]
    #[inline]
    pub const fn forbids_drop(self) -> bool {
        matches!(self, Self::Linear | Self::Relevant)
    }

    /// Whether this discipline forbids using a buffer more than once
    /// (`Linear` or `Affine`).
    #[must_use]
    #[inline]
    pub const fn forbids_reuse(self) -> bool {
        matches!(self, Self::Linear | Self::Affine)
    }
}

/// Why a use-count failed its discipline check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearTypeError {
    /// Linear or Relevant discipline was declared but the count is 0.
    Dropped {
        /// The discipline that forbids drop.
        discipline: LinearDiscipline,
    },
    /// Linear or Affine discipline was declared but the count is > 1.
    Reused {
        /// The discipline that forbids reuse.
        discipline: LinearDiscipline,
        /// The actual observed use count.
        uses: u32,
    },
}

/// Verify that `uses` satisfies the declared `discipline`.
///
/// Returns `Ok(())` when the count is acceptable, otherwise returns
/// the precise discipline-violation reason. Pure const fn  -  single
/// pattern match on the discipline plus one or two comparisons.
///
/// # Errors
///
/// Returns [`LinearTypeError::Dropped`] when a Linear/Relevant buffer
/// has 0 uses, and [`LinearTypeError::Reused`] when a Linear/Affine
/// buffer has > 1 uses.
pub const fn check_linear_use(
    discipline: LinearDiscipline,
    uses: u32,
) -> Result<(), LinearTypeError> {
    match discipline {
        LinearDiscipline::Linear => {
            if uses == 0 {
                Err(LinearTypeError::Dropped { discipline })
            } else if uses > 1 {
                Err(LinearTypeError::Reused { discipline, uses })
            } else {
                Ok(())
            }
        }
        LinearDiscipline::Affine => {
            if uses > 1 {
                Err(LinearTypeError::Reused { discipline, uses })
            } else {
                Ok(())
            }
        }
        LinearDiscipline::Relevant => {
            if uses == 0 {
                Err(LinearTypeError::Dropped { discipline })
            } else {
                Ok(())
            }
        }
        LinearDiscipline::Unrestricted => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_one_use_is_ok() {
        assert_eq!(check_linear_use(LinearDiscipline::Linear, 1), Ok(()));
    }

    #[test]
    fn linear_zero_uses_is_dropped() {
        let err = check_linear_use(LinearDiscipline::Linear, 0).unwrap_err();
        assert_eq!(
            err,
            LinearTypeError::Dropped {
                discipline: LinearDiscipline::Linear
            }
        );
    }

    #[test]
    fn linear_two_uses_is_reused() {
        let err = check_linear_use(LinearDiscipline::Linear, 2).unwrap_err();
        assert_eq!(
            err,
            LinearTypeError::Reused {
                discipline: LinearDiscipline::Linear,
                uses: 2
            }
        );
    }

    #[test]
    fn affine_zero_or_one_use_is_ok() {
        assert_eq!(check_linear_use(LinearDiscipline::Affine, 0), Ok(()));
        assert_eq!(check_linear_use(LinearDiscipline::Affine, 1), Ok(()));
    }

    #[test]
    fn affine_multi_use_is_reused() {
        let err = check_linear_use(LinearDiscipline::Affine, 3).unwrap_err();
        assert!(matches!(err, LinearTypeError::Reused { uses: 3, .. }));
    }

    #[test]
    fn relevant_zero_uses_is_dropped() {
        let err = check_linear_use(LinearDiscipline::Relevant, 0).unwrap_err();
        assert!(matches!(err, LinearTypeError::Dropped { .. }));
    }

    #[test]
    fn relevant_any_nonzero_use_is_ok() {
        assert_eq!(check_linear_use(LinearDiscipline::Relevant, 1), Ok(()));
        assert_eq!(check_linear_use(LinearDiscipline::Relevant, 5), Ok(()));
        assert_eq!(
            check_linear_use(LinearDiscipline::Relevant, u32::MAX),
            Ok(())
        );
    }

    #[test]
    fn unrestricted_accepts_anything() {
        for uses in [0u32, 1, 2, 100, u32::MAX] {
            assert_eq!(
                check_linear_use(LinearDiscipline::Unrestricted, uses),
                Ok(())
            );
        }
    }

    /// Closure-bar: forbids_drop / forbids_reuse helpers must agree
    /// with the actual check_linear_use behavior at the boundary
    /// counts (0 and 2).
    #[test]
    fn helpers_agree_with_checker_at_boundaries() {
        for d in [
            LinearDiscipline::Linear,
            LinearDiscipline::Affine,
            LinearDiscipline::Relevant,
            LinearDiscipline::Unrestricted,
        ] {
            // forbids_drop ⇔ Err on uses=0
            assert_eq!(
                d.forbids_drop(),
                check_linear_use(d, 0).is_err(),
                "forbids_drop disagrees with checker at uses=0 for {:?}",
                d
            );
            // forbids_reuse ⇔ Err on uses=2
            assert_eq!(
                d.forbids_reuse(),
                check_linear_use(d, 2).is_err(),
                "forbids_reuse disagrees with checker at uses=2 for {:?}",
                d
            );
        }
    }
}

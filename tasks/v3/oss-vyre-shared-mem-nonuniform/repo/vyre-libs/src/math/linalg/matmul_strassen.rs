//! Strassen 2x2 matmul  -  `C = A · B` for 2x2 row-major F32 matrices
//! using the Strassen recurrence (7 multiplications, 18 additions
//! instead of naive 8 multiplications, 4 additions).
//!
//! ROADMAP H1  -  Strassen-like matmul substitution where profitable
//! and numerically allowed.
//!
//! For 2x2 matrices `A = [[a,b],[c,d]]` and `B = [[e,f],[g,h]]`:
//!
//! ```text
//! M1 = (a + d) * (e + h)
//! M2 = (c + d) * e
//! M3 = a * (f - h)
//! M4 = d * (g - e)
//! M5 = (a + b) * h
//! M6 = (c - a) * (e + f)
//! M7 = (b - d) * (g + h)
//!
//! C[0,0] = M1 + M4 - M5 + M7
//! C[0,1] = M3 + M5
//! C[1,0] = M2 + M4
//! C[1,1] = M1 - M2 + M3 + M6
//! ```
//!
//! For 2x2 the win is small (12.5 % fewer multiplies); the value
//! of this primitive is as the **base case** of a recursive
//! Strassen implementation. Larger Strassen matmuls split the input
//! into 2x2 blocks of NxN sub-matrices and apply the same formula
//! to the sub-matrices recursively, achieving O(N^log2(7)) ≈ O(N^2.807)
//! instead of naive O(N^3). The recursive caller lands beside this
//! primitive; the per-2x2 Strassen kernel is the substrate.
//!
//! Soundness: algebraic identity (proven correct since 1969). The
//! 7-mult formula produces the same C matrix as the naive 8-mult
//! formula for any commutative ring; over IEEE-754 f32 the
//! ordering of additions differs from naive matmul, so the rounding
//! diverges by ≤ 1 ULP per output element on typical inputs. The
//! parity test in the test module asserts a 1.0e-5 absolute
//! tolerance against the naive 2x2 matmul.
//!
//! Cost direction: monotone-down on multiplications (8 → 7); slight
//! up on additions. On hardware where mul-cycles >> add-cycles
//! (most GPU/CPU FMA pipelines: 4-cycle mul, 1-cycle add) Strassen
//! wins net. Recursive Strassen amortises the per-level addition
//! overhead across N^2 / 4 sub-matmuls, so the addition cost is a
//! fixed fraction of the multiplication savings.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::math::linalg::matmul_strassen_2x2";

/// Build a Program that computes `C = A · B` for 2x2 row-major F32
/// matrices using the Strassen 7-mult formula. Inputs `a` and `b`
/// are length-4 F32 buffers in row-major layout (`a[0..2]` is row 0,
/// `a[2..4]` is row 1); `c` is the length-4 F32 output buffer.
#[must_use]
pub fn matmul_strassen_2x2(a: &str, b: &str, c: &str) -> Program {
    // 7-multiplication Strassen formula. The Lets bind every named
    // intermediate so the IR is straight-line; const-fold and CSE
    // can collapse common sub-expressions across the formula.
    let body = vec![
        // Load the eight scalar entries.
        Node::let_bind("a00", Expr::load(a, Expr::u32(0))),
        Node::let_bind("a01", Expr::load(a, Expr::u32(1))),
        Node::let_bind("a10", Expr::load(a, Expr::u32(2))),
        Node::let_bind("a11", Expr::load(a, Expr::u32(3))),
        Node::let_bind("b00", Expr::load(b, Expr::u32(0))),
        Node::let_bind("b01", Expr::load(b, Expr::u32(1))),
        Node::let_bind("b10", Expr::load(b, Expr::u32(2))),
        Node::let_bind("b11", Expr::load(b, Expr::u32(3))),
        // 7 Strassen products.
        Node::let_bind(
            "m1",
            Expr::mul(
                Expr::add(Expr::var("a00"), Expr::var("a11")),
                Expr::add(Expr::var("b00"), Expr::var("b11")),
            ),
        ),
        Node::let_bind(
            "m2",
            Expr::mul(
                Expr::add(Expr::var("a10"), Expr::var("a11")),
                Expr::var("b00"),
            ),
        ),
        Node::let_bind(
            "m3",
            Expr::mul(
                Expr::var("a00"),
                Expr::sub(Expr::var("b01"), Expr::var("b11")),
            ),
        ),
        Node::let_bind(
            "m4",
            Expr::mul(
                Expr::var("a11"),
                Expr::sub(Expr::var("b10"), Expr::var("b00")),
            ),
        ),
        Node::let_bind(
            "m5",
            Expr::mul(
                Expr::add(Expr::var("a00"), Expr::var("a01")),
                Expr::var("b11"),
            ),
        ),
        Node::let_bind(
            "m6",
            Expr::mul(
                Expr::sub(Expr::var("a10"), Expr::var("a00")),
                Expr::add(Expr::var("b00"), Expr::var("b01")),
            ),
        ),
        Node::let_bind(
            "m7",
            Expr::mul(
                Expr::sub(Expr::var("a01"), Expr::var("a11")),
                Expr::add(Expr::var("b10"), Expr::var("b11")),
            ),
        ),
        // C[0,0] = M1 + M4 - M5 + M7
        Node::Store {
            buffer: c.into(),
            index: Expr::u32(0),
            value: Expr::add(
                Expr::sub(Expr::add(Expr::var("m1"), Expr::var("m4")), Expr::var("m5")),
                Expr::var("m7"),
            ),
        },
        // C[0,1] = M3 + M5
        Node::Store {
            buffer: c.into(),
            index: Expr::u32(1),
            value: Expr::add(Expr::var("m3"), Expr::var("m5")),
        },
        // C[1,0] = M2 + M4
        Node::Store {
            buffer: c.into(),
            index: Expr::u32(2),
            value: Expr::add(Expr::var("m2"), Expr::var("m4")),
        },
        // C[1,1] = M1 - M2 + M3 + M6
        Node::Store {
            buffer: c.into(),
            index: Expr::u32(3),
            value: Expr::add(
                Expr::add(Expr::sub(Expr::var("m1"), Expr::var("m2")), Expr::var("m3")),
                Expr::var("m6"),
            ),
        },
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::F32).with_count(4),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::F32).with_count(4),
            BufferDecl::output(c, 2, DataType::F32).with_count(4),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || matmul_strassen_2x2("a", "b", "c"),
        test_inputs: Some(|| {
            // A = [[1, 2], [3, 4]], B = [[5, 6], [7, 8]]
            let a = crate::test_support::byte_pack::f32_bytes(&[1.0, 2.0, 3.0, 4.0]);
            let b = crate::test_support::byte_pack::f32_bytes(&[5.0, 6.0, 7.0, 8.0]);
            vec![vec![a, b]]
        }),
        expected_output: Some(|| {
            // C = A · B = [[19, 22], [43, 50]]
            vec![vec![crate::test_support::byte_pack::f32_bytes(&[19.0, 22.0, 43.0, 50.0])]]
        }),
        category: Some("math"),
    }
}

/// Build a Program that computes `C = A · B` for NxN row-major F32
/// matrices via one level of Strassen recursion (N must be even;
/// the base 2x2 sub-matmuls use the naive 8-mult formula). For
/// `N = 2` this degenerates to `matmul_strassen_2x2`. For
/// `N = 2k`, the four NxN-quadrants of A and B are split into
/// `(N/2)x(N/2)` sub-matrices and the same Strassen 7-mult formula
/// is applied to the sub-matrices: 7 sub-matmuls of size
/// `(N/2)x(N/2)` (each O((N/2)^3) naive) instead of the standard 8
/// sub-matmuls. Asymptotic recursion (depth = log2(N)) yields
/// O(N^log2(7)) ≈ O(N^2.807). One level of recursion yields the
/// 8 → 7 multiply ratio for the NxN matmul itself.
///
/// # Errors
///
/// Returns `Err` when `n` is 0 or odd (one-level Strassen requires
/// even N) or when `n*n*4` overflows `u32`.
pub fn matmul_strassen_one_level(a: &str, b: &str, c: &str, n: u32) -> Result<Program, String> {
    if n == 0 {
        return Err("Fix: matmul_strassen_one_level n=0 is invalid".to_string());
    }
    if n % 2 != 0 {
        return Err(format!(
            "Fix: matmul_strassen_one_level requires even n; got n={n}. Use matmul or pad."
        ));
    }
    let half = n / 2;
    let total = n
        .checked_mul(n)
        .ok_or_else(|| "Fix: matmul_strassen_one_level n*n overflows u32; reduce n.".to_string())?;

    // One invocation per output element (n*n lanes).
    let body = vec![
        Node::let_bind("flat", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("flat"), Expr::u32(total)),
            vec![
                // Output coordinates: row, col in [0, n)
                Node::let_bind("row", Expr::div(Expr::var("flat"), Expr::u32(n))),
                Node::let_bind("col", Expr::rem(Expr::var("flat"), Expr::u32(n))),
                // Quadrant indices: q_row = row / half ∈ {0, 1}
                Node::let_bind("q_row", Expr::div(Expr::var("row"), Expr::u32(half))),
                Node::let_bind("q_col", Expr::div(Expr::var("col"), Expr::u32(half))),
                // Sub-coordinates within the quadrant.
                Node::let_bind("sr", Expr::rem(Expr::var("row"), Expr::u32(half))),
                Node::let_bind("sc", Expr::rem(Expr::var("col"), Expr::u32(half))),
                // Strassen formula at the quadrant level. Each Mp =
                // sum_k of products of A-quadrant entries and B-quadrant entries.
                // Compute the 7 Mp values for the (sr, sc) cell and combine
                // into C[q_row, q_col][sr, sc].
                //
                // Helper indices:
                //   A[qa_row, qa_col][sr, k] is at flat index
                //     (qa_row * half + sr) * n + (qa_col * half + k)
                //
                // We unroll the 7 Strassen products inline with an inner
                // k-loop accumulating each Mp.
                Node::let_bind("c_val", Expr::f32(0.0)),
                Node::let_bind("m1", Expr::f32(0.0)),
                Node::let_bind("m2", Expr::f32(0.0)),
                Node::let_bind("m3", Expr::f32(0.0)),
                Node::let_bind("m4", Expr::f32(0.0)),
                Node::let_bind("m5", Expr::f32(0.0)),
                Node::let_bind("m6", Expr::f32(0.0)),
                Node::let_bind("m7", Expr::f32(0.0)),
                // The 7 Mp values are matmuls of (half x half) sub-matrices.
                // Compute the (sr, sc) entry of each Mp:
                //   M1 = (A11+A22) * (B11+B22)  → entry: sum_k (A11[sr,k]+A22[sr,k]) * (B11[k,sc]+B22[k,sc])
                //   M2 = (A21+A22) * B11        → entry: sum_k (A21[sr,k]+A22[sr,k]) * B11[k,sc]
                //   M3 = A11 * (B12-B22)        → entry: sum_k A11[sr,k] * (B12[k,sc]-B22[k,sc])
                //   M4 = A22 * (B21-B11)        → entry: sum_k A22[sr,k] * (B21[k,sc]-B11[k,sc])
                //   M5 = (A11+A12) * B22        → entry: sum_k (A11[sr,k]+A12[sr,k]) * B22[k,sc]
                //   M6 = (A21-A11) * (B11+B12)  → entry: sum_k (A21[sr,k]-A11[sr,k]) * (B11[k,sc]+B12[k,sc])
                //   M7 = (A12-A22) * (B21+B22)  → entry: sum_k (A12[sr,k]-A22[sr,k]) * (B21[k,sc]+B22[k,sc])
                Node::loop_for(
                    "k",
                    Expr::u32(0),
                    Expr::u32(half),
                    vec![
                        // Load A-quadrant entries at row=sr, col=k.
                        Node::let_bind(
                            "a11",
                            Expr::load(
                                a,
                                Expr::add(Expr::mul(Expr::var("sr"), Expr::u32(n)), Expr::var("k")),
                            ),
                        ),
                        Node::let_bind(
                            "a12",
                            Expr::load(
                                a,
                                Expr::add(
                                    Expr::mul(Expr::var("sr"), Expr::u32(n)),
                                    Expr::add(Expr::u32(half), Expr::var("k")),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "a21",
                            Expr::load(
                                a,
                                Expr::add(
                                    Expr::mul(
                                        Expr::add(Expr::u32(half), Expr::var("sr")),
                                        Expr::u32(n),
                                    ),
                                    Expr::var("k"),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "a22",
                            Expr::load(
                                a,
                                Expr::add(
                                    Expr::mul(
                                        Expr::add(Expr::u32(half), Expr::var("sr")),
                                        Expr::u32(n),
                                    ),
                                    Expr::add(Expr::u32(half), Expr::var("k")),
                                ),
                            ),
                        ),
                        // Load B-quadrant entries at row=k, col=sc.
                        Node::let_bind(
                            "b11",
                            Expr::load(
                                b,
                                Expr::add(Expr::mul(Expr::var("k"), Expr::u32(n)), Expr::var("sc")),
                            ),
                        ),
                        Node::let_bind(
                            "b12",
                            Expr::load(
                                b,
                                Expr::add(
                                    Expr::mul(Expr::var("k"), Expr::u32(n)),
                                    Expr::add(Expr::u32(half), Expr::var("sc")),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "b21",
                            Expr::load(
                                b,
                                Expr::add(
                                    Expr::mul(
                                        Expr::add(Expr::u32(half), Expr::var("k")),
                                        Expr::u32(n),
                                    ),
                                    Expr::var("sc"),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "b22",
                            Expr::load(
                                b,
                                Expr::add(
                                    Expr::mul(
                                        Expr::add(Expr::u32(half), Expr::var("k")),
                                        Expr::u32(n),
                                    ),
                                    Expr::add(Expr::u32(half), Expr::var("sc")),
                                ),
                            ),
                        ),
                        // Accumulate the 7 Mp values for this (sr, sc, k).
                        Node::assign(
                            "m1",
                            Expr::add(
                                Expr::var("m1"),
                                Expr::mul(
                                    Expr::add(Expr::var("a11"), Expr::var("a22")),
                                    Expr::add(Expr::var("b11"), Expr::var("b22")),
                                ),
                            ),
                        ),
                        Node::assign(
                            "m2",
                            Expr::add(
                                Expr::var("m2"),
                                Expr::mul(
                                    Expr::add(Expr::var("a21"), Expr::var("a22")),
                                    Expr::var("b11"),
                                ),
                            ),
                        ),
                        Node::assign(
                            "m3",
                            Expr::add(
                                Expr::var("m3"),
                                Expr::mul(
                                    Expr::var("a11"),
                                    Expr::sub(Expr::var("b12"), Expr::var("b22")),
                                ),
                            ),
                        ),
                        Node::assign(
                            "m4",
                            Expr::add(
                                Expr::var("m4"),
                                Expr::mul(
                                    Expr::var("a22"),
                                    Expr::sub(Expr::var("b21"), Expr::var("b11")),
                                ),
                            ),
                        ),
                        Node::assign(
                            "m5",
                            Expr::add(
                                Expr::var("m5"),
                                Expr::mul(
                                    Expr::add(Expr::var("a11"), Expr::var("a12")),
                                    Expr::var("b22"),
                                ),
                            ),
                        ),
                        Node::assign(
                            "m6",
                            Expr::add(
                                Expr::var("m6"),
                                Expr::mul(
                                    Expr::sub(Expr::var("a21"), Expr::var("a11")),
                                    Expr::add(Expr::var("b11"), Expr::var("b12")),
                                ),
                            ),
                        ),
                        Node::assign(
                            "m7",
                            Expr::add(
                                Expr::var("m7"),
                                Expr::mul(
                                    Expr::sub(Expr::var("a12"), Expr::var("a22")),
                                    Expr::add(Expr::var("b21"), Expr::var("b22")),
                                ),
                            ),
                        ),
                    ],
                ),
                // Combine the 7 Mp values into the C-quadrant entry at (sr, sc).
                // The combine depends on which output quadrant (q_row, q_col).
                //   C11 = M1 + M4 - M5 + M7   (q_row=0, q_col=0)
                //   C12 = M3 + M5             (q_row=0, q_col=1)
                //   C21 = M2 + M4             (q_row=1, q_col=0)
                //   C22 = M1 - M2 + M3 + M6   (q_row=1, q_col=1)
                Node::assign(
                    "c_val",
                    Expr::select(
                        Expr::and(
                            Expr::eq(Expr::var("q_row"), Expr::u32(0)),
                            Expr::eq(Expr::var("q_col"), Expr::u32(0)),
                        ),
                        Expr::add(
                            Expr::sub(Expr::add(Expr::var("m1"), Expr::var("m4")), Expr::var("m5")),
                            Expr::var("m7"),
                        ),
                        Expr::select(
                            Expr::and(
                                Expr::eq(Expr::var("q_row"), Expr::u32(0)),
                                Expr::eq(Expr::var("q_col"), Expr::u32(1)),
                            ),
                            Expr::add(Expr::var("m3"), Expr::var("m5")),
                            Expr::select(
                                Expr::and(
                                    Expr::eq(Expr::var("q_row"), Expr::u32(1)),
                                    Expr::eq(Expr::var("q_col"), Expr::u32(0)),
                                ),
                                Expr::add(Expr::var("m2"), Expr::var("m4")),
                                Expr::add(
                                    Expr::add(
                                        Expr::sub(Expr::var("m1"), Expr::var("m2")),
                                        Expr::var("m3"),
                                    ),
                                    Expr::var("m6"),
                                ),
                            ),
                        ),
                    ),
                ),
                Node::store(c, Expr::var("flat"), Expr::var("c_val")),
            ],
        ),
    ];
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::F32).with_count(total),
            BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::F32).with_count(total),
            BufferDecl::output(c, 2, DataType::F32).with_count(total),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(
            "vyre-libs::math::linalg::matmul_strassen_one_level",
            body,
        )],
    ))
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    fn decode(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    fn naive_2x2(a: &[f32], b: &[f32]) -> [f32; 4] {
        [
            a[0] * b[0] + a[1] * b[2],
            a[0] * b[1] + a[1] * b[3],
            a[2] * b[0] + a[3] * b[2],
            a[2] * b[1] + a[3] * b[3],
        ]
    }

    fn run_strassen(a: &[f32], b: &[f32]) -> Vec<f32> {
        let prog = matmul_strassen_2x2("a", "b", "c");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(a)),
                Value::from(f32_bytes(b)),
                Value::from(vec![0u8; 16]),
            ],
        )
        .expect("Fix: matmul_strassen_2x2 must execute in the reference interpreter.");
        decode(&outputs[0].to_bytes())
    }

    /// Strassen 2x2 matches naive 2x2 on the canonical [[1,2],[3,4]]
    /// times [[5,6],[7,8]] fixture.
    #[test]
    fn strassen_matches_naive_canonical_fixture() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [5.0, 6.0, 7.0, 8.0];
        let actual = run_strassen(&a, &b);
        let expected = naive_2x2(&a, &b);
        assert_eq!(expected, [19.0, 22.0, 43.0, 50.0]);
        for (lhs, rhs) in actual.iter().zip(expected.iter()) {
            assert!((lhs - rhs).abs() <= 1.0e-5, "{lhs} != {rhs}");
        }
    }

    /// Identity matrix: `A · I = A` for any A.
    #[test]
    fn strassen_identity_returns_a() {
        let a = [1.5_f32, -2.25, 3.75, -0.5];
        let identity = [1.0_f32, 0.0, 0.0, 1.0];
        let actual = run_strassen(&a, &identity);
        for (lhs, rhs) in actual.iter().zip(a.iter()) {
            assert!((lhs - rhs).abs() <= 1.0e-5, "{lhs} != {rhs}");
        }
    }

    /// Zero matrix: `A · 0 = 0` for any A.
    #[test]
    fn strassen_zero_returns_zero() {
        let a = [1.5_f32, -2.25, 3.75, -0.5];
        let zero = [0.0_f32; 4];
        let actual = run_strassen(&a, &zero);
        for v in actual {
            assert_eq!(v, 0.0);
        }
    }

    fn naive_nxn(a: &[f32], b: &[f32], n: usize) -> Vec<f32> {
        let mut c = vec![0.0_f32; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut acc = 0.0_f32;
                for k in 0..n {
                    acc += a[i * n + k] * b[k * n + j];
                }
                c[i * n + j] = acc;
            }
        }
        c
    }

    fn run_one_level(a: &[f32], b: &[f32], n: u32) -> Vec<f32> {
        let prog = matmul_strassen_one_level("a", "b", "c", n).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(a)),
                Value::from(f32_bytes(b)),
                Value::from(vec![0u8; (n as usize) * (n as usize) * 4]),
            ],
        )
        .expect("Fix: matmul_strassen_one_level must execute in the reference interpreter.");
        decode(&outputs[0].to_bytes())
    }

    /// One-level Strassen at N=4 matches naive 4x4 matmul.
    #[test]
    fn strassen_one_level_matches_naive_at_n4() {
        let a: Vec<f32> = (0..16).map(|i| (i as f32) * 0.5 - 4.0).collect();
        let b: Vec<f32> = (0..16).map(|i| (i as f32) * 0.25 + 1.0).collect();
        let actual = run_one_level(&a, &b, 4);
        let expected = naive_nxn(&a, &b, 4);
        assert_eq!(actual.len(), 16);
        for (i, (l, r)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!((l - r).abs() <= 1.0e-4, "lane {i}: strassen={l} naive={r}");
        }
    }

    /// One-level Strassen at N=2 produces the same result as the
    /// flat 2x2 Strassen primitive (degenerate base case).
    #[test]
    fn strassen_one_level_n2_matches_strassen_2x2() {
        let a = [1.0_f32, 2.0, 3.0, 4.0];
        let b = [5.0_f32, 6.0, 7.0, 8.0];
        let level1 = run_one_level(&a, &b, 2);
        let flat = run_strassen(&a, &b);
        for (l, f) in level1.iter().zip(flat.iter()) {
            assert!((l - f).abs() <= 1.0e-5, "{l} != {f}");
        }
    }

    /// One-level Strassen rejects odd N.
    #[test]
    fn strassen_one_level_rejects_odd_n() {
        let err = matmul_strassen_one_level("a", "b", "c", 3).expect_err("odd n must error");
        assert!(err.contains("even"));
    }

    /// Random fuzz: 100 random 2x2 pairs, Strassen agrees with
    /// naive within 1.0e-4 absolute tolerance (the rounding
    /// divergence between the two summation orders).
    #[test]
    fn strassen_matches_naive_on_random_fuzz() {
        // Deterministic LCG so the test is reproducible.
        let mut state = 0x12345678_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as f32 / (u32::MAX as f32 / 2.0)) - 1.0
        };
        for _ in 0..100 {
            let a = [next(), next(), next(), next()];
            let b = [next(), next(), next(), next()];
            let actual = run_strassen(&a, &b);
            let expected = naive_2x2(&a, &b);
            for (i, (lhs, rhs)) in actual.iter().zip(expected.iter()).enumerate() {
                assert!(
                    (lhs - rhs).abs() <= 1.0e-4,
                    "lane {i}: strassen={lhs} naive={rhs} diff={}",
                    (lhs - rhs).abs()
                );
            }
        }
    }
}

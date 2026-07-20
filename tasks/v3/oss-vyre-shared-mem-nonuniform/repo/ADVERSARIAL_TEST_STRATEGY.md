# Adversarial Test Strategy for Vyre

## Executive Summary

Vyre has excellent test coverage with 1400+ test files and strong adversarial foundations. This strategy targets specific gaps to achieve SQLite/NASA-grade testing quality, focusing on areas where hostile inputs could cause panics, undefined behavior, or silent incorrectness.

## Current Strengths

- **vyre-foundation**: 95 test files with strong wire format adversarial tests (truncation, corruption, mutation)
- **vyre-primitives**: 95+ test files with extensive proptest coverage and GPU parity tests
- **vyre-libs**: 95+ test files with C parser focus and GPU roundtrip tests
- **vyre-driver-wgpu**: 140+ test files with C AST semantic contracts and capability tests
- **vyre-driver-cuda**: 95+ test files with generated CUDA reference matrices

## Targeted Gaps by Priority

### P0 - Critical Safety & Correctness

#### 1. Memory Model Stress Tests
**Location**: `vyre-foundation/tests/memory_ordering_adversarial.rs` (new)
- Atomic operations with all memory ordering combinations
- Relaxed vs Acquire/Release on same operations
- Sequential consistency stress with multiple threads
- Data race detection with loom
- Subgroup operations with memory ordering edge cases

#### 2. Resource Exhaustion Defense
**Location**: `vyre-foundation/tests/resource_exhaustion_adversarial.rs` (new)
- OOM at every allocation checkpoint (arena, buffer, wire decode)
- Maximum buffer count stress (16,384 buffers)
- Maximum node count stress (100,000 nodes)
- Deep nesting stack safety (10,000 nested blocks)
- Descriptor table exhaustion

#### 3. Type System Boundary Attacks
**Location**: `vyre-foundation/tests/type_boundary_adversarial.rs` (new)
- Invalid cast chains (F32 → Bool → U32 → invalid)
- Vector type coercion edge cases
- Quantized type boundary violations
- Mixed precision arithmetic with NaN/Inf propagation
- Type inference ambiguity attacks

#### 4. Region Chain Integrity
**Location**: `vyre-foundation/tests/region_chain_adversarial.rs` (new)
- Circular region references
- Orphaned regions without source_region
- Malformed generator IDs
- Region depth limit enforcement
- Source region pointer corruption

### P1 - Robustness & Reliability

#### 5. Extension System Hardening
**Location**: `vyre-foundation/tests/extension_adversarial.rs` (new)
- Oversized opaque payloads (at cap + 1 byte)
- Malformed extension resolver rejection
- Extension kind collision attacks
- Opaque payload with null bytes
- Extension resolver panic survival

#### 6. Backend Capability Negotiation
**Location**: `vyre-driver/tests/capability_adversarial.rs` (new)
- Capability query during active dispatch
- Capability change mid-execution
- Missing capability fallback behavior
- Capability limit boundary testing
- Capability string injection attacks

#### 7. Optimizer Pass Edge Cases
**Location**: `vyre-foundation/tests/optimizer_adversarial.rs` (expand existing)
- CSE with identical but semantically different expressions
- DCE with side-effect detection failures
- Constant folding with overflow/underflow
- Loop unrolling with infinite loop potential
- Fusion with aliasing violations

#### 8. Concurrent Access Stress
**Location**: `vyre-driver/tests/concurrent_adversarial.rs` (new)
- Multiple threads acquiring same backend
- Concurrent dispatch with shared programs
- Registry access during op registration
- Pipeline cache concurrent access
- Module cache race conditions

### P2 - Correctness & Completeness

#### 9. Error Path Coverage
**Location**: `vyre-foundation/tests/error_path_adversarial.rs` (new)
- Every error code V### has a proving test
- Error message Fix: field validation
- Error recovery after partial failure
- Error context preservation
- Error code uniqueness

#### 10. Wire Format Fuzzing Integration
**Location**: `vyre-foundation/fuzz/` (expand existing)
- Structured fuzzing for each wire format component
- Differential fuzzing between encode/decode
- Fuzz with AFL++ or libFuzzer integration
- Corpus management for interesting cases
- Fuzz timeout and resource limits

#### 11. GPU Backend Parity Gaps
**Location**: `vyre-driver-wgpu/tests/parity_adversarial.rs` (new)
- CUDA vs WGPU differential on edge cases
- Subgroup operation parity across backends
- Atomic operation ordering differences
- Workgroup size limit differences
- Memory layout differences

#### 12. Parser Hostile Input Expansion
**Location**: `vyre-libs/tests/parser_hostile_extended.rs` (new)
- C parser with malicious macros
- Recursive macro expansion limits
- Preprocessor directive injection
- Comment stripping edge cases
- Unicode and encoding attacks

## Implementation Organization

### File Structure
```
vyre-foundation/tests/
├── memory_ordering_adversarial.rs       # P0.1
├── resource_exhaustion_adversarial.rs   # P0.2
├── type_boundary_adversarial.rs         # P0.3
├── region_chain_adversarial.rs          # P0.4
├── extension_adversarial.rs             # P1.5
└── error_path_adversarial.rs            # P2.9

vyre-driver/tests/
├── capability_adversarial.rs            # P1.6
└── concurrent_adversarial.rs            # P1.8

vyre-driver-wgpu/tests/
└── parity_adversarial.rs                # P2.11

vyre-libs/tests/
└── parser_hostile_extended.rs           # P2.12
```

### Test Categories Per File

Each adversarial test file should include:
1. **Truncation/Corruption**: Malformed inputs that must fail gracefully
2. **Boundary Cases**: Edge values at limits (max, min, zero)
3. **Stress Tests**: Maximum scale inputs (100K nodes, 16K buffers)
4. **Race Conditions**: Concurrent access patterns (with loom where applicable)
5. **Hostile Patterns**: Malicious input sequences designed to trigger bugs
6. **Property Violations**: Tests that would fail if invariants are broken

## Quality Standards

### Per-Test Requirements
- No `assert!(ok)` - must assert specific error codes or conditions
- No mock-self-confirm - tests must fail if implementation is deleted
- Oracle independence - expected values from independent computation
- Falsifiable - removing or dumbing implementation must turn test red
- Documentation - each test explains what vulnerability it prevents

### Test Organization
- Group related tests in modules with clear section headers
- Use descriptive test names that explain the adversarial scenario
- Include comments explaining why this specific input is hostile
- Reference relevant error codes (V###) or architectural invariants

### Integration with Existing Infrastructure
- Use existing test helpers from `common/` directories
- Follow SKILL.md patterns for each crate
- Integrate with existing proptest infrastructure
- Use existing GPU parity test patterns
- Leverage existing wire format test utilities

## Success Metrics

### Coverage Goals
- Every V### error code has at least one adversarial proving test
- Every allocation path has OOM injection test
- Every public API has hostile input test
- Every concurrent access path has race condition test
- Every wire format component has fuzz target

### Quality Gates
- All new adversarial tests pass on main branch
- No regression in existing test suite
- Clippy clean with `-D warnings`
- All tests run with `--test-threads=1` where required
- GPU tests have proper capability gating

## Execution Plan

### Phase 1: P0 Critical Safety (Week 1)
1. Memory ordering adversarial tests
2. Resource exhaustion defense tests
3. Type system boundary tests
4. Region chain integrity tests

### Phase 2: P1 Robustness (Week 2)
1. Extension system hardening
2. Backend capability negotiation
3. Optimizer pass edge cases
4. Concurrent access stress

### Phase 3: P2 Completeness (Week 3)
1. Error path coverage
2. Wire format fuzzing integration
3. GPU backend parity gaps
4. Parser hostile input expansion

### Phase 4: Validation (Week 4)
1. Full test suite execution
2. Performance regression checks
3. Documentation updates
4. CI integration

## Integration with Release Gate

This adversarial test expansion directly supports RELEASE_GATE.md requirements:
- **A.7**: SQLite/NASA/Chromium-grade testing
- **A.4**: Per-op surface complete with hostile-input fuzz
- **D.1**: Micro-flaw sweep with comprehensive coverage
- **E.1**: Gate closure mechanics with zero unchecked boxes

## Maintenance

### Ongoing Requirements
- New ops must include adversarial tests before merge
- New error codes must have proving tests
- New capabilities must have negotiation tests
- Fuzz corpora must be updated with interesting findings
- Test gaps identified during audits must be filed as issues

### Review Process
- Adversarial tests reviewed for quality and completeness
- Falsifiability verified during code review
- Integration with existing patterns enforced
- Documentation clarity checked
- Performance impact assessed

## References

- RELEASE_GATE.md: Section A.7 (SQLite/NASA-grade testing)
- SKILL.md files: Per-crate testing contracts
- docs/ARCHITECTURE.md: Memory model and invariants
- docs/library-tiers.md: Op organization and dependencies
- docs/region-chain.md: Composition chain invariants

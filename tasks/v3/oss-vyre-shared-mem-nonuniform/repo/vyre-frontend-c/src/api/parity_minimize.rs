//! Source-slice minimization support for parity mismatches.
//!
//! The minimizer is deliberately predicate-driven: clang extraction, vyrec
//! extraction, and fact comparison stay outside this module. The reducer only
//! knows how to propose smaller source/header slices and keep the smallest
//! candidate that still preserves the caller's mismatch predicate.

/// One source file in a parity mismatch reproducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParitySourceFile {
    /// Path used by the reproducer.
    pub path: String,
    /// Source text.
    pub text: String,
}

impl ParitySourceFile {
    /// Creates one source file entry.
    #[must_use]
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            text: text.into(),
        }
    }

    /// Returns the number of source lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        split_preserving_lines(&self.text).len()
    }
}

/// A minimized parity mismatch reproducer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityMismatchReproducer {
    /// Source files kept by the reducer.
    pub files: Vec<ParitySourceFile>,
    /// Number of predicate evaluations performed.
    pub predicate_evaluations: usize,
}

/// Configuration for source-slice minimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParityMinimizerConfig {
    /// Smallest line count to keep in any file.
    pub min_lines_per_file: usize,
    /// Maximum predicate evaluations before stopping.
    pub max_predicate_evaluations: usize,
}

impl Default for ParityMinimizerConfig {
    fn default() -> Self {
        Self {
            min_lines_per_file: 1,
            max_predicate_evaluations: 4096,
        }
    }
}

/// Deterministic parity mismatch source-slice minimizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParitySourceMinimizer {
    config: ParityMinimizerConfig,
}

impl ParitySourceMinimizer {
    /// Creates a minimizer with explicit configuration.
    #[must_use]
    pub const fn new(config: ParityMinimizerConfig) -> Self {
        Self { config }
    }

    /// Creates a minimizer with default configuration.
    #[must_use]
    pub const fn default_configured() -> Self {
        Self {
            config: ParityMinimizerConfig {
                min_lines_per_file: 1,
                max_predicate_evaluations: 4096,
            },
        }
    }

    /// Reduces source/header files while `preserves_mismatch` remains true.
    ///
    /// The predicate must return true only when the candidate still reproduces
    /// the same parity mismatch class. The reducer first tries to remove whole
    /// files, then shrinks each remaining file by deleting line chunks with a
    /// deterministic delta-debugging schedule.
    #[must_use]
    pub fn minimize(
        self,
        files: Vec<ParitySourceFile>,
        mut preserves_mismatch: impl FnMut(&[ParitySourceFile]) -> bool,
    ) -> ParityMismatchReproducer {
        let mut evaluations = 0_usize;
        let mut current = files;
        if !evaluate(
            &mut evaluations,
            self.config.max_predicate_evaluations,
            &current,
            &mut preserves_mismatch,
        ) {
            return ParityMismatchReproducer {
                files: current,
                predicate_evaluations: evaluations,
            };
        }

        let mut file_index = 0;
        while file_index < current.len() && evaluations < self.config.max_predicate_evaluations {
            if current.len() > 1 {
                let mut candidate = current.clone();
                candidate.remove(file_index);
                if evaluate(
                    &mut evaluations,
                    self.config.max_predicate_evaluations,
                    &candidate,
                    &mut preserves_mismatch,
                ) {
                    current = candidate;
                    continue;
                }
            }
            file_index += 1;
        }

        for index in 0..current.len() {
            if evaluations >= self.config.max_predicate_evaluations {
                break;
            }
            current[index] = minimize_file_lines(
                &current[index],
                self.config,
                &current,
                index,
                &mut evaluations,
                &mut preserves_mismatch,
            );
        }

        ParityMismatchReproducer {
            files: current,
            predicate_evaluations: evaluations,
        }
    }
}

impl Default for ParitySourceMinimizer {
    fn default() -> Self {
        Self::default_configured()
    }
}

fn minimize_file_lines(
    file: &ParitySourceFile,
    config: ParityMinimizerConfig,
    all_files: &[ParitySourceFile],
    file_index: usize,
    evaluations: &mut usize,
    preserves_mismatch: &mut impl FnMut(&[ParitySourceFile]) -> bool,
) -> ParitySourceFile {
    let mut lines = split_preserving_lines(&file.text);
    let mut chunk = lines.len() / 2;
    while chunk > 0 && *evaluations < config.max_predicate_evaluations {
        let mut start = 0;
        let mut changed = false;
        while start < lines.len() && *evaluations < config.max_predicate_evaluations {
            let end = usize::min(start + chunk, lines.len());
            if lines.len().saturating_sub(end - start) < config.min_lines_per_file {
                start += chunk;
                continue;
            }
            let mut candidate_lines = lines.clone();
            candidate_lines.drain(start..end);
            let mut candidate_files = all_files.to_vec();
            candidate_files[file_index] =
                ParitySourceFile::new(file.path.clone(), candidate_lines.concat());
            if evaluate(
                evaluations,
                config.max_predicate_evaluations,
                &candidate_files,
                preserves_mismatch,
            ) {
                lines = candidate_lines;
                changed = true;
            } else {
                start += chunk;
            }
        }
        if !changed {
            chunk /= 2;
        }
    }
    ParitySourceFile::new(file.path.clone(), lines.concat())
}

fn evaluate(
    evaluations: &mut usize,
    max_evaluations: usize,
    files: &[ParitySourceFile],
    predicate: &mut impl FnMut(&[ParitySourceFile]) -> bool,
) -> bool {
    if *evaluations >= max_evaluations {
        return false;
    }
    *evaluations += 1;
    predicate(files)
}

fn split_preserving_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split_inclusive('\n').map(ToOwned::to_owned).collect()
}

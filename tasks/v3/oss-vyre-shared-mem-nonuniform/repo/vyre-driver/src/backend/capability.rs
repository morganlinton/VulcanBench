//! Backend capability traits.

use super::{BackendError, DispatchConfig, VyreBackend};
use std::collections::HashSet;
use vyre_foundation::ir::OpId;
use vyre_foundation::ir::Program;

/// Borrowed memory supplied to a backend dispatch.
pub type MemoryRef<'a> = &'a [u8];

/// Owned memory returned by a backend dispatch.
pub type Memory = Vec<u8>;

/// Minimal backend identity and capability contract.
pub trait Backend: Send + Sync {
    /// Stable backend identifier.
    fn id(&self) -> &'static str;
    /// Backend implementation version.
    fn version(&self) -> &'static str;
    /// Operation ids this backend can execute without further lowering.
    fn supported_ops(&self) -> &HashSet<OpId>;
}

impl<T: VyreBackend + ?Sized> Backend for T {
    fn id(&self) -> &'static str {
        VyreBackend::id(self)
    }

    fn version(&self) -> &'static str {
        VyreBackend::version(self)
    }

    fn supported_ops(&self) -> &HashSet<OpId> {
        VyreBackend::supported_ops(self)
    }
}

/// Backend capability for direct program execution.
pub trait Executable: Backend {
    /// Dispatch a validated program.
    fn dispatch(
        &self,
        program: &Program,
        inputs: &[MemoryRef<'_>],
        config: &DispatchConfig,
    ) -> Result<Vec<Memory>, BackendError>;
}

/// Backend capability for stream-oriented execution.
pub trait Streamable: Backend {
    /// Dispatch a program over input chunks.
    fn stream(
        &self,
        program: &Program,
        chunks: &mut dyn Iterator<Item = MemoryRef<'_>>,
        config: &DispatchConfig,
    ) -> Result<Box<dyn Iterator<Item = Result<Memory, BackendError>>>, BackendError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use vyre_foundation::ir::Program;

    struct EchoStreamable {
        ops: HashSet<OpId>,
    }

    impl EchoStreamable {
        fn new() -> Self {
            Self {
                ops: HashSet::new(),
            }
        }
    }

    impl Backend for EchoStreamable {
        fn id(&self) -> &'static str {
            "echo-streamable"
        }

        fn version(&self) -> &'static str {
            "0.4.1-test"
        }

        fn supported_ops(&self) -> &HashSet<OpId> {
            &self.ops
        }
    }

    impl Streamable for EchoStreamable {
        fn stream(
            &self,
            _program: &Program,
            chunks: &mut dyn Iterator<Item = MemoryRef<'_>>,
            _config: &DispatchConfig,
        ) -> Result<Box<dyn Iterator<Item = Result<Memory, BackendError>>>, BackendError> {
            let outputs = chunks
                .map(|chunk| Ok(chunk.to_vec()))
                .collect::<Vec<Result<Memory, BackendError>>>();
            Ok(Box::new(outputs.into_iter()))
        }
    }

    #[test]
    fn streamable_is_object_safe() {
        let backend: Box<dyn Streamable> = Box::new(EchoStreamable::new());
        let program = Program::empty();
        let chunks = [b"ab".as_slice(), b"cd".as_slice()];
        let mut iter = chunks.into_iter();
        let outputs = backend
            .stream(&program, &mut iter, &DispatchConfig::default())
            .expect("Fix: object-safe Streamable dispatch must succeed")
            .collect::<Result<Vec<_>, _>>()
            .expect("Fix: object-safe Streamable iterator must yield owned buffers");
        assert_eq!(outputs, vec![b"ab".to_vec(), b"cd".to_vec()]);
    }
}

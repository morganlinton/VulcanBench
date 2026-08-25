use crate::EmitError;
use std::fmt;
use vyre_foundation::ir::DataType;

/// PTX scalar register classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PtxType {
    /// `.b16` register (`%h<N>`)  -  used for packed f16/bf16 memory values.
    B16,
    /// `.u32` register (`%r<N>`).
    U32,
    /// `.s32` register (`%s<N>`).
    I32,
    /// `.f32` register (`%f<N>`).
    F32,
    /// `.pred` register (`%p<N>`).
    Bool,
    /// `.u64` register (`%rd<N>`)  -  used for pointers.
    U64,
}

impl PtxType {
    pub(crate) fn ptx_type_str(self) -> &'static str {
        match self {
            Self::B16 => "b16",
            Self::U32 => "u32",
            Self::I32 => "s32",
            Self::F32 => "f32",
            Self::Bool => "pred",
            Self::U64 => "u64",
        }
    }

    pub(crate) fn reg_prefix(self) -> &'static str {
        match self {
            Self::B16 => "h",
            Self::U32 => "r",
            Self::I32 => "s",
            Self::F32 => "f",
            Self::Bool => "p",
            Self::U64 => "rd",
        }
    }

    pub(crate) fn from_dtype(dt: &DataType) -> Result<Self, EmitError> {
        match dt {
            DataType::Bool => Ok(Self::Bool),
            DataType::U8 | DataType::U16 | DataType::U32 => Ok(Self::U32),
            DataType::I8 | DataType::I16 | DataType::I32 => Ok(Self::I32),
            DataType::F16 | DataType::BF16 | DataType::F32 => Ok(Self::F32),
            // PTX 64-bit registers (`%rd`, `.u64`) are typeless bit containers
            // — signedness is per-instruction, not per-register — so both U64
            // and I64 map here. `validate::cast::cast_is_valid` allows
            // `i32 -> i64`, and `emit_cast`'s `(I32, U64) => cvt.s64.s32`
            // sign-extend / `(U32, U64) => cvt.u64.u32` zero-extend arms then
            // produce the correct 64-bit pattern. Before this, I64 fell to the
            // `other` arm and a valid `Cast { target: I64 }` errored.
            DataType::U64 | DataType::I64 => Ok(Self::U64),
            // `Bytes` is a packed-byte buffer-element marker, NOT a scalar
            // register type. Folding it into `.u32` here would silently
            // reinterpret a byte stream as a word (Law 10): a `Bytes` buffer
            // load would index words instead of bytes, and a `Cast { Bytes }`
            // of a u32 would no-op (src == dst == .u32). It needs a pack-to-u32
            // pre-pass before emission, so fail closed and name the fix.
            DataType::Bytes => Err(EmitError::UnsupportedDataType(
                "Bytes is a packed-byte buffer element, not a scalar register \
                 type; it requires a pack-to-u32 pre-pass before PTX emission \
                 and must never be reinterpreted as a u32 word"
                    .to_owned(),
            )),
            other => Err(EmitError::UnsupportedDataType(format!("{other:?}"))),
        }
    }
}

/// One named PTX register: a (type, index) pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Reg(pub(crate) PtxType, pub(crate) u32);

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "%{}{}", self.0.reg_prefix(), self.1)
    }
}

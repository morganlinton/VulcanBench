pub mod vyre_spec
pub mod vyre_spec::adversarial_input
pub struct vyre_spec::adversarial_input::AdversarialInput
pub vyre_spec::adversarial_input::AdversarialInput::input: &'static [u8]
pub vyre_spec::adversarial_input::AdversarialInput::reason: &'static str
impl core::clone::Clone for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::clone(&self) -> vyre_spec::adversarial_input::AdversarialInput
impl core::cmp::Eq for vyre_spec::adversarial_input::AdversarialInput
impl core::cmp::PartialEq for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::eq(&self, other: &vyre_spec::adversarial_input::AdversarialInput) -> bool
impl core::fmt::Debug for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::Freeze for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::Send for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::Sync for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::Unpin for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::UnsafeUnpin for vyre_spec::adversarial_input::AdversarialInput
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::adversarial_input::AdversarialInput
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::adversarial_input::AdversarialInput
impl<T, U> core::convert::Into<U> for vyre_spec::adversarial_input::AdversarialInput where U: core::convert::From<T>
pub fn vyre_spec::adversarial_input::AdversarialInput::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::adversarial_input::AdversarialInput where U: core::convert::Into<T>
pub type vyre_spec::adversarial_input::AdversarialInput::Error = core::convert::Infallible
pub fn vyre_spec::adversarial_input::AdversarialInput::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::adversarial_input::AdversarialInput where U: core::convert::TryFrom<T>
pub type vyre_spec::adversarial_input::AdversarialInput::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::adversarial_input::AdversarialInput::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::adversarial_input::AdversarialInput where T: core::clone::Clone
pub type vyre_spec::adversarial_input::AdversarialInput::Owned = T
pub fn vyre_spec::adversarial_input::AdversarialInput::clone_into(&self, target: &mut T)
pub fn vyre_spec::adversarial_input::AdversarialInput::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::adversarial_input::AdversarialInput where T: 'static + ?core::marker::Sized
pub fn vyre_spec::adversarial_input::AdversarialInput::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::adversarial_input::AdversarialInput where T: ?core::marker::Sized
pub fn vyre_spec::adversarial_input::AdversarialInput::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::adversarial_input::AdversarialInput where T: ?core::marker::Sized
pub fn vyre_spec::adversarial_input::AdversarialInput::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::adversarial_input::AdversarialInput where T: core::clone::Clone
pub unsafe fn vyre_spec::adversarial_input::AdversarialInput::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::from(t: T) -> T
pub mod vyre_spec::algebraic_law
#[non_exhaustive] pub enum vyre_spec::algebraic_law::AlgebraicLaw
pub vyre_spec::algebraic_law::AlgebraicLaw::Absorbing
pub vyre_spec::algebraic_law::AlgebraicLaw::Absorbing::element: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::Associative
pub vyre_spec::algebraic_law::AlgebraicLaw::Bounded
pub vyre_spec::algebraic_law::AlgebraicLaw::Bounded::hi: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::Bounded::lo: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::CategoricalAssociative
pub vyre_spec::algebraic_law::AlgebraicLaw::CategoricalIdentity
pub vyre_spec::algebraic_law::AlgebraicLaw::Commutative
pub vyre_spec::algebraic_law::AlgebraicLaw::Complement
pub vyre_spec::algebraic_law::AlgebraicLaw::Complement::complement_op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::Complement::universe: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::Custom
pub vyre_spec::algebraic_law::AlgebraicLaw::Custom::arity: usize
pub vyre_spec::algebraic_law::AlgebraicLaw::Custom::check: vyre_spec::algebraic_law::LawCheckFn
pub vyre_spec::algebraic_law::AlgebraicLaw::Custom::description: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::Custom::name: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::DeMorgan
pub vyre_spec::algebraic_law::AlgebraicLaw::DeMorgan::dual_op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::DeMorgan::inner_op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::DistributiveOver
pub vyre_spec::algebraic_law::AlgebraicLaw::DistributiveOver::over_op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::Idempotent
pub vyre_spec::algebraic_law::AlgebraicLaw::Identity
pub vyre_spec::algebraic_law::AlgebraicLaw::Identity::element: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::InverseOf
pub vyre_spec::algebraic_law::AlgebraicLaw::InverseOf::op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::Involution
pub vyre_spec::algebraic_law::AlgebraicLaw::LatticeAbsorption
pub vyre_spec::algebraic_law::AlgebraicLaw::LatticeAbsorption::dual_op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::LeftAbsorbing
pub vyre_spec::algebraic_law::AlgebraicLaw::LeftAbsorbing::element: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::LeftIdentity
pub vyre_spec::algebraic_law::AlgebraicLaw::LeftIdentity::element: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::Monotone
pub vyre_spec::algebraic_law::AlgebraicLaw::Monotonic
pub vyre_spec::algebraic_law::AlgebraicLaw::Monotonic::direction: vyre_spec::monotonic_direction::MonotonicDirection
pub vyre_spec::algebraic_law::AlgebraicLaw::RightAbsorbing
pub vyre_spec::algebraic_law::AlgebraicLaw::RightAbsorbing::element: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::RightIdentity
pub vyre_spec::algebraic_law::AlgebraicLaw::RightIdentity::element: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::SelfInverse
pub vyre_spec::algebraic_law::AlgebraicLaw::SelfInverse::result: u32
pub vyre_spec::algebraic_law::AlgebraicLaw::Trichotomy
pub vyre_spec::algebraic_law::AlgebraicLaw::Trichotomy::equal_op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::Trichotomy::greater_op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::Trichotomy::less_op: &'static str
pub vyre_spec::algebraic_law::AlgebraicLaw::ZeroProduct
pub vyre_spec::algebraic_law::AlgebraicLaw::ZeroProduct::holds: bool
impl vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::is_binary(&self) -> bool
pub fn vyre_spec::algebraic_law::AlgebraicLaw::is_unary(&self) -> bool
pub fn vyre_spec::algebraic_law::AlgebraicLaw::name(&self) -> &str
impl core::clone::Clone for vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::clone(&self) -> vyre_spec::algebraic_law::AlgebraicLaw
impl core::cmp::PartialEq for vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::eq(&self, other: &Self) -> bool
impl core::fmt::Debug for vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_spec::algebraic_law::AlgebraicLaw
impl core::marker::Send for vyre_spec::algebraic_law::AlgebraicLaw
impl core::marker::Sync for vyre_spec::algebraic_law::AlgebraicLaw
impl core::marker::Unpin for vyre_spec::algebraic_law::AlgebraicLaw
impl core::marker::UnsafeUnpin for vyre_spec::algebraic_law::AlgebraicLaw
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::algebraic_law::AlgebraicLaw
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::algebraic_law::AlgebraicLaw
impl<T, U> core::convert::Into<U> for vyre_spec::algebraic_law::AlgebraicLaw where U: core::convert::From<T>
pub fn vyre_spec::algebraic_law::AlgebraicLaw::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::algebraic_law::AlgebraicLaw where U: core::convert::Into<T>
pub type vyre_spec::algebraic_law::AlgebraicLaw::Error = core::convert::Infallible
pub fn vyre_spec::algebraic_law::AlgebraicLaw::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::algebraic_law::AlgebraicLaw where U: core::convert::TryFrom<T>
pub type vyre_spec::algebraic_law::AlgebraicLaw::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::algebraic_law::AlgebraicLaw::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::algebraic_law::AlgebraicLaw where T: core::clone::Clone
pub type vyre_spec::algebraic_law::AlgebraicLaw::Owned = T
pub fn vyre_spec::algebraic_law::AlgebraicLaw::clone_into(&self, target: &mut T)
pub fn vyre_spec::algebraic_law::AlgebraicLaw::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::algebraic_law::AlgebraicLaw where T: 'static + ?core::marker::Sized
pub fn vyre_spec::algebraic_law::AlgebraicLaw::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::algebraic_law::AlgebraicLaw where T: ?core::marker::Sized
pub fn vyre_spec::algebraic_law::AlgebraicLaw::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::algebraic_law::AlgebraicLaw where T: ?core::marker::Sized
pub fn vyre_spec::algebraic_law::AlgebraicLaw::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::algebraic_law::AlgebraicLaw where T: core::clone::Clone
pub unsafe fn vyre_spec::algebraic_law::AlgebraicLaw::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::from(t: T) -> T
pub type vyre_spec::algebraic_law::LawCheckFn = fn(fn(&[u8]) -> alloc::vec::Vec<u8>, &[u32]) -> bool
pub mod vyre_spec::all_algebraic_laws
pub fn vyre_spec::all_algebraic_laws::all_algebraic_laws() -> &'static [vyre_spec::algebraic_law::AlgebraicLaw]
pub mod vyre_spec::atomic_op
#[non_exhaustive] pub enum vyre_spec::atomic_op::AtomicOp
pub vyre_spec::atomic_op::AtomicOp::Add
pub vyre_spec::atomic_op::AtomicOp::And
pub vyre_spec::atomic_op::AtomicOp::CompareExchange
pub vyre_spec::atomic_op::AtomicOp::CompareExchangeWeak
pub vyre_spec::atomic_op::AtomicOp::Exchange
pub vyre_spec::atomic_op::AtomicOp::FetchNand
pub vyre_spec::atomic_op::AtomicOp::LruUpdate
pub vyre_spec::atomic_op::AtomicOp::Max
pub vyre_spec::atomic_op::AtomicOp::Min
pub vyre_spec::atomic_op::AtomicOp::Opaque(vyre_spec::extension::ExtensionAtomicOpId)
pub vyre_spec::atomic_op::AtomicOp::Or
pub vyre_spec::atomic_op::AtomicOp::Xor
impl vyre_spec::atomic_op::AtomicOp
pub const fn vyre_spec::atomic_op::AtomicOp::builtin_wire_tag(&self) -> core::option::Option<u8>
impl core::clone::Clone for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::clone(&self) -> vyre_spec::atomic_op::AtomicOp
impl core::cmp::Eq for vyre_spec::atomic_op::AtomicOp
impl core::cmp::PartialEq for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::eq(&self, other: &vyre_spec::atomic_op::AtomicOp) -> bool
impl core::fmt::Debug for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::atomic_op::AtomicOp
impl core::marker::StructuralPartialEq for vyre_spec::atomic_op::AtomicOp
impl serde_core::ser::Serialize for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::atomic_op::AtomicOp
impl core::marker::Send for vyre_spec::atomic_op::AtomicOp
impl core::marker::Sync for vyre_spec::atomic_op::AtomicOp
impl core::marker::Unpin for vyre_spec::atomic_op::AtomicOp
impl core::marker::UnsafeUnpin for vyre_spec::atomic_op::AtomicOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::atomic_op::AtomicOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::atomic_op::AtomicOp
impl<T, U> core::convert::Into<U> for vyre_spec::atomic_op::AtomicOp where U: core::convert::From<T>
pub fn vyre_spec::atomic_op::AtomicOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::atomic_op::AtomicOp where U: core::convert::Into<T>
pub type vyre_spec::atomic_op::AtomicOp::Error = core::convert::Infallible
pub fn vyre_spec::atomic_op::AtomicOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::atomic_op::AtomicOp where U: core::convert::TryFrom<T>
pub type vyre_spec::atomic_op::AtomicOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::atomic_op::AtomicOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::atomic_op::AtomicOp where T: core::clone::Clone
pub type vyre_spec::atomic_op::AtomicOp::Owned = T
pub fn vyre_spec::atomic_op::AtomicOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::atomic_op::AtomicOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::atomic_op::AtomicOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::atomic_op::AtomicOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::atomic_op::AtomicOp where T: ?core::marker::Sized
pub fn vyre_spec::atomic_op::AtomicOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::atomic_op::AtomicOp where T: ?core::marker::Sized
pub fn vyre_spec::atomic_op::AtomicOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::atomic_op::AtomicOp where T: core::clone::Clone
pub unsafe fn vyre_spec::atomic_op::AtomicOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::atomic_op::AtomicOp where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::bin_op
#[non_exhaustive] pub enum vyre_spec::bin_op::BinOp
pub vyre_spec::bin_op::BinOp::AbsDiff
pub vyre_spec::bin_op::BinOp::Add
pub vyre_spec::bin_op::BinOp::And
pub vyre_spec::bin_op::BinOp::Ballot
pub vyre_spec::bin_op::BinOp::BitAnd
pub vyre_spec::bin_op::BinOp::BitOr
pub vyre_spec::bin_op::BinOp::BitXor
pub vyre_spec::bin_op::BinOp::Div
pub vyre_spec::bin_op::BinOp::Eq
pub vyre_spec::bin_op::BinOp::Ge
pub vyre_spec::bin_op::BinOp::Gt
pub vyre_spec::bin_op::BinOp::Le
pub vyre_spec::bin_op::BinOp::Lt
pub vyre_spec::bin_op::BinOp::Max
pub vyre_spec::bin_op::BinOp::Min
pub vyre_spec::bin_op::BinOp::Mod
pub vyre_spec::bin_op::BinOp::Mul
pub vyre_spec::bin_op::BinOp::MulHigh
pub vyre_spec::bin_op::BinOp::Ne
pub vyre_spec::bin_op::BinOp::Opaque(vyre_spec::extension::ExtensionBinOpId)
pub vyre_spec::bin_op::BinOp::Or
pub vyre_spec::bin_op::BinOp::RotateLeft
pub vyre_spec::bin_op::BinOp::RotateRight
pub vyre_spec::bin_op::BinOp::SaturatingAdd
pub vyre_spec::bin_op::BinOp::SaturatingMul
pub vyre_spec::bin_op::BinOp::SaturatingSub
pub vyre_spec::bin_op::BinOp::Shl
pub vyre_spec::bin_op::BinOp::Shr
pub vyre_spec::bin_op::BinOp::Shuffle
pub vyre_spec::bin_op::BinOp::Sub
pub vyre_spec::bin_op::BinOp::WaveBroadcast
pub vyre_spec::bin_op::BinOp::WaveReduce
pub vyre_spec::bin_op::BinOp::WrappingAdd
pub vyre_spec::bin_op::BinOp::WrappingSub
impl vyre_spec::bin_op::BinOp
pub const fn vyre_spec::bin_op::BinOp::builtin_wire_tag(&self) -> core::option::Option<u8>
impl vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::intensity(&self) -> vyre_spec::bin_op::OpIntensity
impl core::clone::Clone for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::clone(&self) -> vyre_spec::bin_op::BinOp
impl core::cmp::Eq for vyre_spec::bin_op::BinOp
impl core::cmp::PartialEq for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::eq(&self, other: &vyre_spec::bin_op::BinOp) -> bool
impl core::fmt::Debug for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::bin_op::BinOp
impl core::marker::StructuralPartialEq for vyre_spec::bin_op::BinOp
impl serde_core::ser::Serialize for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::bin_op::BinOp
impl core::marker::Send for vyre_spec::bin_op::BinOp
impl core::marker::Sync for vyre_spec::bin_op::BinOp
impl core::marker::Unpin for vyre_spec::bin_op::BinOp
impl core::marker::UnsafeUnpin for vyre_spec::bin_op::BinOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::bin_op::BinOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::bin_op::BinOp
impl<T, U> core::convert::Into<U> for vyre_spec::bin_op::BinOp where U: core::convert::From<T>
pub fn vyre_spec::bin_op::BinOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::bin_op::BinOp where U: core::convert::Into<T>
pub type vyre_spec::bin_op::BinOp::Error = core::convert::Infallible
pub fn vyre_spec::bin_op::BinOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::bin_op::BinOp where U: core::convert::TryFrom<T>
pub type vyre_spec::bin_op::BinOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::bin_op::BinOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::bin_op::BinOp where T: core::clone::Clone
pub type vyre_spec::bin_op::BinOp::Owned = T
pub fn vyre_spec::bin_op::BinOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::bin_op::BinOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::bin_op::BinOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::bin_op::BinOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::bin_op::BinOp where T: ?core::marker::Sized
pub fn vyre_spec::bin_op::BinOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::bin_op::BinOp where T: ?core::marker::Sized
pub fn vyre_spec::bin_op::BinOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::bin_op::BinOp where T: core::clone::Clone
pub unsafe fn vyre_spec::bin_op::BinOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::bin_op::BinOp where T: for<'de> serde_core::de::Deserialize<'de>
pub enum vyre_spec::bin_op::OpIntensity
pub vyre_spec::bin_op::OpIntensity::Free
pub vyre_spec::bin_op::OpIntensity::Heavy
pub vyre_spec::bin_op::OpIntensity::Light
pub vyre_spec::bin_op::OpIntensity::Medium
impl core::clone::Clone for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::clone(&self) -> vyre_spec::bin_op::OpIntensity
impl core::cmp::Eq for vyre_spec::bin_op::OpIntensity
impl core::cmp::Ord for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::cmp(&self, other: &vyre_spec::bin_op::OpIntensity) -> core::cmp::Ordering
impl core::cmp::PartialEq for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::eq(&self, other: &vyre_spec::bin_op::OpIntensity) -> bool
impl core::cmp::PartialOrd for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::partial_cmp(&self, other: &vyre_spec::bin_op::OpIntensity) -> core::option::Option<core::cmp::Ordering>
impl core::fmt::Debug for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::bin_op::OpIntensity
impl core::marker::StructuralPartialEq for vyre_spec::bin_op::OpIntensity
impl serde_core::ser::Serialize for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::bin_op::OpIntensity
impl core::marker::Send for vyre_spec::bin_op::OpIntensity
impl core::marker::Sync for vyre_spec::bin_op::OpIntensity
impl core::marker::Unpin for vyre_spec::bin_op::OpIntensity
impl core::marker::UnsafeUnpin for vyre_spec::bin_op::OpIntensity
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::bin_op::OpIntensity
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::bin_op::OpIntensity
impl<T, U> core::convert::Into<U> for vyre_spec::bin_op::OpIntensity where U: core::convert::From<T>
pub fn vyre_spec::bin_op::OpIntensity::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::bin_op::OpIntensity where U: core::convert::Into<T>
pub type vyre_spec::bin_op::OpIntensity::Error = core::convert::Infallible
pub fn vyre_spec::bin_op::OpIntensity::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::bin_op::OpIntensity where U: core::convert::TryFrom<T>
pub type vyre_spec::bin_op::OpIntensity::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::bin_op::OpIntensity::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::bin_op::OpIntensity where T: core::clone::Clone
pub type vyre_spec::bin_op::OpIntensity::Owned = T
pub fn vyre_spec::bin_op::OpIntensity::clone_into(&self, target: &mut T)
pub fn vyre_spec::bin_op::OpIntensity::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::bin_op::OpIntensity where T: 'static + ?core::marker::Sized
pub fn vyre_spec::bin_op::OpIntensity::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::bin_op::OpIntensity where T: ?core::marker::Sized
pub fn vyre_spec::bin_op::OpIntensity::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::bin_op::OpIntensity where T: ?core::marker::Sized
pub fn vyre_spec::bin_op::OpIntensity::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::bin_op::OpIntensity where T: core::clone::Clone
pub unsafe fn vyre_spec::bin_op::OpIntensity::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::bin_op::OpIntensity
pub fn vyre_spec::bin_op::OpIntensity::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::bin_op::OpIntensity where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::buffer_access
#[non_exhaustive] pub enum vyre_spec::buffer_access::BufferAccess
pub vyre_spec::buffer_access::BufferAccess::ReadOnly
pub vyre_spec::buffer_access::BufferAccess::ReadWrite
pub vyre_spec::buffer_access::BufferAccess::Uniform
pub vyre_spec::buffer_access::BufferAccess::Workgroup
pub vyre_spec::buffer_access::BufferAccess::WriteOnly
impl core::clone::Clone for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::clone(&self) -> vyre_spec::buffer_access::BufferAccess
impl core::cmp::Eq for vyre_spec::buffer_access::BufferAccess
impl core::cmp::PartialEq for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::eq(&self, other: &vyre_spec::buffer_access::BufferAccess) -> bool
impl core::fmt::Debug for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::buffer_access::BufferAccess
impl serde_core::ser::Serialize for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::buffer_access::BufferAccess
impl core::marker::Send for vyre_spec::buffer_access::BufferAccess
impl core::marker::Sync for vyre_spec::buffer_access::BufferAccess
impl core::marker::Unpin for vyre_spec::buffer_access::BufferAccess
impl core::marker::UnsafeUnpin for vyre_spec::buffer_access::BufferAccess
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::buffer_access::BufferAccess
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::buffer_access::BufferAccess
impl<T, U> core::convert::Into<U> for vyre_spec::buffer_access::BufferAccess where U: core::convert::From<T>
pub fn vyre_spec::buffer_access::BufferAccess::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::buffer_access::BufferAccess where U: core::convert::Into<T>
pub type vyre_spec::buffer_access::BufferAccess::Error = core::convert::Infallible
pub fn vyre_spec::buffer_access::BufferAccess::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::buffer_access::BufferAccess where U: core::convert::TryFrom<T>
pub type vyre_spec::buffer_access::BufferAccess::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::buffer_access::BufferAccess::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::buffer_access::BufferAccess where T: core::clone::Clone
pub type vyre_spec::buffer_access::BufferAccess::Owned = T
pub fn vyre_spec::buffer_access::BufferAccess::clone_into(&self, target: &mut T)
pub fn vyre_spec::buffer_access::BufferAccess::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::buffer_access::BufferAccess where T: 'static + ?core::marker::Sized
pub fn vyre_spec::buffer_access::BufferAccess::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::buffer_access::BufferAccess where T: ?core::marker::Sized
pub fn vyre_spec::buffer_access::BufferAccess::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::buffer_access::BufferAccess where T: ?core::marker::Sized
pub fn vyre_spec::buffer_access::BufferAccess::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::buffer_access::BufferAccess where T: core::clone::Clone
pub unsafe fn vyre_spec::buffer_access::BufferAccess::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::buffer_access::BufferAccess where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::by_category
pub fn vyre_spec::by_category::by_category(category: vyre_spec::invariant_category::InvariantCategory) -> impl core::iter::traits::iterator::Iterator<Item = &'static vyre_spec::invariant::Invariant>
pub mod vyre_spec::by_id
pub fn vyre_spec::by_id::by_id(id: vyre_spec::engine_invariant::InvariantId) -> core::option::Option<&'static vyre_spec::invariant::Invariant>
pub mod vyre_spec::catalog_is_complete
pub fn vyre_spec::catalog_is_complete::catalog_is_complete() -> bool
pub mod vyre_spec::category
#[non_exhaustive] pub enum vyre_spec::category::Category
pub vyre_spec::category::Category::A
pub vyre_spec::category::Category::A::composition_of: alloc::vec::Vec<&'static str>
pub vyre_spec::category::Category::C
pub vyre_spec::category::Category::C::backend_availability: vyre_spec::category::BackendAvailabilityPredicate
pub vyre_spec::category::Category::C::hardware: &'static str
impl vyre_spec::category::Category
pub fn vyre_spec::category::Category::is_unclassified(&self) -> bool
pub fn vyre_spec::category::Category::unclassified() -> Self
impl core::clone::Clone for vyre_spec::category::Category
pub fn vyre_spec::category::Category::clone(&self) -> vyre_spec::category::Category
impl core::cmp::Eq for vyre_spec::category::Category
impl core::cmp::PartialEq for vyre_spec::category::Category
pub fn vyre_spec::category::Category::eq(&self, other: &Self) -> bool
impl core::fmt::Debug for vyre_spec::category::Category
pub fn vyre_spec::category::Category::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_spec::category::Category
impl core::marker::Send for vyre_spec::category::Category
impl core::marker::Sync for vyre_spec::category::Category
impl core::marker::Unpin for vyre_spec::category::Category
impl core::marker::UnsafeUnpin for vyre_spec::category::Category
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::category::Category
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::category::Category
impl<T, U> core::convert::Into<U> for vyre_spec::category::Category where U: core::convert::From<T>
pub fn vyre_spec::category::Category::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::category::Category where U: core::convert::Into<T>
pub type vyre_spec::category::Category::Error = core::convert::Infallible
pub fn vyre_spec::category::Category::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::category::Category where U: core::convert::TryFrom<T>
pub type vyre_spec::category::Category::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::category::Category::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::category::Category where T: core::clone::Clone
pub type vyre_spec::category::Category::Owned = T
pub fn vyre_spec::category::Category::clone_into(&self, target: &mut T)
pub fn vyre_spec::category::Category::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::category::Category where T: 'static + ?core::marker::Sized
pub fn vyre_spec::category::Category::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::category::Category where T: ?core::marker::Sized
pub fn vyre_spec::category::Category::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::category::Category where T: ?core::marker::Sized
pub fn vyre_spec::category::Category::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::category::Category where T: core::clone::Clone
pub unsafe fn vyre_spec::category::Category::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::category::Category
pub fn vyre_spec::category::Category::from(t: T) -> T
pub struct vyre_spec::category::BackendAvailabilityPredicate
impl vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::available(&self, op: &str) -> bool
pub const fn vyre_spec::category::BackendAvailabilityPredicate::new(predicate: fn(&str) -> bool) -> Self
impl core::clone::Clone for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::clone(&self) -> vyre_spec::category::BackendAvailabilityPredicate
impl core::fmt::Debug for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_spec::category::BackendAvailabilityPredicate
impl vyre_spec::category::BackendAvailability for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::available(&self, op: &str) -> bool
impl core::marker::Freeze for vyre_spec::category::BackendAvailabilityPredicate
impl core::marker::Send for vyre_spec::category::BackendAvailabilityPredicate
impl core::marker::Sync for vyre_spec::category::BackendAvailabilityPredicate
impl core::marker::Unpin for vyre_spec::category::BackendAvailabilityPredicate
impl core::marker::UnsafeUnpin for vyre_spec::category::BackendAvailabilityPredicate
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::category::BackendAvailabilityPredicate
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::category::BackendAvailabilityPredicate
impl<T, U> core::convert::Into<U> for vyre_spec::category::BackendAvailabilityPredicate where U: core::convert::From<T>
pub fn vyre_spec::category::BackendAvailabilityPredicate::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::category::BackendAvailabilityPredicate where U: core::convert::Into<T>
pub type vyre_spec::category::BackendAvailabilityPredicate::Error = core::convert::Infallible
pub fn vyre_spec::category::BackendAvailabilityPredicate::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::category::BackendAvailabilityPredicate where U: core::convert::TryFrom<T>
pub type vyre_spec::category::BackendAvailabilityPredicate::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::category::BackendAvailabilityPredicate::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::category::BackendAvailabilityPredicate where T: core::clone::Clone
pub type vyre_spec::category::BackendAvailabilityPredicate::Owned = T
pub fn vyre_spec::category::BackendAvailabilityPredicate::clone_into(&self, target: &mut T)
pub fn vyre_spec::category::BackendAvailabilityPredicate::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::category::BackendAvailabilityPredicate where T: 'static + ?core::marker::Sized
pub fn vyre_spec::category::BackendAvailabilityPredicate::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::category::BackendAvailabilityPredicate where T: ?core::marker::Sized
pub fn vyre_spec::category::BackendAvailabilityPredicate::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::category::BackendAvailabilityPredicate where T: ?core::marker::Sized
pub fn vyre_spec::category::BackendAvailabilityPredicate::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::category::BackendAvailabilityPredicate where T: core::clone::Clone
pub unsafe fn vyre_spec::category::BackendAvailabilityPredicate::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::from(t: T) -> T
pub trait vyre_spec::category::BackendAvailability: core::marker::Send + core::marker::Sync
pub fn vyre_spec::category::BackendAvailability::available(&self, op: &str) -> bool
impl vyre_spec::category::BackendAvailability for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::available(&self, op: &str) -> bool
impl<F> vyre_spec::category::BackendAvailability for F where F: core::ops::function::Fn(&str) -> bool + core::marker::Send + core::marker::Sync
pub fn F::available(&self, op: &str) -> bool
pub mod vyre_spec::collective_op
#[non_exhaustive] pub enum vyre_spec::collective_op::CollectiveOp
pub vyre_spec::collective_op::CollectiveOp::BitAnd
pub vyre_spec::collective_op::CollectiveOp::BitOr
pub vyre_spec::collective_op::CollectiveOp::BitXor
pub vyre_spec::collective_op::CollectiveOp::Max
pub vyre_spec::collective_op::CollectiveOp::Min
pub vyre_spec::collective_op::CollectiveOp::Sum
impl vyre_spec::collective_op::CollectiveOp
pub const fn vyre_spec::collective_op::CollectiveOp::builtin_wire_tag(self) -> u8
pub fn vyre_spec::collective_op::CollectiveOp::from_wire_tag(tag: u8) -> core::result::Result<Self, alloc::string::String>
impl core::clone::Clone for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::clone(&self) -> vyre_spec::collective_op::CollectiveOp
impl core::cmp::Eq for vyre_spec::collective_op::CollectiveOp
impl core::cmp::PartialEq for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::eq(&self, other: &vyre_spec::collective_op::CollectiveOp) -> bool
impl core::fmt::Debug for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::collective_op::CollectiveOp
impl core::marker::StructuralPartialEq for vyre_spec::collective_op::CollectiveOp
impl serde_core::ser::Serialize for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::collective_op::CollectiveOp
impl core::marker::Send for vyre_spec::collective_op::CollectiveOp
impl core::marker::Sync for vyre_spec::collective_op::CollectiveOp
impl core::marker::Unpin for vyre_spec::collective_op::CollectiveOp
impl core::marker::UnsafeUnpin for vyre_spec::collective_op::CollectiveOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::collective_op::CollectiveOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::collective_op::CollectiveOp
impl<T, U> core::convert::Into<U> for vyre_spec::collective_op::CollectiveOp where U: core::convert::From<T>
pub fn vyre_spec::collective_op::CollectiveOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::collective_op::CollectiveOp where U: core::convert::Into<T>
pub type vyre_spec::collective_op::CollectiveOp::Error = core::convert::Infallible
pub fn vyre_spec::collective_op::CollectiveOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::collective_op::CollectiveOp where U: core::convert::TryFrom<T>
pub type vyre_spec::collective_op::CollectiveOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::collective_op::CollectiveOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::collective_op::CollectiveOp where T: core::clone::Clone
pub type vyre_spec::collective_op::CollectiveOp::Owned = T
pub fn vyre_spec::collective_op::CollectiveOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::collective_op::CollectiveOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::collective_op::CollectiveOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::collective_op::CollectiveOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::collective_op::CollectiveOp where T: ?core::marker::Sized
pub fn vyre_spec::collective_op::CollectiveOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::collective_op::CollectiveOp where T: ?core::marker::Sized
pub fn vyre_spec::collective_op::CollectiveOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::collective_op::CollectiveOp where T: core::clone::Clone
pub unsafe fn vyre_spec::collective_op::CollectiveOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::collective_op::CollectiveOp where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::collective_op::CommGroup(pub u32)
impl vyre_spec::collective_op::CommGroup
pub const vyre_spec::collective_op::CommGroup::WORLD: Self
pub const fn vyre_spec::collective_op::CommGroup::as_u32(self) -> u32
impl core::clone::Clone for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::clone(&self) -> vyre_spec::collective_op::CommGroup
impl core::cmp::Eq for vyre_spec::collective_op::CommGroup
impl core::cmp::PartialEq for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::eq(&self, other: &vyre_spec::collective_op::CommGroup) -> bool
impl core::fmt::Debug for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::collective_op::CommGroup
impl core::marker::StructuralPartialEq for vyre_spec::collective_op::CommGroup
impl serde_core::ser::Serialize for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::collective_op::CommGroup
impl core::marker::Send for vyre_spec::collective_op::CommGroup
impl core::marker::Sync for vyre_spec::collective_op::CommGroup
impl core::marker::Unpin for vyre_spec::collective_op::CommGroup
impl core::marker::UnsafeUnpin for vyre_spec::collective_op::CommGroup
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::collective_op::CommGroup
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::collective_op::CommGroup
impl<T, U> core::convert::Into<U> for vyre_spec::collective_op::CommGroup where U: core::convert::From<T>
pub fn vyre_spec::collective_op::CommGroup::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::collective_op::CommGroup where U: core::convert::Into<T>
pub type vyre_spec::collective_op::CommGroup::Error = core::convert::Infallible
pub fn vyre_spec::collective_op::CommGroup::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::collective_op::CommGroup where U: core::convert::TryFrom<T>
pub type vyre_spec::collective_op::CommGroup::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::collective_op::CommGroup::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::collective_op::CommGroup where T: core::clone::Clone
pub type vyre_spec::collective_op::CommGroup::Owned = T
pub fn vyre_spec::collective_op::CommGroup::clone_into(&self, target: &mut T)
pub fn vyre_spec::collective_op::CommGroup::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::collective_op::CommGroup where T: 'static + ?core::marker::Sized
pub fn vyre_spec::collective_op::CommGroup::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::collective_op::CommGroup where T: ?core::marker::Sized
pub fn vyre_spec::collective_op::CommGroup::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::collective_op::CommGroup where T: ?core::marker::Sized
pub fn vyre_spec::collective_op::CommGroup::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::collective_op::CommGroup where T: core::clone::Clone
pub unsafe fn vyre_spec::collective_op::CommGroup::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::collective_op::CommGroup where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::convention
#[non_exhaustive] pub enum vyre_spec::convention::Convention
pub vyre_spec::convention::Convention::V1
pub vyre_spec::convention::Convention::V2
pub vyre_spec::convention::Convention::V2::lookup_binding: u32
impl core::clone::Clone for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::clone(&self) -> vyre_spec::convention::Convention
impl core::cmp::Eq for vyre_spec::convention::Convention
impl core::cmp::PartialEq for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::eq(&self, other: &vyre_spec::convention::Convention) -> bool
impl core::default::Default for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::default() -> vyre_spec::convention::Convention
impl core::fmt::Debug for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::convention::Convention
impl serde_core::ser::Serialize for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::convention::Convention
impl core::marker::Send for vyre_spec::convention::Convention
impl core::marker::Sync for vyre_spec::convention::Convention
impl core::marker::Unpin for vyre_spec::convention::Convention
impl core::marker::UnsafeUnpin for vyre_spec::convention::Convention
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::convention::Convention
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::convention::Convention
impl<T, U> core::convert::Into<U> for vyre_spec::convention::Convention where U: core::convert::From<T>
pub fn vyre_spec::convention::Convention::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::convention::Convention where U: core::convert::Into<T>
pub type vyre_spec::convention::Convention::Error = core::convert::Infallible
pub fn vyre_spec::convention::Convention::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::convention::Convention where U: core::convert::TryFrom<T>
pub type vyre_spec::convention::Convention::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::convention::Convention::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::convention::Convention where T: core::clone::Clone
pub type vyre_spec::convention::Convention::Owned = T
pub fn vyre_spec::convention::Convention::clone_into(&self, target: &mut T)
pub fn vyre_spec::convention::Convention::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::convention::Convention where T: 'static + ?core::marker::Sized
pub fn vyre_spec::convention::Convention::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::convention::Convention where T: ?core::marker::Sized
pub fn vyre_spec::convention::Convention::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::convention::Convention where T: ?core::marker::Sized
pub fn vyre_spec::convention::Convention::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::convention::Convention where T: core::clone::Clone
pub unsafe fn vyre_spec::convention::Convention::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::convention::Convention where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::data_type
#[non_exhaustive] pub enum vyre_spec::data_type::DataType
pub vyre_spec::data_type::DataType::Array
pub vyre_spec::data_type::DataType::Array::element_size: usize
pub vyre_spec::data_type::DataType::BF16
pub vyre_spec::data_type::DataType::Bool
pub vyre_spec::data_type::DataType::Bytes
pub vyre_spec::data_type::DataType::DeviceMesh
pub vyre_spec::data_type::DataType::DeviceMesh::axes: smallvec::SmallVec<[u32; 3]>
pub vyre_spec::data_type::DataType::F16
pub vyre_spec::data_type::DataType::F32
pub vyre_spec::data_type::DataType::F64
pub vyre_spec::data_type::DataType::F8E4M3
pub vyre_spec::data_type::DataType::F8E5M2
pub vyre_spec::data_type::DataType::FP4
pub vyre_spec::data_type::DataType::Handle(vyre_spec::data_type::TypeId)
pub vyre_spec::data_type::DataType::I16
pub vyre_spec::data_type::DataType::I32
pub vyre_spec::data_type::DataType::I4
pub vyre_spec::data_type::DataType::I64
pub vyre_spec::data_type::DataType::I8
pub vyre_spec::data_type::DataType::NF4
pub vyre_spec::data_type::DataType::Opaque(vyre_spec::extension::ExtensionDataTypeId)
pub vyre_spec::data_type::DataType::Quantized
pub vyre_spec::data_type::DataType::Quantized::scale: vyre_spec::data_type::QuantizationScale
pub vyre_spec::data_type::DataType::Quantized::storage: alloc::boxed::Box<Self>
pub vyre_spec::data_type::DataType::Quantized::zero_point: vyre_spec::data_type::QuantizationZeroPoint
pub vyre_spec::data_type::DataType::SparseBsr
pub vyre_spec::data_type::DataType::SparseBsr::block_cols: u32
pub vyre_spec::data_type::DataType::SparseBsr::block_rows: u32
pub vyre_spec::data_type::DataType::SparseBsr::element: alloc::boxed::Box<Self>
pub vyre_spec::data_type::DataType::SparseCoo
pub vyre_spec::data_type::DataType::SparseCoo::element: alloc::boxed::Box<Self>
pub vyre_spec::data_type::DataType::SparseCsr
pub vyre_spec::data_type::DataType::SparseCsr::element: alloc::boxed::Box<Self>
pub vyre_spec::data_type::DataType::Tensor
pub vyre_spec::data_type::DataType::TensorShaped
pub vyre_spec::data_type::DataType::TensorShaped::element: alloc::boxed::Box<Self>
pub vyre_spec::data_type::DataType::TensorShaped::shape: smallvec::SmallVec<[u32; 4]>
pub vyre_spec::data_type::DataType::U16
pub vyre_spec::data_type::DataType::U32
pub vyre_spec::data_type::DataType::U64
pub vyre_spec::data_type::DataType::U8
pub vyre_spec::data_type::DataType::Vec
pub vyre_spec::data_type::DataType::Vec::count: u8
pub vyre_spec::data_type::DataType::Vec::element: alloc::boxed::Box<Self>
pub vyre_spec::data_type::DataType::Vec2U32
pub vyre_spec::data_type::DataType::Vec4U32
impl vyre_spec::data_type::DataType
pub const fn vyre_spec::data_type::DataType::bit_width(&self) -> core::option::Option<usize>
pub const fn vyre_spec::data_type::DataType::element_size(&self) -> core::option::Option<usize>
pub const fn vyre_spec::data_type::DataType::max_bytes(&self) -> core::option::Option<usize>
pub const fn vyre_spec::data_type::DataType::min_bytes(&self) -> usize
pub fn vyre_spec::data_type::DataType::packed_size_bytes(&self, element_count: usize) -> core::result::Result<core::option::Option<usize>, alloc::string::String>
pub const fn vyre_spec::data_type::DataType::size_bytes(&self) -> core::option::Option<usize>
impl vyre_spec::data_type::DataType
pub const fn vyre_spec::data_type::DataType::builtin_wire_tag(&self) -> core::option::Option<u8>
pub const fn vyre_spec::data_type::DataType::is_float_family(&self) -> bool
pub const fn vyre_spec::data_type::DataType::is_quantized(&self) -> bool
pub const fn vyre_spec::data_type::DataType::is_quantized_storage(&self) -> bool
impl vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::validate_layout(&self) -> core::result::Result<(), alloc::string::String>
impl core::clone::Clone for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::clone(&self) -> vyre_spec::data_type::DataType
impl core::cmp::Eq for vyre_spec::data_type::DataType
impl core::cmp::PartialEq for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::eq(&self, other: &vyre_spec::data_type::DataType) -> bool
impl core::fmt::Debug for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::data_type::DataType
impl serde_core::ser::Serialize for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::data_type::DataType
impl core::marker::Send for vyre_spec::data_type::DataType
impl core::marker::Sync for vyre_spec::data_type::DataType
impl core::marker::Unpin for vyre_spec::data_type::DataType
impl core::marker::UnsafeUnpin for vyre_spec::data_type::DataType
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::data_type::DataType
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::data_type::DataType
impl<T, U> core::convert::Into<U> for vyre_spec::data_type::DataType where U: core::convert::From<T>
pub fn vyre_spec::data_type::DataType::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::data_type::DataType where U: core::convert::Into<T>
pub type vyre_spec::data_type::DataType::Error = core::convert::Infallible
pub fn vyre_spec::data_type::DataType::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::data_type::DataType where U: core::convert::TryFrom<T>
pub type vyre_spec::data_type::DataType::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::data_type::DataType::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::data_type::DataType where T: core::clone::Clone
pub type vyre_spec::data_type::DataType::Owned = T
pub fn vyre_spec::data_type::DataType::clone_into(&self, target: &mut T)
pub fn vyre_spec::data_type::DataType::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::data_type::DataType where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::data_type::DataType::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::data_type::DataType where T: 'static + ?core::marker::Sized
pub fn vyre_spec::data_type::DataType::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::data_type::DataType where T: ?core::marker::Sized
pub fn vyre_spec::data_type::DataType::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::data_type::DataType where T: ?core::marker::Sized
pub fn vyre_spec::data_type::DataType::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::data_type::DataType where T: core::clone::Clone
pub unsafe fn vyre_spec::data_type::DataType::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::data_type::DataType where T: for<'de> serde_core::de::Deserialize<'de>
pub enum vyre_spec::data_type::QuantizationScale
pub vyre_spec::data_type::QuantizationScale::PerChannel
pub vyre_spec::data_type::QuantizationScale::PerChannel::axis: u32
pub vyre_spec::data_type::QuantizationScale::PerGroup
pub vyre_spec::data_type::QuantizationScale::PerGroup::group_size: u32
pub vyre_spec::data_type::QuantizationScale::PerTensor
impl core::clone::Clone for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::clone(&self) -> vyre_spec::data_type::QuantizationScale
impl core::cmp::Eq for vyre_spec::data_type::QuantizationScale
impl core::cmp::PartialEq for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::eq(&self, other: &vyre_spec::data_type::QuantizationScale) -> bool
impl core::fmt::Debug for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::data_type::QuantizationScale
impl serde_core::ser::Serialize for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::data_type::QuantizationScale
impl core::marker::Send for vyre_spec::data_type::QuantizationScale
impl core::marker::Sync for vyre_spec::data_type::QuantizationScale
impl core::marker::Unpin for vyre_spec::data_type::QuantizationScale
impl core::marker::UnsafeUnpin for vyre_spec::data_type::QuantizationScale
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::data_type::QuantizationScale
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::data_type::QuantizationScale
impl<T, U> core::convert::Into<U> for vyre_spec::data_type::QuantizationScale where U: core::convert::From<T>
pub fn vyre_spec::data_type::QuantizationScale::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::data_type::QuantizationScale where U: core::convert::Into<T>
pub type vyre_spec::data_type::QuantizationScale::Error = core::convert::Infallible
pub fn vyre_spec::data_type::QuantizationScale::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::data_type::QuantizationScale where U: core::convert::TryFrom<T>
pub type vyre_spec::data_type::QuantizationScale::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::data_type::QuantizationScale::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::data_type::QuantizationScale where T: core::clone::Clone
pub type vyre_spec::data_type::QuantizationScale::Owned = T
pub fn vyre_spec::data_type::QuantizationScale::clone_into(&self, target: &mut T)
pub fn vyre_spec::data_type::QuantizationScale::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::data_type::QuantizationScale where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationScale::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::data_type::QuantizationScale where T: 'static + ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationScale::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::data_type::QuantizationScale where T: ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationScale::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::data_type::QuantizationScale where T: ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationScale::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::data_type::QuantizationScale where T: core::clone::Clone
pub unsafe fn vyre_spec::data_type::QuantizationScale::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::data_type::QuantizationScale where T: for<'de> serde_core::de::Deserialize<'de>
pub enum vyre_spec::data_type::QuantizationZeroPoint
pub vyre_spec::data_type::QuantizationZeroPoint::Absent
pub vyre_spec::data_type::QuantizationZeroPoint::PerChannel
pub vyre_spec::data_type::QuantizationZeroPoint::PerChannel::axis: u32
pub vyre_spec::data_type::QuantizationZeroPoint::PerGroup
pub vyre_spec::data_type::QuantizationZeroPoint::PerGroup::group_size: u32
pub vyre_spec::data_type::QuantizationZeroPoint::PerTensor
impl core::clone::Clone for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::clone(&self) -> vyre_spec::data_type::QuantizationZeroPoint
impl core::cmp::Eq for vyre_spec::data_type::QuantizationZeroPoint
impl core::cmp::PartialEq for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::eq(&self, other: &vyre_spec::data_type::QuantizationZeroPoint) -> bool
impl core::fmt::Debug for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::data_type::QuantizationZeroPoint
impl serde_core::ser::Serialize for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::data_type::QuantizationZeroPoint
impl core::marker::Send for vyre_spec::data_type::QuantizationZeroPoint
impl core::marker::Sync for vyre_spec::data_type::QuantizationZeroPoint
impl core::marker::Unpin for vyre_spec::data_type::QuantizationZeroPoint
impl core::marker::UnsafeUnpin for vyre_spec::data_type::QuantizationZeroPoint
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::data_type::QuantizationZeroPoint
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::data_type::QuantizationZeroPoint
impl<T, U> core::convert::Into<U> for vyre_spec::data_type::QuantizationZeroPoint where U: core::convert::From<T>
pub fn vyre_spec::data_type::QuantizationZeroPoint::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::data_type::QuantizationZeroPoint where U: core::convert::Into<T>
pub type vyre_spec::data_type::QuantizationZeroPoint::Error = core::convert::Infallible
pub fn vyre_spec::data_type::QuantizationZeroPoint::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::data_type::QuantizationZeroPoint where U: core::convert::TryFrom<T>
pub type vyre_spec::data_type::QuantizationZeroPoint::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::data_type::QuantizationZeroPoint::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::data_type::QuantizationZeroPoint where T: core::clone::Clone
pub type vyre_spec::data_type::QuantizationZeroPoint::Owned = T
pub fn vyre_spec::data_type::QuantizationZeroPoint::clone_into(&self, target: &mut T)
pub fn vyre_spec::data_type::QuantizationZeroPoint::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::data_type::QuantizationZeroPoint where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationZeroPoint::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::data_type::QuantizationZeroPoint where T: 'static + ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationZeroPoint::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::data_type::QuantizationZeroPoint where T: ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationZeroPoint::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::data_type::QuantizationZeroPoint where T: ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationZeroPoint::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::data_type::QuantizationZeroPoint where T: core::clone::Clone
pub unsafe fn vyre_spec::data_type::QuantizationZeroPoint::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::data_type::QuantizationZeroPoint where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::data_type::TypeId(pub u32)
impl vyre_spec::data_type::TypeId
pub const fn vyre_spec::data_type::TypeId::as_u32(self) -> u32
impl core::clone::Clone for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::clone(&self) -> vyre_spec::data_type::TypeId
impl core::cmp::Eq for vyre_spec::data_type::TypeId
impl core::cmp::PartialEq for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::eq(&self, other: &vyre_spec::data_type::TypeId) -> bool
impl core::fmt::Debug for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::data_type::TypeId
impl core::marker::StructuralPartialEq for vyre_spec::data_type::TypeId
impl serde_core::ser::Serialize for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::data_type::TypeId
impl core::marker::Send for vyre_spec::data_type::TypeId
impl core::marker::Sync for vyre_spec::data_type::TypeId
impl core::marker::Unpin for vyre_spec::data_type::TypeId
impl core::marker::UnsafeUnpin for vyre_spec::data_type::TypeId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::data_type::TypeId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::data_type::TypeId
impl<T, U> core::convert::Into<U> for vyre_spec::data_type::TypeId where U: core::convert::From<T>
pub fn vyre_spec::data_type::TypeId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::data_type::TypeId where U: core::convert::Into<T>
pub type vyre_spec::data_type::TypeId::Error = core::convert::Infallible
pub fn vyre_spec::data_type::TypeId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::data_type::TypeId where U: core::convert::TryFrom<T>
pub type vyre_spec::data_type::TypeId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::data_type::TypeId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::data_type::TypeId where T: core::clone::Clone
pub type vyre_spec::data_type::TypeId::Owned = T
pub fn vyre_spec::data_type::TypeId::clone_into(&self, target: &mut T)
pub fn vyre_spec::data_type::TypeId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::data_type::TypeId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::data_type::TypeId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::data_type::TypeId where T: ?core::marker::Sized
pub fn vyre_spec::data_type::TypeId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::data_type::TypeId where T: ?core::marker::Sized
pub fn vyre_spec::data_type::TypeId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::data_type::TypeId where T: core::clone::Clone
pub unsafe fn vyre_spec::data_type::TypeId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::data_type::TypeId where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::engine_invariant
#[non_exhaustive] pub enum vyre_spec::engine_invariant::EngineInvariant
pub vyre_spec::engine_invariant::EngineInvariant::I1 = 1
pub vyre_spec::engine_invariant::EngineInvariant::I10 = 10
pub vyre_spec::engine_invariant::EngineInvariant::I11 = 11
pub vyre_spec::engine_invariant::EngineInvariant::I12 = 12
pub vyre_spec::engine_invariant::EngineInvariant::I13 = 13
pub vyre_spec::engine_invariant::EngineInvariant::I14 = 14
pub vyre_spec::engine_invariant::EngineInvariant::I15 = 15
pub vyre_spec::engine_invariant::EngineInvariant::I2 = 2
pub vyre_spec::engine_invariant::EngineInvariant::I3 = 3
pub vyre_spec::engine_invariant::EngineInvariant::I4 = 4
pub vyre_spec::engine_invariant::EngineInvariant::I5 = 5
pub vyre_spec::engine_invariant::EngineInvariant::I6 = 6
pub vyre_spec::engine_invariant::EngineInvariant::I7 = 7
pub vyre_spec::engine_invariant::EngineInvariant::I8 = 8
pub vyre_spec::engine_invariant::EngineInvariant::I9 = 9
impl vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::iter() -> impl core::iter::traits::iterator::Iterator<Item = Self>
pub const fn vyre_spec::engine_invariant::EngineInvariant::ordinal(&self) -> u8
impl core::clone::Clone for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::clone(&self) -> vyre_spec::engine_invariant::EngineInvariant
impl core::cmp::Eq for vyre_spec::engine_invariant::EngineInvariant
impl core::cmp::Ord for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::cmp(&self, other: &vyre_spec::engine_invariant::EngineInvariant) -> core::cmp::Ordering
impl core::cmp::PartialEq for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::eq(&self, other: &vyre_spec::engine_invariant::EngineInvariant) -> bool
impl core::cmp::PartialOrd for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::partial_cmp(&self, other: &vyre_spec::engine_invariant::EngineInvariant) -> core::option::Option<core::cmp::Ordering>
impl core::fmt::Debug for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::StructuralPartialEq for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::Freeze for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::Send for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::Sync for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::Unpin for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::UnsafeUnpin for vyre_spec::engine_invariant::EngineInvariant
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::engine_invariant::EngineInvariant
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::engine_invariant::EngineInvariant
impl<T, U> core::convert::Into<U> for vyre_spec::engine_invariant::EngineInvariant where U: core::convert::From<T>
pub fn vyre_spec::engine_invariant::EngineInvariant::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::engine_invariant::EngineInvariant where U: core::convert::Into<T>
pub type vyre_spec::engine_invariant::EngineInvariant::Error = core::convert::Infallible
pub fn vyre_spec::engine_invariant::EngineInvariant::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::engine_invariant::EngineInvariant where U: core::convert::TryFrom<T>
pub type vyre_spec::engine_invariant::EngineInvariant::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::engine_invariant::EngineInvariant::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::engine_invariant::EngineInvariant where T: core::clone::Clone
pub type vyre_spec::engine_invariant::EngineInvariant::Owned = T
pub fn vyre_spec::engine_invariant::EngineInvariant::clone_into(&self, target: &mut T)
pub fn vyre_spec::engine_invariant::EngineInvariant::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::engine_invariant::EngineInvariant where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::engine_invariant::EngineInvariant::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::engine_invariant::EngineInvariant where T: 'static + ?core::marker::Sized
pub fn vyre_spec::engine_invariant::EngineInvariant::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::engine_invariant::EngineInvariant where T: ?core::marker::Sized
pub fn vyre_spec::engine_invariant::EngineInvariant::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::engine_invariant::EngineInvariant where T: ?core::marker::Sized
pub fn vyre_spec::engine_invariant::EngineInvariant::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::engine_invariant::EngineInvariant where T: core::clone::Clone
pub unsafe fn vyre_spec::engine_invariant::EngineInvariant::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::from(t: T) -> T
pub type vyre_spec::engine_invariant::InvariantId = vyre_spec::engine_invariant::EngineInvariant
pub mod vyre_spec::expr_variant
pub fn vyre_spec::expr_variant::expr_variants() -> &'static [&'static str]
pub mod vyre_spec::extension
pub struct vyre_spec::extension::ExtensionAtomicOpId(pub u32)
impl vyre_spec::extension::ExtensionAtomicOpId
pub const vyre_spec::extension::ExtensionAtomicOpId::EXTENSION_RANGE_MASK: u32
pub const fn vyre_spec::extension::ExtensionAtomicOpId::as_u32(self) -> u32
pub const fn vyre_spec::extension::ExtensionAtomicOpId::from_name(name: &str) -> Self
pub const fn vyre_spec::extension::ExtensionAtomicOpId::is_extension(self) -> bool
impl core::clone::Clone for vyre_spec::extension::ExtensionAtomicOpId
pub fn vyre_spec::extension::ExtensionAtomicOpId::clone(&self) -> vyre_spec::extension::ExtensionAtomicOpId
impl core::cmp::Eq for vyre_spec::extension::ExtensionAtomicOpId
impl core::cmp::PartialEq for vyre_spec::extension::ExtensionAtomicOpId
pub fn vyre_spec::extension::ExtensionAtomicOpId::eq(&self, other: &vyre_spec::extension::ExtensionAtomicOpId) -> bool
impl core::fmt::Debug for vyre_spec::extension::ExtensionAtomicOpId
pub fn vyre_spec::extension::ExtensionAtomicOpId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::extension::ExtensionAtomicOpId
pub fn vyre_spec::extension::ExtensionAtomicOpId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::extension::ExtensionAtomicOpId
impl core::marker::StructuralPartialEq for vyre_spec::extension::ExtensionAtomicOpId
impl serde_core::ser::Serialize for vyre_spec::extension::ExtensionAtomicOpId
pub fn vyre_spec::extension::ExtensionAtomicOpId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::extension::ExtensionAtomicOpId
pub fn vyre_spec::extension::ExtensionAtomicOpId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::extension::ExtensionAtomicOpId
impl core::marker::Send for vyre_spec::extension::ExtensionAtomicOpId
impl core::marker::Sync for vyre_spec::extension::ExtensionAtomicOpId
impl core::marker::Unpin for vyre_spec::extension::ExtensionAtomicOpId
impl core::marker::UnsafeUnpin for vyre_spec::extension::ExtensionAtomicOpId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::extension::ExtensionAtomicOpId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::extension::ExtensionAtomicOpId
impl<T, U> core::convert::Into<U> for vyre_spec::extension::ExtensionAtomicOpId where U: core::convert::From<T>
pub fn vyre_spec::extension::ExtensionAtomicOpId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::extension::ExtensionAtomicOpId where U: core::convert::Into<T>
pub type vyre_spec::extension::ExtensionAtomicOpId::Error = core::convert::Infallible
pub fn vyre_spec::extension::ExtensionAtomicOpId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::extension::ExtensionAtomicOpId where U: core::convert::TryFrom<T>
pub type vyre_spec::extension::ExtensionAtomicOpId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::extension::ExtensionAtomicOpId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::extension::ExtensionAtomicOpId where T: core::clone::Clone
pub type vyre_spec::extension::ExtensionAtomicOpId::Owned = T
pub fn vyre_spec::extension::ExtensionAtomicOpId::clone_into(&self, target: &mut T)
pub fn vyre_spec::extension::ExtensionAtomicOpId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::extension::ExtensionAtomicOpId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionAtomicOpId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::extension::ExtensionAtomicOpId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionAtomicOpId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::extension::ExtensionAtomicOpId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionAtomicOpId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::extension::ExtensionAtomicOpId where T: core::clone::Clone
pub unsafe fn vyre_spec::extension::ExtensionAtomicOpId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::extension::ExtensionAtomicOpId
pub fn vyre_spec::extension::ExtensionAtomicOpId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::extension::ExtensionAtomicOpId where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::extension::ExtensionBinOpId(pub u32)
impl vyre_spec::extension::ExtensionBinOpId
pub const vyre_spec::extension::ExtensionBinOpId::EXTENSION_RANGE_MASK: u32
pub const fn vyre_spec::extension::ExtensionBinOpId::as_u32(self) -> u32
pub const fn vyre_spec::extension::ExtensionBinOpId::from_name(name: &str) -> Self
pub const fn vyre_spec::extension::ExtensionBinOpId::is_extension(self) -> bool
impl core::clone::Clone for vyre_spec::extension::ExtensionBinOpId
pub fn vyre_spec::extension::ExtensionBinOpId::clone(&self) -> vyre_spec::extension::ExtensionBinOpId
impl core::cmp::Eq for vyre_spec::extension::ExtensionBinOpId
impl core::cmp::PartialEq for vyre_spec::extension::ExtensionBinOpId
pub fn vyre_spec::extension::ExtensionBinOpId::eq(&self, other: &vyre_spec::extension::ExtensionBinOpId) -> bool
impl core::fmt::Debug for vyre_spec::extension::ExtensionBinOpId
pub fn vyre_spec::extension::ExtensionBinOpId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::extension::ExtensionBinOpId
pub fn vyre_spec::extension::ExtensionBinOpId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::extension::ExtensionBinOpId
impl core::marker::StructuralPartialEq for vyre_spec::extension::ExtensionBinOpId
impl serde_core::ser::Serialize for vyre_spec::extension::ExtensionBinOpId
pub fn vyre_spec::extension::ExtensionBinOpId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::extension::ExtensionBinOpId
pub fn vyre_spec::extension::ExtensionBinOpId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::extension::ExtensionBinOpId
impl core::marker::Send for vyre_spec::extension::ExtensionBinOpId
impl core::marker::Sync for vyre_spec::extension::ExtensionBinOpId
impl core::marker::Unpin for vyre_spec::extension::ExtensionBinOpId
impl core::marker::UnsafeUnpin for vyre_spec::extension::ExtensionBinOpId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::extension::ExtensionBinOpId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::extension::ExtensionBinOpId
impl<T, U> core::convert::Into<U> for vyre_spec::extension::ExtensionBinOpId where U: core::convert::From<T>
pub fn vyre_spec::extension::ExtensionBinOpId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::extension::ExtensionBinOpId where U: core::convert::Into<T>
pub type vyre_spec::extension::ExtensionBinOpId::Error = core::convert::Infallible
pub fn vyre_spec::extension::ExtensionBinOpId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::extension::ExtensionBinOpId where U: core::convert::TryFrom<T>
pub type vyre_spec::extension::ExtensionBinOpId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::extension::ExtensionBinOpId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::extension::ExtensionBinOpId where T: core::clone::Clone
pub type vyre_spec::extension::ExtensionBinOpId::Owned = T
pub fn vyre_spec::extension::ExtensionBinOpId::clone_into(&self, target: &mut T)
pub fn vyre_spec::extension::ExtensionBinOpId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::extension::ExtensionBinOpId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionBinOpId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::extension::ExtensionBinOpId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionBinOpId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::extension::ExtensionBinOpId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionBinOpId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::extension::ExtensionBinOpId where T: core::clone::Clone
pub unsafe fn vyre_spec::extension::ExtensionBinOpId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::extension::ExtensionBinOpId
pub fn vyre_spec::extension::ExtensionBinOpId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::extension::ExtensionBinOpId where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::extension::ExtensionDataTypeId(pub u32)
impl vyre_spec::extension::ExtensionDataTypeId
pub const vyre_spec::extension::ExtensionDataTypeId::EXTENSION_RANGE_MASK: u32
pub const fn vyre_spec::extension::ExtensionDataTypeId::as_u32(self) -> u32
pub const fn vyre_spec::extension::ExtensionDataTypeId::from_name(name: &str) -> Self
pub const fn vyre_spec::extension::ExtensionDataTypeId::is_extension(self) -> bool
impl core::clone::Clone for vyre_spec::extension::ExtensionDataTypeId
pub fn vyre_spec::extension::ExtensionDataTypeId::clone(&self) -> vyre_spec::extension::ExtensionDataTypeId
impl core::cmp::Eq for vyre_spec::extension::ExtensionDataTypeId
impl core::cmp::PartialEq for vyre_spec::extension::ExtensionDataTypeId
pub fn vyre_spec::extension::ExtensionDataTypeId::eq(&self, other: &vyre_spec::extension::ExtensionDataTypeId) -> bool
impl core::fmt::Debug for vyre_spec::extension::ExtensionDataTypeId
pub fn vyre_spec::extension::ExtensionDataTypeId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::extension::ExtensionDataTypeId
pub fn vyre_spec::extension::ExtensionDataTypeId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::extension::ExtensionDataTypeId
impl core::marker::StructuralPartialEq for vyre_spec::extension::ExtensionDataTypeId
impl serde_core::ser::Serialize for vyre_spec::extension::ExtensionDataTypeId
pub fn vyre_spec::extension::ExtensionDataTypeId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::extension::ExtensionDataTypeId
pub fn vyre_spec::extension::ExtensionDataTypeId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::extension::ExtensionDataTypeId
impl core::marker::Send for vyre_spec::extension::ExtensionDataTypeId
impl core::marker::Sync for vyre_spec::extension::ExtensionDataTypeId
impl core::marker::Unpin for vyre_spec::extension::ExtensionDataTypeId
impl core::marker::UnsafeUnpin for vyre_spec::extension::ExtensionDataTypeId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::extension::ExtensionDataTypeId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::extension::ExtensionDataTypeId
impl<T, U> core::convert::Into<U> for vyre_spec::extension::ExtensionDataTypeId where U: core::convert::From<T>
pub fn vyre_spec::extension::ExtensionDataTypeId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::extension::ExtensionDataTypeId where U: core::convert::Into<T>
pub type vyre_spec::extension::ExtensionDataTypeId::Error = core::convert::Infallible
pub fn vyre_spec::extension::ExtensionDataTypeId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::extension::ExtensionDataTypeId where U: core::convert::TryFrom<T>
pub type vyre_spec::extension::ExtensionDataTypeId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::extension::ExtensionDataTypeId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::extension::ExtensionDataTypeId where T: core::clone::Clone
pub type vyre_spec::extension::ExtensionDataTypeId::Owned = T
pub fn vyre_spec::extension::ExtensionDataTypeId::clone_into(&self, target: &mut T)
pub fn vyre_spec::extension::ExtensionDataTypeId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::extension::ExtensionDataTypeId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionDataTypeId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::extension::ExtensionDataTypeId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionDataTypeId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::extension::ExtensionDataTypeId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionDataTypeId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::extension::ExtensionDataTypeId where T: core::clone::Clone
pub unsafe fn vyre_spec::extension::ExtensionDataTypeId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::extension::ExtensionDataTypeId
pub fn vyre_spec::extension::ExtensionDataTypeId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::extension::ExtensionDataTypeId where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::extension::ExtensionRuleConditionId(pub u32)
impl vyre_spec::extension::ExtensionRuleConditionId
pub const vyre_spec::extension::ExtensionRuleConditionId::EXTENSION_RANGE_MASK: u32
pub const fn vyre_spec::extension::ExtensionRuleConditionId::as_u32(self) -> u32
pub const fn vyre_spec::extension::ExtensionRuleConditionId::from_name(name: &str) -> Self
pub const fn vyre_spec::extension::ExtensionRuleConditionId::is_extension(self) -> bool
impl core::clone::Clone for vyre_spec::extension::ExtensionRuleConditionId
pub fn vyre_spec::extension::ExtensionRuleConditionId::clone(&self) -> vyre_spec::extension::ExtensionRuleConditionId
impl core::cmp::Eq for vyre_spec::extension::ExtensionRuleConditionId
impl core::cmp::PartialEq for vyre_spec::extension::ExtensionRuleConditionId
pub fn vyre_spec::extension::ExtensionRuleConditionId::eq(&self, other: &vyre_spec::extension::ExtensionRuleConditionId) -> bool
impl core::fmt::Debug for vyre_spec::extension::ExtensionRuleConditionId
pub fn vyre_spec::extension::ExtensionRuleConditionId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::extension::ExtensionRuleConditionId
pub fn vyre_spec::extension::ExtensionRuleConditionId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::extension::ExtensionRuleConditionId
impl core::marker::StructuralPartialEq for vyre_spec::extension::ExtensionRuleConditionId
impl serde_core::ser::Serialize for vyre_spec::extension::ExtensionRuleConditionId
pub fn vyre_spec::extension::ExtensionRuleConditionId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::extension::ExtensionRuleConditionId
pub fn vyre_spec::extension::ExtensionRuleConditionId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::extension::ExtensionRuleConditionId
impl core::marker::Send for vyre_spec::extension::ExtensionRuleConditionId
impl core::marker::Sync for vyre_spec::extension::ExtensionRuleConditionId
impl core::marker::Unpin for vyre_spec::extension::ExtensionRuleConditionId
impl core::marker::UnsafeUnpin for vyre_spec::extension::ExtensionRuleConditionId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::extension::ExtensionRuleConditionId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::extension::ExtensionRuleConditionId
impl<T, U> core::convert::Into<U> for vyre_spec::extension::ExtensionRuleConditionId where U: core::convert::From<T>
pub fn vyre_spec::extension::ExtensionRuleConditionId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::extension::ExtensionRuleConditionId where U: core::convert::Into<T>
pub type vyre_spec::extension::ExtensionRuleConditionId::Error = core::convert::Infallible
pub fn vyre_spec::extension::ExtensionRuleConditionId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::extension::ExtensionRuleConditionId where U: core::convert::TryFrom<T>
pub type vyre_spec::extension::ExtensionRuleConditionId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::extension::ExtensionRuleConditionId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::extension::ExtensionRuleConditionId where T: core::clone::Clone
pub type vyre_spec::extension::ExtensionRuleConditionId::Owned = T
pub fn vyre_spec::extension::ExtensionRuleConditionId::clone_into(&self, target: &mut T)
pub fn vyre_spec::extension::ExtensionRuleConditionId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::extension::ExtensionRuleConditionId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionRuleConditionId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::extension::ExtensionRuleConditionId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionRuleConditionId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::extension::ExtensionRuleConditionId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionRuleConditionId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::extension::ExtensionRuleConditionId where T: core::clone::Clone
pub unsafe fn vyre_spec::extension::ExtensionRuleConditionId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::extension::ExtensionRuleConditionId
pub fn vyre_spec::extension::ExtensionRuleConditionId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::extension::ExtensionRuleConditionId where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::extension::ExtensionTernaryOpId(pub u32)
impl vyre_spec::extension::ExtensionTernaryOpId
pub const vyre_spec::extension::ExtensionTernaryOpId::EXTENSION_RANGE_MASK: u32
pub const fn vyre_spec::extension::ExtensionTernaryOpId::as_u32(self) -> u32
pub const fn vyre_spec::extension::ExtensionTernaryOpId::from_name(name: &str) -> Self
pub const fn vyre_spec::extension::ExtensionTernaryOpId::is_extension(self) -> bool
impl core::clone::Clone for vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_spec::extension::ExtensionTernaryOpId::clone(&self) -> vyre_spec::extension::ExtensionTernaryOpId
impl core::cmp::Eq for vyre_spec::extension::ExtensionTernaryOpId
impl core::cmp::PartialEq for vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_spec::extension::ExtensionTernaryOpId::eq(&self, other: &vyre_spec::extension::ExtensionTernaryOpId) -> bool
impl core::fmt::Debug for vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_spec::extension::ExtensionTernaryOpId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_spec::extension::ExtensionTernaryOpId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::extension::ExtensionTernaryOpId
impl core::marker::StructuralPartialEq for vyre_spec::extension::ExtensionTernaryOpId
impl serde_core::ser::Serialize for vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_spec::extension::ExtensionTernaryOpId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_spec::extension::ExtensionTernaryOpId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::extension::ExtensionTernaryOpId
impl core::marker::Send for vyre_spec::extension::ExtensionTernaryOpId
impl core::marker::Sync for vyre_spec::extension::ExtensionTernaryOpId
impl core::marker::Unpin for vyre_spec::extension::ExtensionTernaryOpId
impl core::marker::UnsafeUnpin for vyre_spec::extension::ExtensionTernaryOpId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::extension::ExtensionTernaryOpId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::extension::ExtensionTernaryOpId
impl<T, U> core::convert::Into<U> for vyre_spec::extension::ExtensionTernaryOpId where U: core::convert::From<T>
pub fn vyre_spec::extension::ExtensionTernaryOpId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::extension::ExtensionTernaryOpId where U: core::convert::Into<T>
pub type vyre_spec::extension::ExtensionTernaryOpId::Error = core::convert::Infallible
pub fn vyre_spec::extension::ExtensionTernaryOpId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::extension::ExtensionTernaryOpId where U: core::convert::TryFrom<T>
pub type vyre_spec::extension::ExtensionTernaryOpId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::extension::ExtensionTernaryOpId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::extension::ExtensionTernaryOpId where T: core::clone::Clone
pub type vyre_spec::extension::ExtensionTernaryOpId::Owned = T
pub fn vyre_spec::extension::ExtensionTernaryOpId::clone_into(&self, target: &mut T)
pub fn vyre_spec::extension::ExtensionTernaryOpId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::extension::ExtensionTernaryOpId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionTernaryOpId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::extension::ExtensionTernaryOpId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionTernaryOpId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::extension::ExtensionTernaryOpId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionTernaryOpId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::extension::ExtensionTernaryOpId where T: core::clone::Clone
pub unsafe fn vyre_spec::extension::ExtensionTernaryOpId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_spec::extension::ExtensionTernaryOpId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::extension::ExtensionTernaryOpId where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::extension::ExtensionUnOpId(pub u32)
impl vyre_spec::extension::ExtensionUnOpId
pub const vyre_spec::extension::ExtensionUnOpId::EXTENSION_RANGE_MASK: u32
pub const fn vyre_spec::extension::ExtensionUnOpId::as_u32(self) -> u32
pub const fn vyre_spec::extension::ExtensionUnOpId::from_name(name: &str) -> Self
pub const fn vyre_spec::extension::ExtensionUnOpId::is_extension(self) -> bool
impl core::clone::Clone for vyre_spec::extension::ExtensionUnOpId
pub fn vyre_spec::extension::ExtensionUnOpId::clone(&self) -> vyre_spec::extension::ExtensionUnOpId
impl core::cmp::Eq for vyre_spec::extension::ExtensionUnOpId
impl core::cmp::PartialEq for vyre_spec::extension::ExtensionUnOpId
pub fn vyre_spec::extension::ExtensionUnOpId::eq(&self, other: &vyre_spec::extension::ExtensionUnOpId) -> bool
impl core::fmt::Debug for vyre_spec::extension::ExtensionUnOpId
pub fn vyre_spec::extension::ExtensionUnOpId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::extension::ExtensionUnOpId
pub fn vyre_spec::extension::ExtensionUnOpId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::extension::ExtensionUnOpId
impl core::marker::StructuralPartialEq for vyre_spec::extension::ExtensionUnOpId
impl serde_core::ser::Serialize for vyre_spec::extension::ExtensionUnOpId
pub fn vyre_spec::extension::ExtensionUnOpId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::extension::ExtensionUnOpId
pub fn vyre_spec::extension::ExtensionUnOpId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::extension::ExtensionUnOpId
impl core::marker::Send for vyre_spec::extension::ExtensionUnOpId
impl core::marker::Sync for vyre_spec::extension::ExtensionUnOpId
impl core::marker::Unpin for vyre_spec::extension::ExtensionUnOpId
impl core::marker::UnsafeUnpin for vyre_spec::extension::ExtensionUnOpId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::extension::ExtensionUnOpId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::extension::ExtensionUnOpId
impl<T, U> core::convert::Into<U> for vyre_spec::extension::ExtensionUnOpId where U: core::convert::From<T>
pub fn vyre_spec::extension::ExtensionUnOpId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::extension::ExtensionUnOpId where U: core::convert::Into<T>
pub type vyre_spec::extension::ExtensionUnOpId::Error = core::convert::Infallible
pub fn vyre_spec::extension::ExtensionUnOpId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::extension::ExtensionUnOpId where U: core::convert::TryFrom<T>
pub type vyre_spec::extension::ExtensionUnOpId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::extension::ExtensionUnOpId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::extension::ExtensionUnOpId where T: core::clone::Clone
pub type vyre_spec::extension::ExtensionUnOpId::Owned = T
pub fn vyre_spec::extension::ExtensionUnOpId::clone_into(&self, target: &mut T)
pub fn vyre_spec::extension::ExtensionUnOpId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::extension::ExtensionUnOpId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionUnOpId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::extension::ExtensionUnOpId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionUnOpId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::extension::ExtensionUnOpId where T: ?core::marker::Sized
pub fn vyre_spec::extension::ExtensionUnOpId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::extension::ExtensionUnOpId where T: core::clone::Clone
pub unsafe fn vyre_spec::extension::ExtensionUnOpId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::extension::ExtensionUnOpId
pub fn vyre_spec::extension::ExtensionUnOpId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::extension::ExtensionUnOpId where T: for<'de> serde_core::de::Deserialize<'de>
pub trait vyre_spec::extension::ExtensionAtomicOp: core::marker::Send + core::marker::Sync + core::fmt::Debug + 'static
pub fn vyre_spec::extension::ExtensionAtomicOp::display_name(&self) -> &'static str
pub fn vyre_spec::extension::ExtensionAtomicOp::id(&self) -> vyre_spec::extension::ExtensionAtomicOpId
pub trait vyre_spec::extension::ExtensionBinOp: core::marker::Send + core::marker::Sync + core::fmt::Debug + 'static
pub fn vyre_spec::extension::ExtensionBinOp::display_name(&self) -> &'static str
pub fn vyre_spec::extension::ExtensionBinOp::eval_u32(&self, _a: u32, _b: u32) -> core::option::Option<u32>
pub fn vyre_spec::extension::ExtensionBinOp::id(&self) -> vyre_spec::extension::ExtensionBinOpId
pub trait vyre_spec::extension::ExtensionDataType: core::marker::Send + core::marker::Sync + core::fmt::Debug + 'static
pub fn vyre_spec::extension::ExtensionDataType::display_name(&self) -> &'static str
pub fn vyre_spec::extension::ExtensionDataType::id(&self) -> vyre_spec::extension::ExtensionDataTypeId
pub fn vyre_spec::extension::ExtensionDataType::is_float_family(&self) -> bool
pub fn vyre_spec::extension::ExtensionDataType::is_host_shareable(&self) -> bool
pub fn vyre_spec::extension::ExtensionDataType::max_bytes(&self) -> core::option::Option<usize>
pub fn vyre_spec::extension::ExtensionDataType::min_bytes(&self) -> usize
pub fn vyre_spec::extension::ExtensionDataType::size_bytes(&self) -> core::option::Option<usize>
pub trait vyre_spec::extension::ExtensionTernaryOp: core::marker::Send + core::marker::Sync + core::fmt::Debug + 'static
pub fn vyre_spec::extension::ExtensionTernaryOp::display_name(&self) -> &'static str
pub fn vyre_spec::extension::ExtensionTernaryOp::id(&self) -> vyre_spec::extension::ExtensionTernaryOpId
pub trait vyre_spec::extension::ExtensionUnOp: core::marker::Send + core::marker::Sync + core::fmt::Debug + 'static
pub fn vyre_spec::extension::ExtensionUnOp::display_name(&self) -> &'static str
pub fn vyre_spec::extension::ExtensionUnOp::eval_u32(&self, _a: u32) -> core::option::Option<u32>
pub fn vyre_spec::extension::ExtensionUnOp::id(&self) -> vyre_spec::extension::ExtensionUnOpId
pub mod vyre_spec::float_type
#[non_exhaustive] pub enum vyre_spec::float_type::FloatType
pub vyre_spec::float_type::FloatType::BF16
pub vyre_spec::float_type::FloatType::F16
pub vyre_spec::float_type::FloatType::F32
impl core::clone::Clone for vyre_spec::float_type::FloatType
pub fn vyre_spec::float_type::FloatType::clone(&self) -> vyre_spec::float_type::FloatType
impl core::cmp::Eq for vyre_spec::float_type::FloatType
impl core::cmp::PartialEq for vyre_spec::float_type::FloatType
pub fn vyre_spec::float_type::FloatType::eq(&self, other: &vyre_spec::float_type::FloatType) -> bool
impl core::fmt::Debug for vyre_spec::float_type::FloatType
pub fn vyre_spec::float_type::FloatType::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::float_type::FloatType
impl core::marker::Freeze for vyre_spec::float_type::FloatType
impl core::marker::Send for vyre_spec::float_type::FloatType
impl core::marker::Sync for vyre_spec::float_type::FloatType
impl core::marker::Unpin for vyre_spec::float_type::FloatType
impl core::marker::UnsafeUnpin for vyre_spec::float_type::FloatType
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::float_type::FloatType
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::float_type::FloatType
impl<T, U> core::convert::Into<U> for vyre_spec::float_type::FloatType where U: core::convert::From<T>
pub fn vyre_spec::float_type::FloatType::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::float_type::FloatType where U: core::convert::Into<T>
pub type vyre_spec::float_type::FloatType::Error = core::convert::Infallible
pub fn vyre_spec::float_type::FloatType::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::float_type::FloatType where U: core::convert::TryFrom<T>
pub type vyre_spec::float_type::FloatType::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::float_type::FloatType::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::float_type::FloatType where T: core::clone::Clone
pub type vyre_spec::float_type::FloatType::Owned = T
pub fn vyre_spec::float_type::FloatType::clone_into(&self, target: &mut T)
pub fn vyre_spec::float_type::FloatType::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::float_type::FloatType where T: 'static + ?core::marker::Sized
pub fn vyre_spec::float_type::FloatType::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::float_type::FloatType where T: ?core::marker::Sized
pub fn vyre_spec::float_type::FloatType::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::float_type::FloatType where T: ?core::marker::Sized
pub fn vyre_spec::float_type::FloatType::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::float_type::FloatType where T: core::clone::Clone
pub unsafe fn vyre_spec::float_type::FloatType::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::float_type::FloatType
pub fn vyre_spec::float_type::FloatType::from(t: T) -> T
pub mod vyre_spec::golden_sample
pub struct vyre_spec::golden_sample::GoldenSample
pub vyre_spec::golden_sample::GoldenSample::expected: &'static [u8]
pub vyre_spec::golden_sample::GoldenSample::input: &'static [u8]
pub vyre_spec::golden_sample::GoldenSample::op_id: &'static str
pub vyre_spec::golden_sample::GoldenSample::reason: &'static str
impl core::clone::Clone for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::clone(&self) -> vyre_spec::golden_sample::GoldenSample
impl core::cmp::Eq for vyre_spec::golden_sample::GoldenSample
impl core::cmp::PartialEq for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::eq(&self, other: &vyre_spec::golden_sample::GoldenSample) -> bool
impl core::fmt::Debug for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::golden_sample::GoldenSample
impl core::marker::Freeze for vyre_spec::golden_sample::GoldenSample
impl core::marker::Send for vyre_spec::golden_sample::GoldenSample
impl core::marker::Sync for vyre_spec::golden_sample::GoldenSample
impl core::marker::Unpin for vyre_spec::golden_sample::GoldenSample
impl core::marker::UnsafeUnpin for vyre_spec::golden_sample::GoldenSample
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::golden_sample::GoldenSample
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::golden_sample::GoldenSample
impl<T, U> core::convert::Into<U> for vyre_spec::golden_sample::GoldenSample where U: core::convert::From<T>
pub fn vyre_spec::golden_sample::GoldenSample::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::golden_sample::GoldenSample where U: core::convert::Into<T>
pub type vyre_spec::golden_sample::GoldenSample::Error = core::convert::Infallible
pub fn vyre_spec::golden_sample::GoldenSample::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::golden_sample::GoldenSample where U: core::convert::TryFrom<T>
pub type vyre_spec::golden_sample::GoldenSample::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::golden_sample::GoldenSample::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::golden_sample::GoldenSample where T: core::clone::Clone
pub type vyre_spec::golden_sample::GoldenSample::Owned = T
pub fn vyre_spec::golden_sample::GoldenSample::clone_into(&self, target: &mut T)
pub fn vyre_spec::golden_sample::GoldenSample::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::golden_sample::GoldenSample where T: 'static + ?core::marker::Sized
pub fn vyre_spec::golden_sample::GoldenSample::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::golden_sample::GoldenSample where T: ?core::marker::Sized
pub fn vyre_spec::golden_sample::GoldenSample::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::golden_sample::GoldenSample where T: ?core::marker::Sized
pub fn vyre_spec::golden_sample::GoldenSample::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::golden_sample::GoldenSample where T: core::clone::Clone
pub unsafe fn vyre_spec::golden_sample::GoldenSample::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::from(t: T) -> T
pub mod vyre_spec::intrinsic_descriptor
pub struct vyre_spec::intrinsic_descriptor::Backend
impl vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::id(&self) -> &str
pub fn vyre_spec::intrinsic_descriptor::Backend::name(&self) -> &str
pub fn vyre_spec::intrinsic_descriptor::Backend::named(id: impl core::convert::Into<vyre_spec::intrinsic_descriptor::BackendId>, name: impl core::convert::Into<alloc::sync::Arc<str>>) -> Self
pub fn vyre_spec::intrinsic_descriptor::Backend::new(id: impl core::convert::Into<vyre_spec::intrinsic_descriptor::BackendId>) -> Self
impl core::clone::Clone for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::clone(&self) -> vyre_spec::intrinsic_descriptor::Backend
impl core::cmp::Eq for vyre_spec::intrinsic_descriptor::Backend
impl core::cmp::PartialEq for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::eq(&self, other: &vyre_spec::intrinsic_descriptor::Backend) -> bool
impl core::convert::From<&str> for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::from(id: &str) -> Self
impl core::convert::From<&vyre_spec::intrinsic_descriptor::Backend> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(backend: &vyre_spec::intrinsic_descriptor::Backend) -> Self
impl core::convert::From<vyre_spec::intrinsic_descriptor::BackendId> for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::from(id: vyre_spec::intrinsic_descriptor::BackendId) -> Self
impl core::fmt::Debug for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::Freeze for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::Send for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::Sync for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::Unpin for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_descriptor::Backend
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_descriptor::Backend
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_descriptor::Backend
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_descriptor::Backend where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_descriptor::Backend::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_descriptor::Backend where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_descriptor::Backend::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_descriptor::Backend::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_descriptor::Backend where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_descriptor::Backend::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_descriptor::Backend::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_descriptor::Backend where T: core::clone::Clone
pub type vyre_spec::intrinsic_descriptor::Backend::Owned = T
pub fn vyre_spec::intrinsic_descriptor::Backend::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_descriptor::Backend::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::intrinsic_descriptor::Backend where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::Backend::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_descriptor::Backend where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::Backend::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_descriptor::Backend where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::Backend::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_descriptor::Backend where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_descriptor::Backend::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::from(t: T) -> T
pub struct vyre_spec::intrinsic_descriptor::BackendId(_)
impl vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::as_str(&self) -> &str
pub fn vyre_spec::intrinsic_descriptor::BackendId::new(name: impl core::convert::Into<alloc::sync::Arc<str>>) -> Self
impl core::clone::Clone for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::clone(&self) -> vyre_spec::intrinsic_descriptor::BackendId
impl core::cmp::Eq for vyre_spec::intrinsic_descriptor::BackendId
impl core::cmp::PartialEq for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::eq(&self, other: &Self) -> bool
impl core::convert::From<&str> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(name: &str) -> Self
impl core::convert::From<&vyre_spec::intrinsic_descriptor::Backend> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(backend: &vyre_spec::intrinsic_descriptor::Backend) -> Self
impl core::convert::From<alloc::string::String> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(name: alloc::string::String) -> Self
impl core::convert::From<vyre_spec::intrinsic_descriptor::BackendId> for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::from(id: vyre_spec::intrinsic_descriptor::BackendId) -> Self
impl core::fmt::Debug for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::hash<H: core::hash::Hasher>(&self, state: &mut H)
impl core::marker::Freeze for vyre_spec::intrinsic_descriptor::BackendId
impl core::marker::Send for vyre_spec::intrinsic_descriptor::BackendId
impl core::marker::Sync for vyre_spec::intrinsic_descriptor::BackendId
impl core::marker::Unpin for vyre_spec::intrinsic_descriptor::BackendId
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_descriptor::BackendId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_descriptor::BackendId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_descriptor::BackendId
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_descriptor::BackendId where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_descriptor::BackendId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_descriptor::BackendId where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_descriptor::BackendId::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_descriptor::BackendId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_descriptor::BackendId where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_descriptor::BackendId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_descriptor::BackendId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_descriptor::BackendId where T: core::clone::Clone
pub type vyre_spec::intrinsic_descriptor::BackendId::Owned = T
pub fn vyre_spec::intrinsic_descriptor::BackendId::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_descriptor::BackendId::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::intrinsic_descriptor::BackendId where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::BackendId::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::intrinsic_descriptor::BackendId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::BackendId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_descriptor::BackendId where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::BackendId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_descriptor::BackendId where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::BackendId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_descriptor::BackendId where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_descriptor::BackendId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(t: T) -> T
#[non_exhaustive] pub struct vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::contract(&self) -> core::option::Option<&vyre_spec::op_contract::OperationContract>
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::cpu_fn(&self) -> vyre_spec::intrinsic_descriptor::CpuFn
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::hardware(&self) -> &'static str
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::name(&self) -> &'static str
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::new(name: &'static str, hardware: &'static str, cpu_fn: vyre_spec::intrinsic_descriptor::CpuFn) -> Self
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::with_contract(name: &'static str, hardware: &'static str, cpu_fn: vyre_spec::intrinsic_descriptor::CpuFn, contract: vyre_spec::op_contract::OperationContract) -> Self
impl core::clone::Clone for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::clone(&self) -> vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::fmt::Debug for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::marker::Send for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::marker::Sync for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::marker::Unpin for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: core::clone::Clone
pub type vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::Owned = T
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::from(t: T) -> T
pub type vyre_spec::intrinsic_descriptor::CpuFn = fn(input: &[u8], output: &mut alloc::vec::Vec<u8>)
pub mod vyre_spec::intrinsic_table
pub struct vyre_spec::intrinsic_table::IntrinsicLowering
pub vyre_spec::intrinsic_table::IntrinsicLowering::backend: vyre_spec::intrinsic_descriptor::BackendId
pub vyre_spec::intrinsic_table::IntrinsicLowering::name: &'static str
impl vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::new(backend: impl core::convert::Into<vyre_spec::intrinsic_descriptor::BackendId>, name: &'static str) -> Self
impl core::clone::Clone for vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::clone(&self) -> vyre_spec::intrinsic_table::IntrinsicLowering
impl core::cmp::Eq for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::cmp::PartialEq for vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::eq(&self, other: &vyre_spec::intrinsic_table::IntrinsicLowering) -> bool
impl core::fmt::Debug for vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::Freeze for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::Send for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::Sync for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::Unpin for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_table::IntrinsicLowering
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_table::IntrinsicLowering where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_table::IntrinsicLowering where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_table::IntrinsicLowering::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_table::IntrinsicLowering where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_table::IntrinsicLowering::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_table::IntrinsicLowering where T: core::clone::Clone
pub type vyre_spec::intrinsic_table::IntrinsicLowering::Owned = T
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::intrinsic_table::IntrinsicLowering where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_table::IntrinsicLowering where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_table::IntrinsicLowering where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_table::IntrinsicLowering where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_table::IntrinsicLowering::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::from(t: T) -> T
pub struct vyre_spec::intrinsic_table::IntrinsicTable
pub vyre_spec::intrinsic_table::IntrinsicTable::lowerings: alloc::vec::Vec<vyre_spec::intrinsic_table::IntrinsicLowering>
impl vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::has_backend(&self, backend: &vyre_spec::intrinsic_descriptor::BackendId) -> bool
pub fn vyre_spec::intrinsic_table::IntrinsicTable::missing_backends<'a>(&'a self, required: &'a [vyre_spec::intrinsic_descriptor::BackendId]) -> impl core::iter::traits::iterator::Iterator<Item = &'a str> + 'a
impl core::clone::Clone for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::clone(&self) -> vyre_spec::intrinsic_table::IntrinsicTable
impl core::cmp::Eq for vyre_spec::intrinsic_table::IntrinsicTable
impl core::cmp::PartialEq for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::eq(&self, other: &vyre_spec::intrinsic_table::IntrinsicTable) -> bool
impl core::default::Default for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::default() -> vyre_spec::intrinsic_table::IntrinsicTable
impl core::fmt::Debug for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::Freeze for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::Send for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::Sync for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::Unpin for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_table::IntrinsicTable
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_table::IntrinsicTable
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_table::IntrinsicTable
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_table::IntrinsicTable where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_table::IntrinsicTable::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_table::IntrinsicTable where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_table::IntrinsicTable::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_table::IntrinsicTable::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_table::IntrinsicTable where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_table::IntrinsicTable::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_table::IntrinsicTable::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_table::IntrinsicTable where T: core::clone::Clone
pub type vyre_spec::intrinsic_table::IntrinsicTable::Owned = T
pub fn vyre_spec::intrinsic_table::IntrinsicTable::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_table::IntrinsicTable::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::intrinsic_table::IntrinsicTable where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicTable::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_table::IntrinsicTable where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicTable::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_table::IntrinsicTable where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicTable::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_table::IntrinsicTable where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_table::IntrinsicTable::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::from(t: T) -> T
pub mod vyre_spec::invariant
pub struct vyre_spec::invariant::Invariant
pub vyre_spec::invariant::Invariant::category: vyre_spec::invariant_category::InvariantCategory
pub vyre_spec::invariant::Invariant::description: &'static str
pub vyre_spec::invariant::Invariant::id: vyre_spec::engine_invariant::InvariantId
pub vyre_spec::invariant::Invariant::name: &'static str
pub vyre_spec::invariant::Invariant::test_family: fn() -> &'static [vyre_spec::test_descriptor::TestDescriptor]
impl core::clone::Clone for vyre_spec::invariant::Invariant
pub fn vyre_spec::invariant::Invariant::clone(&self) -> vyre_spec::invariant::Invariant
impl core::fmt::Debug for vyre_spec::invariant::Invariant
pub fn vyre_spec::invariant::Invariant::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_spec::invariant::Invariant
impl core::marker::Send for vyre_spec::invariant::Invariant
impl core::marker::Sync for vyre_spec::invariant::Invariant
impl core::marker::Unpin for vyre_spec::invariant::Invariant
impl core::marker::UnsafeUnpin for vyre_spec::invariant::Invariant
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::invariant::Invariant
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::invariant::Invariant
impl<T, U> core::convert::Into<U> for vyre_spec::invariant::Invariant where U: core::convert::From<T>
pub fn vyre_spec::invariant::Invariant::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::invariant::Invariant where U: core::convert::Into<T>
pub type vyre_spec::invariant::Invariant::Error = core::convert::Infallible
pub fn vyre_spec::invariant::Invariant::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::invariant::Invariant where U: core::convert::TryFrom<T>
pub type vyre_spec::invariant::Invariant::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::invariant::Invariant::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::invariant::Invariant where T: core::clone::Clone
pub type vyre_spec::invariant::Invariant::Owned = T
pub fn vyre_spec::invariant::Invariant::clone_into(&self, target: &mut T)
pub fn vyre_spec::invariant::Invariant::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::invariant::Invariant where T: 'static + ?core::marker::Sized
pub fn vyre_spec::invariant::Invariant::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::invariant::Invariant where T: ?core::marker::Sized
pub fn vyre_spec::invariant::Invariant::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::invariant::Invariant where T: ?core::marker::Sized
pub fn vyre_spec::invariant::Invariant::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::invariant::Invariant where T: core::clone::Clone
pub unsafe fn vyre_spec::invariant::Invariant::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::invariant::Invariant
pub fn vyre_spec::invariant::Invariant::from(t: T) -> T
pub mod vyre_spec::invariant_category
#[non_exhaustive] pub enum vyre_spec::invariant_category::InvariantCategory
pub vyre_spec::invariant_category::InvariantCategory::Algebra
pub vyre_spec::invariant_category::InvariantCategory::Execution
pub vyre_spec::invariant_category::InvariantCategory::Resource
pub vyre_spec::invariant_category::InvariantCategory::Stability
impl core::clone::Clone for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::clone(&self) -> vyre_spec::invariant_category::InvariantCategory
impl core::cmp::Eq for vyre_spec::invariant_category::InvariantCategory
impl core::cmp::PartialEq for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::eq(&self, other: &vyre_spec::invariant_category::InvariantCategory) -> bool
impl core::fmt::Debug for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::invariant_category::InvariantCategory
impl core::marker::StructuralPartialEq for vyre_spec::invariant_category::InvariantCategory
impl core::marker::Freeze for vyre_spec::invariant_category::InvariantCategory
impl core::marker::Send for vyre_spec::invariant_category::InvariantCategory
impl core::marker::Sync for vyre_spec::invariant_category::InvariantCategory
impl core::marker::Unpin for vyre_spec::invariant_category::InvariantCategory
impl core::marker::UnsafeUnpin for vyre_spec::invariant_category::InvariantCategory
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::invariant_category::InvariantCategory
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::invariant_category::InvariantCategory
impl<T, U> core::convert::Into<U> for vyre_spec::invariant_category::InvariantCategory where U: core::convert::From<T>
pub fn vyre_spec::invariant_category::InvariantCategory::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::invariant_category::InvariantCategory where U: core::convert::Into<T>
pub type vyre_spec::invariant_category::InvariantCategory::Error = core::convert::Infallible
pub fn vyre_spec::invariant_category::InvariantCategory::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::invariant_category::InvariantCategory where U: core::convert::TryFrom<T>
pub type vyre_spec::invariant_category::InvariantCategory::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::invariant_category::InvariantCategory::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::invariant_category::InvariantCategory where T: core::clone::Clone
pub type vyre_spec::invariant_category::InvariantCategory::Owned = T
pub fn vyre_spec::invariant_category::InvariantCategory::clone_into(&self, target: &mut T)
pub fn vyre_spec::invariant_category::InvariantCategory::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::invariant_category::InvariantCategory where T: 'static + ?core::marker::Sized
pub fn vyre_spec::invariant_category::InvariantCategory::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::invariant_category::InvariantCategory where T: ?core::marker::Sized
pub fn vyre_spec::invariant_category::InvariantCategory::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::invariant_category::InvariantCategory where T: ?core::marker::Sized
pub fn vyre_spec::invariant_category::InvariantCategory::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::invariant_category::InvariantCategory where T: core::clone::Clone
pub unsafe fn vyre_spec::invariant_category::InvariantCategory::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::from(t: T) -> T
pub mod vyre_spec::invariants
pub fn vyre_spec::invariants::empty_test_family() -> &'static [vyre_spec::test_descriptor::TestDescriptor]
pub fn vyre_spec::invariants::invariants() -> &'static [vyre_spec::invariant::Invariant]
pub mod vyre_spec::kat_vector
pub struct vyre_spec::kat_vector::KatVector
pub vyre_spec::kat_vector::KatVector::expected: &'static [u8]
pub vyre_spec::kat_vector::KatVector::input: &'static [u8]
pub vyre_spec::kat_vector::KatVector::source: &'static str
impl core::clone::Clone for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::clone(&self) -> vyre_spec::kat_vector::KatVector
impl core::cmp::Eq for vyre_spec::kat_vector::KatVector
impl core::cmp::PartialEq for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::eq(&self, other: &vyre_spec::kat_vector::KatVector) -> bool
impl core::fmt::Debug for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::kat_vector::KatVector
impl core::marker::Freeze for vyre_spec::kat_vector::KatVector
impl core::marker::Send for vyre_spec::kat_vector::KatVector
impl core::marker::Sync for vyre_spec::kat_vector::KatVector
impl core::marker::Unpin for vyre_spec::kat_vector::KatVector
impl core::marker::UnsafeUnpin for vyre_spec::kat_vector::KatVector
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::kat_vector::KatVector
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::kat_vector::KatVector
impl<T, U> core::convert::Into<U> for vyre_spec::kat_vector::KatVector where U: core::convert::From<T>
pub fn vyre_spec::kat_vector::KatVector::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::kat_vector::KatVector where U: core::convert::Into<T>
pub type vyre_spec::kat_vector::KatVector::Error = core::convert::Infallible
pub fn vyre_spec::kat_vector::KatVector::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::kat_vector::KatVector where U: core::convert::TryFrom<T>
pub type vyre_spec::kat_vector::KatVector::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::kat_vector::KatVector::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::kat_vector::KatVector where T: core::clone::Clone
pub type vyre_spec::kat_vector::KatVector::Owned = T
pub fn vyre_spec::kat_vector::KatVector::clone_into(&self, target: &mut T)
pub fn vyre_spec::kat_vector::KatVector::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::kat_vector::KatVector where T: 'static + ?core::marker::Sized
pub fn vyre_spec::kat_vector::KatVector::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::kat_vector::KatVector where T: ?core::marker::Sized
pub fn vyre_spec::kat_vector::KatVector::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::kat_vector::KatVector where T: ?core::marker::Sized
pub fn vyre_spec::kat_vector::KatVector::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::kat_vector::KatVector where T: core::clone::Clone
pub unsafe fn vyre_spec::kat_vector::KatVector::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::from(t: T) -> T
pub mod vyre_spec::law_catalog
pub fn vyre_spec::law_catalog::law_catalog() -> &'static [&'static str]
pub mod vyre_spec::layer
#[non_exhaustive] pub enum vyre_spec::layer::Layer
pub vyre_spec::layer::Layer::L0
pub vyre_spec::layer::Layer::L1
pub vyre_spec::layer::Layer::L2
pub vyre_spec::layer::Layer::L3
pub vyre_spec::layer::Layer::L4
pub vyre_spec::layer::Layer::L5
impl vyre_spec::layer::Layer
pub const fn vyre_spec::layer::Layer::id(&self) -> &'static str
pub const fn vyre_spec::layer::Layer::layer_description(&self) -> &'static str
impl core::clone::Clone for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::clone(&self) -> vyre_spec::layer::Layer
impl core::cmp::Eq for vyre_spec::layer::Layer
impl core::cmp::PartialEq for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::eq(&self, other: &vyre_spec::layer::Layer) -> bool
impl core::fmt::Debug for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::layer::Layer
impl serde_core::ser::Serialize for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::layer::Layer
impl core::marker::Send for vyre_spec::layer::Layer
impl core::marker::Sync for vyre_spec::layer::Layer
impl core::marker::Unpin for vyre_spec::layer::Layer
impl core::marker::UnsafeUnpin for vyre_spec::layer::Layer
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::layer::Layer
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::layer::Layer
impl<T, U> core::convert::Into<U> for vyre_spec::layer::Layer where U: core::convert::From<T>
pub fn vyre_spec::layer::Layer::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::layer::Layer where U: core::convert::Into<T>
pub type vyre_spec::layer::Layer::Error = core::convert::Infallible
pub fn vyre_spec::layer::Layer::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::layer::Layer where U: core::convert::TryFrom<T>
pub type vyre_spec::layer::Layer::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::layer::Layer::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::layer::Layer where T: core::clone::Clone
pub type vyre_spec::layer::Layer::Owned = T
pub fn vyre_spec::layer::Layer::clone_into(&self, target: &mut T)
pub fn vyre_spec::layer::Layer::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::layer::Layer where T: 'static + ?core::marker::Sized
pub fn vyre_spec::layer::Layer::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::layer::Layer where T: ?core::marker::Sized
pub fn vyre_spec::layer::Layer::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::layer::Layer where T: ?core::marker::Sized
pub fn vyre_spec::layer::Layer::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::layer::Layer where T: core::clone::Clone
pub unsafe fn vyre_spec::layer::Layer::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::layer::Layer where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::metadata_category
#[non_exhaustive] pub enum vyre_spec::metadata_category::MetadataCategory
pub vyre_spec::metadata_category::MetadataCategory::A
pub vyre_spec::metadata_category::MetadataCategory::B
pub vyre_spec::metadata_category::MetadataCategory::C
pub vyre_spec::metadata_category::MetadataCategory::Unclassified
impl vyre_spec::metadata_category::MetadataCategory
pub const fn vyre_spec::metadata_category::MetadataCategory::category_id(&self) -> &'static str
impl core::clone::Clone for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::clone(&self) -> vyre_spec::metadata_category::MetadataCategory
impl core::cmp::Eq for vyre_spec::metadata_category::MetadataCategory
impl core::cmp::PartialEq for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::eq(&self, other: &vyre_spec::metadata_category::MetadataCategory) -> bool
impl core::fmt::Debug for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::metadata_category::MetadataCategory
impl serde_core::ser::Serialize for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::metadata_category::MetadataCategory
impl core::marker::Send for vyre_spec::metadata_category::MetadataCategory
impl core::marker::Sync for vyre_spec::metadata_category::MetadataCategory
impl core::marker::Unpin for vyre_spec::metadata_category::MetadataCategory
impl core::marker::UnsafeUnpin for vyre_spec::metadata_category::MetadataCategory
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::metadata_category::MetadataCategory
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::metadata_category::MetadataCategory
impl<T, U> core::convert::Into<U> for vyre_spec::metadata_category::MetadataCategory where U: core::convert::From<T>
pub fn vyre_spec::metadata_category::MetadataCategory::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::metadata_category::MetadataCategory where U: core::convert::Into<T>
pub type vyre_spec::metadata_category::MetadataCategory::Error = core::convert::Infallible
pub fn vyre_spec::metadata_category::MetadataCategory::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::metadata_category::MetadataCategory where U: core::convert::TryFrom<T>
pub type vyre_spec::metadata_category::MetadataCategory::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::metadata_category::MetadataCategory::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::metadata_category::MetadataCategory where T: core::clone::Clone
pub type vyre_spec::metadata_category::MetadataCategory::Owned = T
pub fn vyre_spec::metadata_category::MetadataCategory::clone_into(&self, target: &mut T)
pub fn vyre_spec::metadata_category::MetadataCategory::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::metadata_category::MetadataCategory where T: 'static + ?core::marker::Sized
pub fn vyre_spec::metadata_category::MetadataCategory::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::metadata_category::MetadataCategory where T: ?core::marker::Sized
pub fn vyre_spec::metadata_category::MetadataCategory::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::metadata_category::MetadataCategory where T: ?core::marker::Sized
pub fn vyre_spec::metadata_category::MetadataCategory::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::metadata_category::MetadataCategory where T: core::clone::Clone
pub unsafe fn vyre_spec::metadata_category::MetadataCategory::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::metadata_category::MetadataCategory where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::monotonic_direction
#[non_exhaustive] pub enum vyre_spec::monotonic_direction::MonotonicDirection
pub vyre_spec::monotonic_direction::MonotonicDirection::NonDecreasing
pub vyre_spec::monotonic_direction::MonotonicDirection::NonIncreasing
impl core::clone::Clone for vyre_spec::monotonic_direction::MonotonicDirection
pub fn vyre_spec::monotonic_direction::MonotonicDirection::clone(&self) -> vyre_spec::monotonic_direction::MonotonicDirection
impl core::cmp::Eq for vyre_spec::monotonic_direction::MonotonicDirection
impl core::cmp::PartialEq for vyre_spec::monotonic_direction::MonotonicDirection
pub fn vyre_spec::monotonic_direction::MonotonicDirection::eq(&self, other: &vyre_spec::monotonic_direction::MonotonicDirection) -> bool
impl core::fmt::Debug for vyre_spec::monotonic_direction::MonotonicDirection
pub fn vyre_spec::monotonic_direction::MonotonicDirection::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::StructuralPartialEq for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::Freeze for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::Send for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::Sync for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::Unpin for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::UnsafeUnpin for vyre_spec::monotonic_direction::MonotonicDirection
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::monotonic_direction::MonotonicDirection
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::monotonic_direction::MonotonicDirection
impl<T, U> core::convert::Into<U> for vyre_spec::monotonic_direction::MonotonicDirection where U: core::convert::From<T>
pub fn vyre_spec::monotonic_direction::MonotonicDirection::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::monotonic_direction::MonotonicDirection where U: core::convert::Into<T>
pub type vyre_spec::monotonic_direction::MonotonicDirection::Error = core::convert::Infallible
pub fn vyre_spec::monotonic_direction::MonotonicDirection::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::monotonic_direction::MonotonicDirection where U: core::convert::TryFrom<T>
pub type vyre_spec::monotonic_direction::MonotonicDirection::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::monotonic_direction::MonotonicDirection::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::monotonic_direction::MonotonicDirection where T: core::clone::Clone
pub type vyre_spec::monotonic_direction::MonotonicDirection::Owned = T
pub fn vyre_spec::monotonic_direction::MonotonicDirection::clone_into(&self, target: &mut T)
pub fn vyre_spec::monotonic_direction::MonotonicDirection::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::monotonic_direction::MonotonicDirection where T: 'static + ?core::marker::Sized
pub fn vyre_spec::monotonic_direction::MonotonicDirection::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::monotonic_direction::MonotonicDirection where T: ?core::marker::Sized
pub fn vyre_spec::monotonic_direction::MonotonicDirection::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::monotonic_direction::MonotonicDirection where T: ?core::marker::Sized
pub fn vyre_spec::monotonic_direction::MonotonicDirection::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::monotonic_direction::MonotonicDirection where T: core::clone::Clone
pub unsafe fn vyre_spec::monotonic_direction::MonotonicDirection::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::monotonic_direction::MonotonicDirection
pub fn vyre_spec::monotonic_direction::MonotonicDirection::from(t: T) -> T
pub mod vyre_spec::op_contract
#[non_exhaustive] pub enum vyre_spec::op_contract::CostHint
pub vyre_spec::op_contract::CostHint::Cheap
pub vyre_spec::op_contract::CostHint::Expensive
pub vyre_spec::op_contract::CostHint::Medium
pub vyre_spec::op_contract::CostHint::Unknown
impl core::clone::Clone for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::clone(&self) -> vyre_spec::op_contract::CostHint
impl core::cmp::Eq for vyre_spec::op_contract::CostHint
impl core::cmp::PartialEq for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::eq(&self, other: &vyre_spec::op_contract::CostHint) -> bool
impl core::fmt::Debug for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::op_contract::CostHint
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::CostHint
impl serde_core::ser::Serialize for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::CostHint
impl core::marker::Send for vyre_spec::op_contract::CostHint
impl core::marker::Sync for vyre_spec::op_contract::CostHint
impl core::marker::Unpin for vyre_spec::op_contract::CostHint
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::CostHint
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::CostHint
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::CostHint
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::CostHint where U: core::convert::From<T>
pub fn vyre_spec::op_contract::CostHint::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::CostHint where U: core::convert::Into<T>
pub type vyre_spec::op_contract::CostHint::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::CostHint::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::CostHint where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::CostHint::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::CostHint::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::CostHint where T: core::clone::Clone
pub type vyre_spec::op_contract::CostHint::Owned = T
pub fn vyre_spec::op_contract::CostHint::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::CostHint::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::CostHint where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::CostHint::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::CostHint where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::CostHint::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::CostHint where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::CostHint::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::CostHint where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::CostHint::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::CostHint where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::op_contract::DeterminismClass
pub vyre_spec::op_contract::DeterminismClass::Deterministic
pub vyre_spec::op_contract::DeterminismClass::DeterministicModuloRounding
pub vyre_spec::op_contract::DeterminismClass::NonDeterministic
impl core::clone::Clone for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::clone(&self) -> vyre_spec::op_contract::DeterminismClass
impl core::cmp::Eq for vyre_spec::op_contract::DeterminismClass
impl core::cmp::PartialEq for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::eq(&self, other: &vyre_spec::op_contract::DeterminismClass) -> bool
impl core::fmt::Debug for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::op_contract::DeterminismClass
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::DeterminismClass
impl serde_core::ser::Serialize for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::DeterminismClass
impl core::marker::Send for vyre_spec::op_contract::DeterminismClass
impl core::marker::Sync for vyre_spec::op_contract::DeterminismClass
impl core::marker::Unpin for vyre_spec::op_contract::DeterminismClass
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::DeterminismClass
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::DeterminismClass
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::DeterminismClass
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::DeterminismClass where U: core::convert::From<T>
pub fn vyre_spec::op_contract::DeterminismClass::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::DeterminismClass where U: core::convert::Into<T>
pub type vyre_spec::op_contract::DeterminismClass::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::DeterminismClass::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::DeterminismClass where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::DeterminismClass::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::DeterminismClass::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::DeterminismClass where T: core::clone::Clone
pub type vyre_spec::op_contract::DeterminismClass::Owned = T
pub fn vyre_spec::op_contract::DeterminismClass::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::DeterminismClass::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::DeterminismClass where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::DeterminismClass::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::DeterminismClass where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::DeterminismClass::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::DeterminismClass where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::DeterminismClass::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::DeterminismClass where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::DeterminismClass::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::DeterminismClass where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::op_contract::SideEffectClass
pub vyre_spec::op_contract::SideEffectClass::Atomic
pub vyre_spec::op_contract::SideEffectClass::Pure
pub vyre_spec::op_contract::SideEffectClass::ReadsMemory
pub vyre_spec::op_contract::SideEffectClass::Synchronizing
pub vyre_spec::op_contract::SideEffectClass::WritesMemory
impl core::clone::Clone for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::clone(&self) -> vyre_spec::op_contract::SideEffectClass
impl core::cmp::Eq for vyre_spec::op_contract::SideEffectClass
impl core::cmp::PartialEq for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::eq(&self, other: &vyre_spec::op_contract::SideEffectClass) -> bool
impl core::fmt::Debug for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::op_contract::SideEffectClass
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::SideEffectClass
impl serde_core::ser::Serialize for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::SideEffectClass
impl core::marker::Send for vyre_spec::op_contract::SideEffectClass
impl core::marker::Sync for vyre_spec::op_contract::SideEffectClass
impl core::marker::Unpin for vyre_spec::op_contract::SideEffectClass
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::SideEffectClass
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::SideEffectClass
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::SideEffectClass
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::SideEffectClass where U: core::convert::From<T>
pub fn vyre_spec::op_contract::SideEffectClass::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::SideEffectClass where U: core::convert::Into<T>
pub type vyre_spec::op_contract::SideEffectClass::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::SideEffectClass::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::SideEffectClass where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::SideEffectClass::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::SideEffectClass::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::SideEffectClass where T: core::clone::Clone
pub type vyre_spec::op_contract::SideEffectClass::Owned = T
pub fn vyre_spec::op_contract::SideEffectClass::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::SideEffectClass::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::SideEffectClass where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::SideEffectClass::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::SideEffectClass where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::SideEffectClass::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::SideEffectClass where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::SideEffectClass::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::SideEffectClass where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::SideEffectClass::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::SideEffectClass where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::op_contract::CapabilityId(pub alloc::string::String)
impl vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::as_str(&self) -> &str
pub fn vyre_spec::op_contract::CapabilityId::new(name: impl core::convert::Into<alloc::string::String>) -> Self
impl core::clone::Clone for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::clone(&self) -> vyre_spec::op_contract::CapabilityId
impl core::cmp::Eq for vyre_spec::op_contract::CapabilityId
impl core::cmp::PartialEq for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::eq(&self, other: &vyre_spec::op_contract::CapabilityId) -> bool
impl core::fmt::Debug for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::CapabilityId
impl serde_core::ser::Serialize for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::CapabilityId
impl core::marker::Send for vyre_spec::op_contract::CapabilityId
impl core::marker::Sync for vyre_spec::op_contract::CapabilityId
impl core::marker::Unpin for vyre_spec::op_contract::CapabilityId
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::CapabilityId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::CapabilityId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::CapabilityId
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::CapabilityId where U: core::convert::From<T>
pub fn vyre_spec::op_contract::CapabilityId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::CapabilityId where U: core::convert::Into<T>
pub type vyre_spec::op_contract::CapabilityId::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::CapabilityId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::CapabilityId where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::CapabilityId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::CapabilityId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::CapabilityId where T: core::clone::Clone
pub type vyre_spec::op_contract::CapabilityId::Owned = T
pub fn vyre_spec::op_contract::CapabilityId::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::CapabilityId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::CapabilityId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::CapabilityId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::CapabilityId where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::CapabilityId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::CapabilityId where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::CapabilityId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::CapabilityId where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::CapabilityId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::CapabilityId where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::op_contract::OperationContract
pub vyre_spec::op_contract::OperationContract::capability_requirements: core::option::Option<smallvec::SmallVec<[vyre_spec::op_contract::CapabilityId; 4]>>
pub vyre_spec::op_contract::OperationContract::cost_hint: core::option::Option<vyre_spec::op_contract::CostHint>
pub vyre_spec::op_contract::OperationContract::determinism: core::option::Option<vyre_spec::op_contract::DeterminismClass>
pub vyre_spec::op_contract::OperationContract::side_effect: core::option::Option<vyre_spec::op_contract::SideEffectClass>
impl vyre_spec::op_contract::OperationContract
pub const fn vyre_spec::op_contract::OperationContract::none() -> Self
impl core::clone::Clone for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::clone(&self) -> vyre_spec::op_contract::OperationContract
impl core::cmp::Eq for vyre_spec::op_contract::OperationContract
impl core::cmp::PartialEq for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::eq(&self, other: &vyre_spec::op_contract::OperationContract) -> bool
impl core::default::Default for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::default() -> Self
impl core::fmt::Debug for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::OperationContract
impl serde_core::ser::Serialize for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::OperationContract
impl core::marker::Send for vyre_spec::op_contract::OperationContract
impl core::marker::Sync for vyre_spec::op_contract::OperationContract
impl core::marker::Unpin for vyre_spec::op_contract::OperationContract
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::OperationContract
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::OperationContract
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::OperationContract
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::OperationContract where U: core::convert::From<T>
pub fn vyre_spec::op_contract::OperationContract::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::OperationContract where U: core::convert::Into<T>
pub type vyre_spec::op_contract::OperationContract::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::OperationContract::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::OperationContract where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::OperationContract::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::OperationContract::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::OperationContract where T: core::clone::Clone
pub type vyre_spec::op_contract::OperationContract::Owned = T
pub fn vyre_spec::op_contract::OperationContract::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::OperationContract::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::OperationContract where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::OperationContract::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::OperationContract where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::OperationContract::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::OperationContract where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::OperationContract::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::OperationContract where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::OperationContract::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::OperationContract where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::op_metadata
pub struct vyre_spec::op_metadata::OpMetadata
pub vyre_spec::op_metadata::OpMetadata::archetype_signature: &'static str
pub vyre_spec::op_metadata::OpMetadata::category: vyre_spec::metadata_category::MetadataCategory
pub vyre_spec::op_metadata::OpMetadata::contract: core::option::Option<vyre_spec::op_contract::OperationContract>
pub vyre_spec::op_metadata::OpMetadata::description: &'static str
pub vyre_spec::op_metadata::OpMetadata::id: &'static str
pub vyre_spec::op_metadata::OpMetadata::layer: vyre_spec::layer::Layer
pub vyre_spec::op_metadata::OpMetadata::signature: &'static str
pub vyre_spec::op_metadata::OpMetadata::strictness: &'static str
pub vyre_spec::op_metadata::OpMetadata::version: u32
impl core::clone::Clone for vyre_spec::op_metadata::OpMetadata
pub fn vyre_spec::op_metadata::OpMetadata::clone(&self) -> vyre_spec::op_metadata::OpMetadata
impl core::cmp::Eq for vyre_spec::op_metadata::OpMetadata
impl core::cmp::PartialEq for vyre_spec::op_metadata::OpMetadata
pub fn vyre_spec::op_metadata::OpMetadata::eq(&self, other: &vyre_spec::op_metadata::OpMetadata) -> bool
impl core::fmt::Debug for vyre_spec::op_metadata::OpMetadata
pub fn vyre_spec::op_metadata::OpMetadata::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::op_metadata::OpMetadata
impl core::marker::Freeze for vyre_spec::op_metadata::OpMetadata
impl core::marker::Send for vyre_spec::op_metadata::OpMetadata
impl core::marker::Sync for vyre_spec::op_metadata::OpMetadata
impl core::marker::Unpin for vyre_spec::op_metadata::OpMetadata
impl core::marker::UnsafeUnpin for vyre_spec::op_metadata::OpMetadata
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_metadata::OpMetadata
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_metadata::OpMetadata
impl<T, U> core::convert::Into<U> for vyre_spec::op_metadata::OpMetadata where U: core::convert::From<T>
pub fn vyre_spec::op_metadata::OpMetadata::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_metadata::OpMetadata where U: core::convert::Into<T>
pub type vyre_spec::op_metadata::OpMetadata::Error = core::convert::Infallible
pub fn vyre_spec::op_metadata::OpMetadata::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_metadata::OpMetadata where U: core::convert::TryFrom<T>
pub type vyre_spec::op_metadata::OpMetadata::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_metadata::OpMetadata::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_metadata::OpMetadata where T: core::clone::Clone
pub type vyre_spec::op_metadata::OpMetadata::Owned = T
pub fn vyre_spec::op_metadata::OpMetadata::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_metadata::OpMetadata::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_metadata::OpMetadata where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_metadata::OpMetadata::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_metadata::OpMetadata where T: ?core::marker::Sized
pub fn vyre_spec::op_metadata::OpMetadata::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_metadata::OpMetadata where T: ?core::marker::Sized
pub fn vyre_spec::op_metadata::OpMetadata::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_metadata::OpMetadata where T: core::clone::Clone
pub unsafe fn vyre_spec::op_metadata::OpMetadata::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_metadata::OpMetadata
pub fn vyre_spec::op_metadata::OpMetadata::from(t: T) -> T
pub mod vyre_spec::op_signature
pub struct vyre_spec::op_signature::OpSignature
pub vyre_spec::op_signature::OpSignature::contract: core::option::Option<vyre_spec::op_contract::OperationContract>
pub vyre_spec::op_signature::OpSignature::input_params: core::option::Option<alloc::vec::Vec<vyre_spec::op_signature::SignatureParam>>
pub vyre_spec::op_signature::OpSignature::inputs: alloc::vec::Vec<vyre_spec::data_type::DataType>
pub vyre_spec::op_signature::OpSignature::output: vyre_spec::data_type::DataType
pub vyre_spec::op_signature::OpSignature::output_params: core::option::Option<alloc::vec::Vec<vyre_spec::op_signature::SignatureParam>>
impl vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::min_input_bytes(&self) -> usize
impl core::clone::Clone for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::clone(&self) -> vyre_spec::op_signature::OpSignature
impl core::cmp::Eq for vyre_spec::op_signature::OpSignature
impl core::cmp::PartialEq for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::eq(&self, other: &vyre_spec::op_signature::OpSignature) -> bool
impl core::fmt::Debug for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::op_signature::OpSignature
impl serde_core::ser::Serialize for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_signature::OpSignature
impl core::marker::Send for vyre_spec::op_signature::OpSignature
impl core::marker::Sync for vyre_spec::op_signature::OpSignature
impl core::marker::Unpin for vyre_spec::op_signature::OpSignature
impl core::marker::UnsafeUnpin for vyre_spec::op_signature::OpSignature
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_signature::OpSignature
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_signature::OpSignature
impl<T, U> core::convert::Into<U> for vyre_spec::op_signature::OpSignature where U: core::convert::From<T>
pub fn vyre_spec::op_signature::OpSignature::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_signature::OpSignature where U: core::convert::Into<T>
pub type vyre_spec::op_signature::OpSignature::Error = core::convert::Infallible
pub fn vyre_spec::op_signature::OpSignature::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_signature::OpSignature where U: core::convert::TryFrom<T>
pub type vyre_spec::op_signature::OpSignature::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_signature::OpSignature::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_signature::OpSignature where T: core::clone::Clone
pub type vyre_spec::op_signature::OpSignature::Owned = T
pub fn vyre_spec::op_signature::OpSignature::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_signature::OpSignature::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_signature::OpSignature where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_signature::OpSignature::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_signature::OpSignature where T: ?core::marker::Sized
pub fn vyre_spec::op_signature::OpSignature::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_signature::OpSignature where T: ?core::marker::Sized
pub fn vyre_spec::op_signature::OpSignature::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_signature::OpSignature where T: core::clone::Clone
pub unsafe fn vyre_spec::op_signature::OpSignature::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_signature::OpSignature where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::op_signature::SignatureParam
pub vyre_spec::op_signature::SignatureParam::metadata: core::option::Option<alloc::string::String>
pub vyre_spec::op_signature::SignatureParam::name: alloc::string::String
pub vyre_spec::op_signature::SignatureParam::ty: vyre_spec::data_type::DataType
impl core::clone::Clone for vyre_spec::op_signature::SignatureParam
pub fn vyre_spec::op_signature::SignatureParam::clone(&self) -> vyre_spec::op_signature::SignatureParam
impl core::cmp::Eq for vyre_spec::op_signature::SignatureParam
impl core::cmp::PartialEq for vyre_spec::op_signature::SignatureParam
pub fn vyre_spec::op_signature::SignatureParam::eq(&self, other: &vyre_spec::op_signature::SignatureParam) -> bool
impl core::fmt::Debug for vyre_spec::op_signature::SignatureParam
pub fn vyre_spec::op_signature::SignatureParam::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_signature::SignatureParam
pub fn vyre_spec::op_signature::SignatureParam::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::op_signature::SignatureParam
impl serde_core::ser::Serialize for vyre_spec::op_signature::SignatureParam
pub fn vyre_spec::op_signature::SignatureParam::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_signature::SignatureParam
pub fn vyre_spec::op_signature::SignatureParam::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_signature::SignatureParam
impl core::marker::Send for vyre_spec::op_signature::SignatureParam
impl core::marker::Sync for vyre_spec::op_signature::SignatureParam
impl core::marker::Unpin for vyre_spec::op_signature::SignatureParam
impl core::marker::UnsafeUnpin for vyre_spec::op_signature::SignatureParam
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_signature::SignatureParam
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_signature::SignatureParam
impl<T, U> core::convert::Into<U> for vyre_spec::op_signature::SignatureParam where U: core::convert::From<T>
pub fn vyre_spec::op_signature::SignatureParam::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_signature::SignatureParam where U: core::convert::Into<T>
pub type vyre_spec::op_signature::SignatureParam::Error = core::convert::Infallible
pub fn vyre_spec::op_signature::SignatureParam::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_signature::SignatureParam where U: core::convert::TryFrom<T>
pub type vyre_spec::op_signature::SignatureParam::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_signature::SignatureParam::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_signature::SignatureParam where T: core::clone::Clone
pub type vyre_spec::op_signature::SignatureParam::Owned = T
pub fn vyre_spec::op_signature::SignatureParam::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_signature::SignatureParam::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_signature::SignatureParam where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_signature::SignatureParam::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_signature::SignatureParam where T: ?core::marker::Sized
pub fn vyre_spec::op_signature::SignatureParam::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_signature::SignatureParam where T: ?core::marker::Sized
pub fn vyre_spec::op_signature::SignatureParam::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_signature::SignatureParam where T: core::clone::Clone
pub unsafe fn vyre_spec::op_signature::SignatureParam::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_signature::SignatureParam
pub fn vyre_spec::op_signature::SignatureParam::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_signature::SignatureParam where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::pg_node_kind
#[non_exhaustive] #[repr(u32)] pub enum vyre_spec::pg_node_kind::PgNodeKind
pub vyre_spec::pg_node_kind::PgNodeKind::AddrOf = 13
pub vyre_spec::pg_node_kind::PgNodeKind::ArrayAccess = 16
pub vyre_spec::pg_node_kind::PgNodeKind::Assignment = 3
pub vyre_spec::pg_node_kind::PgNodeKind::Binary = 4
pub vyre_spec::pg_node_kind::PgNodeKind::Cast = 14
pub vyre_spec::pg_node_kind::PgNodeKind::Comparison = 5
pub vyre_spec::pg_node_kind::PgNodeKind::Deref = 12
pub vyre_spec::pg_node_kind::PgNodeKind::ForStmt = 9
pub vyre_spec::pg_node_kind::PgNodeKind::FunctionCall = 6
pub vyre_spec::pg_node_kind::PgNodeKind::FunctionDef = 7
pub vyre_spec::pg_node_kind::PgNodeKind::IfStmt = 8
pub vyre_spec::pg_node_kind::PgNodeKind::LiteralFloat = 20
pub vyre_spec::pg_node_kind::PgNodeKind::LiteralInt = 18
pub vyre_spec::pg_node_kind::PgNodeKind::LiteralStr = 19
pub vyre_spec::pg_node_kind::PgNodeKind::MemberAccess = 15
pub vyre_spec::pg_node_kind::PgNodeKind::ReturnStmt = 11
pub vyre_spec::pg_node_kind::PgNodeKind::StructDecl = 17
pub vyre_spec::pg_node_kind::PgNodeKind::VariableDecl = 1
pub vyre_spec::pg_node_kind::PgNodeKind::VariableUse = 2
pub vyre_spec::pg_node_kind::PgNodeKind::WhileStmt = 10
impl vyre_spec::pg_node_kind::PgNodeKind
pub const fn vyre_spec::pg_node_kind::PgNodeKind::from_u32(value: u32) -> core::option::Option<Self>
impl core::clone::Clone for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::clone(&self) -> vyre_spec::pg_node_kind::PgNodeKind
impl core::cmp::Eq for vyre_spec::pg_node_kind::PgNodeKind
impl core::cmp::PartialEq for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::eq(&self, other: &vyre_spec::pg_node_kind::PgNodeKind) -> bool
impl core::fmt::Debug for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::StructuralPartialEq for vyre_spec::pg_node_kind::PgNodeKind
impl serde_core::ser::Serialize for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::Send for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::Sync for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::Unpin for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::UnsafeUnpin for vyre_spec::pg_node_kind::PgNodeKind
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::pg_node_kind::PgNodeKind
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::pg_node_kind::PgNodeKind
impl<T, U> core::convert::Into<U> for vyre_spec::pg_node_kind::PgNodeKind where U: core::convert::From<T>
pub fn vyre_spec::pg_node_kind::PgNodeKind::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::pg_node_kind::PgNodeKind where U: core::convert::Into<T>
pub type vyre_spec::pg_node_kind::PgNodeKind::Error = core::convert::Infallible
pub fn vyre_spec::pg_node_kind::PgNodeKind::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::pg_node_kind::PgNodeKind where U: core::convert::TryFrom<T>
pub type vyre_spec::pg_node_kind::PgNodeKind::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::pg_node_kind::PgNodeKind::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::pg_node_kind::PgNodeKind where T: core::clone::Clone
pub type vyre_spec::pg_node_kind::PgNodeKind::Owned = T
pub fn vyre_spec::pg_node_kind::PgNodeKind::clone_into(&self, target: &mut T)
pub fn vyre_spec::pg_node_kind::PgNodeKind::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::pg_node_kind::PgNodeKind where T: 'static + ?core::marker::Sized
pub fn vyre_spec::pg_node_kind::PgNodeKind::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::pg_node_kind::PgNodeKind where T: ?core::marker::Sized
pub fn vyre_spec::pg_node_kind::PgNodeKind::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::pg_node_kind::PgNodeKind where T: ?core::marker::Sized
pub fn vyre_spec::pg_node_kind::PgNodeKind::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::pg_node_kind::PgNodeKind where T: core::clone::Clone
pub unsafe fn vyre_spec::pg_node_kind::PgNodeKind::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::pg_node_kind::PgNodeKind where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::semiring
pub enum vyre_spec::semiring::Semiring
pub vyre_spec::semiring::Semiring::BoolAnd
pub vyre_spec::semiring::Semiring::BoolOr
pub vyre_spec::semiring::Semiring::Gf2
pub vyre_spec::semiring::Semiring::Lineage
pub vyre_spec::semiring::Semiring::MaxPlus
pub vyre_spec::semiring::Semiring::MaxTimes
pub vyre_spec::semiring::Semiring::MinPlus
pub vyre_spec::semiring::Semiring::Real
impl vyre_spec::semiring::Semiring
pub const fn vyre_spec::semiring::Semiring::identity(self) -> u32
impl core::clone::Clone for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::clone(&self) -> vyre_spec::semiring::Semiring
impl core::cmp::Eq for vyre_spec::semiring::Semiring
impl core::cmp::PartialEq for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::eq(&self, other: &vyre_spec::semiring::Semiring) -> bool
impl core::fmt::Debug for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::semiring::Semiring
impl core::marker::StructuralPartialEq for vyre_spec::semiring::Semiring
impl serde_core::ser::Serialize for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::semiring::Semiring
impl core::marker::Send for vyre_spec::semiring::Semiring
impl core::marker::Sync for vyre_spec::semiring::Semiring
impl core::marker::Unpin for vyre_spec::semiring::Semiring
impl core::marker::UnsafeUnpin for vyre_spec::semiring::Semiring
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::semiring::Semiring
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::semiring::Semiring
impl<T, U> core::convert::Into<U> for vyre_spec::semiring::Semiring where U: core::convert::From<T>
pub fn vyre_spec::semiring::Semiring::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::semiring::Semiring where U: core::convert::Into<T>
pub type vyre_spec::semiring::Semiring::Error = core::convert::Infallible
pub fn vyre_spec::semiring::Semiring::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::semiring::Semiring where U: core::convert::TryFrom<T>
pub type vyre_spec::semiring::Semiring::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::semiring::Semiring::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::semiring::Semiring where T: core::clone::Clone
pub type vyre_spec::semiring::Semiring::Owned = T
pub fn vyre_spec::semiring::Semiring::clone_into(&self, target: &mut T)
pub fn vyre_spec::semiring::Semiring::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::semiring::Semiring where T: 'static + ?core::marker::Sized
pub fn vyre_spec::semiring::Semiring::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::semiring::Semiring where T: ?core::marker::Sized
pub fn vyre_spec::semiring::Semiring::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::semiring::Semiring where T: ?core::marker::Sized
pub fn vyre_spec::semiring::Semiring::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::semiring::Semiring where T: core::clone::Clone
pub unsafe fn vyre_spec::semiring::Semiring::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::semiring::Semiring where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::subgroup_reduce_op
#[non_exhaustive] pub enum vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Add
pub vyre_spec::subgroup_reduce_op::SubgroupReduceOp::And
pub vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Max
pub vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Min
pub vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Mul
pub vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Or
pub vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Xor
impl vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub const vyre_spec::subgroup_reduce_op::SubgroupReduceOp::ALL: [Self; 7]
pub const fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::as_str(self) -> &'static str
pub const fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::builtin_wire_tag(self) -> u8
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::combine_f32(self, acc: f32, lane: f32) -> core::option::Option<f32>
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::f32_identity(self) -> core::option::Option<f32>
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::from_wire_tag(tag: u8) -> core::result::Result<Self, alloc::string::String>
pub const fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::is_bitwise(self) -> bool
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::reduce_u32(self, lanes: impl core::iter::traits::collect::IntoIterator<Item = u32>) -> u32
impl core::clone::Clone for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::clone(&self) -> vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::cmp::Eq for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::cmp::PartialEq for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::eq(&self, other: &vyre_spec::subgroup_reduce_op::SubgroupReduceOp) -> bool
impl core::fmt::Debug for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::StructuralPartialEq for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl serde_core::ser::Serialize for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::Send for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::Sync for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::Unpin for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::UnsafeUnpin for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl<T, U> core::convert::Into<U> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where U: core::convert::From<T>
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where U: core::convert::Into<T>
pub type vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Error = core::convert::Infallible
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where U: core::convert::TryFrom<T>
pub type vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: core::clone::Clone
pub type vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Owned = T
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: ?core::marker::Sized
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: ?core::marker::Sized
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: core::clone::Clone
pub unsafe fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::ternary_op
#[non_exhaustive] pub enum vyre_spec::ternary_op::TernaryOp
pub vyre_spec::ternary_op::TernaryOp::Fma
pub vyre_spec::ternary_op::TernaryOp::Opaque(vyre_spec::extension::ExtensionTernaryOpId)
pub vyre_spec::ternary_op::TernaryOp::Select
impl vyre_spec::ternary_op::TernaryOp
pub const fn vyre_spec::ternary_op::TernaryOp::builtin_wire_tag(&self) -> core::option::Option<u8>
impl core::clone::Clone for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::clone(&self) -> vyre_spec::ternary_op::TernaryOp
impl core::cmp::Eq for vyre_spec::ternary_op::TernaryOp
impl core::cmp::PartialEq for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::eq(&self, other: &vyre_spec::ternary_op::TernaryOp) -> bool
impl core::fmt::Debug for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::ternary_op::TernaryOp
impl core::marker::StructuralPartialEq for vyre_spec::ternary_op::TernaryOp
impl serde_core::ser::Serialize for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::ternary_op::TernaryOp
impl core::marker::Send for vyre_spec::ternary_op::TernaryOp
impl core::marker::Sync for vyre_spec::ternary_op::TernaryOp
impl core::marker::Unpin for vyre_spec::ternary_op::TernaryOp
impl core::marker::UnsafeUnpin for vyre_spec::ternary_op::TernaryOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::ternary_op::TernaryOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::ternary_op::TernaryOp
impl<T, U> core::convert::Into<U> for vyre_spec::ternary_op::TernaryOp where U: core::convert::From<T>
pub fn vyre_spec::ternary_op::TernaryOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::ternary_op::TernaryOp where U: core::convert::Into<T>
pub type vyre_spec::ternary_op::TernaryOp::Error = core::convert::Infallible
pub fn vyre_spec::ternary_op::TernaryOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::ternary_op::TernaryOp where U: core::convert::TryFrom<T>
pub type vyre_spec::ternary_op::TernaryOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::ternary_op::TernaryOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::ternary_op::TernaryOp where T: core::clone::Clone
pub type vyre_spec::ternary_op::TernaryOp::Owned = T
pub fn vyre_spec::ternary_op::TernaryOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::ternary_op::TernaryOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::ternary_op::TernaryOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::ternary_op::TernaryOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::ternary_op::TernaryOp where T: ?core::marker::Sized
pub fn vyre_spec::ternary_op::TernaryOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::ternary_op::TernaryOp where T: ?core::marker::Sized
pub fn vyre_spec::ternary_op::TernaryOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::ternary_op::TernaryOp where T: core::clone::Clone
pub unsafe fn vyre_spec::ternary_op::TernaryOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::ternary_op::TernaryOp where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::test_descriptor
pub struct vyre_spec::test_descriptor::TestDescriptor
pub vyre_spec::test_descriptor::TestDescriptor::invariant: vyre_spec::engine_invariant::InvariantId
pub vyre_spec::test_descriptor::TestDescriptor::name: &'static str
pub vyre_spec::test_descriptor::TestDescriptor::purpose: &'static str
impl core::clone::Clone for vyre_spec::test_descriptor::TestDescriptor
pub fn vyre_spec::test_descriptor::TestDescriptor::clone(&self) -> vyre_spec::test_descriptor::TestDescriptor
impl core::cmp::Eq for vyre_spec::test_descriptor::TestDescriptor
impl core::cmp::PartialEq for vyre_spec::test_descriptor::TestDescriptor
pub fn vyre_spec::test_descriptor::TestDescriptor::eq(&self, other: &vyre_spec::test_descriptor::TestDescriptor) -> bool
impl core::fmt::Debug for vyre_spec::test_descriptor::TestDescriptor
pub fn vyre_spec::test_descriptor::TestDescriptor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::Freeze for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::Send for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::Sync for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::Unpin for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::UnsafeUnpin for vyre_spec::test_descriptor::TestDescriptor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::test_descriptor::TestDescriptor
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::test_descriptor::TestDescriptor
impl<T, U> core::convert::Into<U> for vyre_spec::test_descriptor::TestDescriptor where U: core::convert::From<T>
pub fn vyre_spec::test_descriptor::TestDescriptor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::test_descriptor::TestDescriptor where U: core::convert::Into<T>
pub type vyre_spec::test_descriptor::TestDescriptor::Error = core::convert::Infallible
pub fn vyre_spec::test_descriptor::TestDescriptor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::test_descriptor::TestDescriptor where U: core::convert::TryFrom<T>
pub type vyre_spec::test_descriptor::TestDescriptor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::test_descriptor::TestDescriptor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::test_descriptor::TestDescriptor where T: core::clone::Clone
pub type vyre_spec::test_descriptor::TestDescriptor::Owned = T
pub fn vyre_spec::test_descriptor::TestDescriptor::clone_into(&self, target: &mut T)
pub fn vyre_spec::test_descriptor::TestDescriptor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::test_descriptor::TestDescriptor where T: 'static + ?core::marker::Sized
pub fn vyre_spec::test_descriptor::TestDescriptor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::test_descriptor::TestDescriptor where T: ?core::marker::Sized
pub fn vyre_spec::test_descriptor::TestDescriptor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::test_descriptor::TestDescriptor where T: ?core::marker::Sized
pub fn vyre_spec::test_descriptor::TestDescriptor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::test_descriptor::TestDescriptor where T: core::clone::Clone
pub unsafe fn vyre_spec::test_descriptor::TestDescriptor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::test_descriptor::TestDescriptor
pub fn vyre_spec::test_descriptor::TestDescriptor::from(t: T) -> T
pub mod vyre_spec::un_op
#[non_exhaustive] pub enum vyre_spec::un_op::UnOp
pub vyre_spec::un_op::UnOp::Abs
pub vyre_spec::un_op::UnOp::Acos
pub vyre_spec::un_op::UnOp::Asin
pub vyre_spec::un_op::UnOp::Atan
pub vyre_spec::un_op::UnOp::BitNot
pub vyre_spec::un_op::UnOp::Ceil
pub vyre_spec::un_op::UnOp::Clz
pub vyre_spec::un_op::UnOp::Cos
pub vyre_spec::un_op::UnOp::Cosh
pub vyre_spec::un_op::UnOp::Ctz
pub vyre_spec::un_op::UnOp::Exp
pub vyre_spec::un_op::UnOp::Exp2
pub vyre_spec::un_op::UnOp::Floor
pub vyre_spec::un_op::UnOp::InverseSqrt
pub vyre_spec::un_op::UnOp::IsFinite
pub vyre_spec::un_op::UnOp::IsInf
pub vyre_spec::un_op::UnOp::IsNan
pub vyre_spec::un_op::UnOp::Log
pub vyre_spec::un_op::UnOp::Log2
pub vyre_spec::un_op::UnOp::LogicalNot
pub vyre_spec::un_op::UnOp::Negate
pub vyre_spec::un_op::UnOp::Opaque(vyre_spec::extension::ExtensionUnOpId)
pub vyre_spec::un_op::UnOp::Popcount
pub vyre_spec::un_op::UnOp::Reciprocal
pub vyre_spec::un_op::UnOp::ReverseBits
pub vyre_spec::un_op::UnOp::Round
pub vyre_spec::un_op::UnOp::Sign
pub vyre_spec::un_op::UnOp::Sin
pub vyre_spec::un_op::UnOp::Sinh
pub vyre_spec::un_op::UnOp::Sqrt
pub vyre_spec::un_op::UnOp::Tan
pub vyre_spec::un_op::UnOp::Tanh
pub vyre_spec::un_op::UnOp::Trunc
pub vyre_spec::un_op::UnOp::Unpack4High
pub vyre_spec::un_op::UnOp::Unpack4Low
pub vyre_spec::un_op::UnOp::Unpack8High
pub vyre_spec::un_op::UnOp::Unpack8Low
impl vyre_spec::un_op::UnOp
pub const fn vyre_spec::un_op::UnOp::builtin_wire_tag(&self) -> core::option::Option<u8>
impl core::clone::Clone for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::clone(&self) -> vyre_spec::un_op::UnOp
impl core::cmp::Eq for vyre_spec::un_op::UnOp
impl core::cmp::PartialEq for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::eq(&self, other: &vyre_spec::un_op::UnOp) -> bool
impl core::fmt::Debug for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::un_op::UnOp
impl serde_core::ser::Serialize for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::un_op::UnOp
impl core::marker::Send for vyre_spec::un_op::UnOp
impl core::marker::Sync for vyre_spec::un_op::UnOp
impl core::marker::Unpin for vyre_spec::un_op::UnOp
impl core::marker::UnsafeUnpin for vyre_spec::un_op::UnOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::un_op::UnOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::un_op::UnOp
impl<T, U> core::convert::Into<U> for vyre_spec::un_op::UnOp where U: core::convert::From<T>
pub fn vyre_spec::un_op::UnOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::un_op::UnOp where U: core::convert::Into<T>
pub type vyre_spec::un_op::UnOp::Error = core::convert::Infallible
pub fn vyre_spec::un_op::UnOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::un_op::UnOp where U: core::convert::TryFrom<T>
pub type vyre_spec::un_op::UnOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::un_op::UnOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::un_op::UnOp where T: core::clone::Clone
pub type vyre_spec::un_op::UnOp::Owned = T
pub fn vyre_spec::un_op::UnOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::un_op::UnOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::un_op::UnOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::un_op::UnOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::un_op::UnOp where T: ?core::marker::Sized
pub fn vyre_spec::un_op::UnOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::un_op::UnOp where T: ?core::marker::Sized
pub fn vyre_spec::un_op::UnOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::un_op::UnOp where T: core::clone::Clone
pub unsafe fn vyre_spec::un_op::UnOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::un_op::UnOp where T: for<'de> serde_core::de::Deserialize<'de>
pub mod vyre_spec::verification
#[non_exhaustive] pub enum vyre_spec::verification::Verification
pub vyre_spec::verification::Verification::ExhaustiveFloat
pub vyre_spec::verification::Verification::ExhaustiveFloat::typ: vyre_spec::float_type::FloatType
pub vyre_spec::verification::Verification::ExhaustiveU16
pub vyre_spec::verification::Verification::ExhaustiveU8
pub vyre_spec::verification::Verification::WitnessedU32
pub vyre_spec::verification::Verification::WitnessedU32::count: u64
pub vyre_spec::verification::Verification::WitnessedU32::seed: u64
impl vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::witness_count(&self) -> core::option::Option<u64>
impl core::clone::Clone for vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::clone(&self) -> vyre_spec::verification::Verification
impl core::cmp::Eq for vyre_spec::verification::Verification
impl core::cmp::PartialEq for vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::eq(&self, other: &vyre_spec::verification::Verification) -> bool
impl core::fmt::Debug for vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::verification::Verification
impl core::marker::Freeze for vyre_spec::verification::Verification
impl core::marker::Send for vyre_spec::verification::Verification
impl core::marker::Sync for vyre_spec::verification::Verification
impl core::marker::Unpin for vyre_spec::verification::Verification
impl core::marker::UnsafeUnpin for vyre_spec::verification::Verification
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::verification::Verification
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::verification::Verification
impl<T, U> core::convert::Into<U> for vyre_spec::verification::Verification where U: core::convert::From<T>
pub fn vyre_spec::verification::Verification::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::verification::Verification where U: core::convert::Into<T>
pub type vyre_spec::verification::Verification::Error = core::convert::Infallible
pub fn vyre_spec::verification::Verification::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::verification::Verification where U: core::convert::TryFrom<T>
pub type vyre_spec::verification::Verification::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::verification::Verification::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::verification::Verification where T: core::clone::Clone
pub type vyre_spec::verification::Verification::Owned = T
pub fn vyre_spec::verification::Verification::clone_into(&self, target: &mut T)
pub fn vyre_spec::verification::Verification::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::verification::Verification where T: 'static + ?core::marker::Sized
pub fn vyre_spec::verification::Verification::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::verification::Verification where T: ?core::marker::Sized
pub fn vyre_spec::verification::Verification::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::verification::Verification where T: ?core::marker::Sized
pub fn vyre_spec::verification::Verification::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::verification::Verification where T: core::clone::Clone
pub unsafe fn vyre_spec::verification::Verification::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::from(t: T) -> T
#[non_exhaustive] pub enum vyre_spec::AlgebraicLaw
pub vyre_spec::AlgebraicLaw::Absorbing
pub vyre_spec::AlgebraicLaw::Absorbing::element: u32
pub vyre_spec::AlgebraicLaw::Associative
pub vyre_spec::AlgebraicLaw::Bounded
pub vyre_spec::AlgebraicLaw::Bounded::hi: u32
pub vyre_spec::AlgebraicLaw::Bounded::lo: u32
pub vyre_spec::AlgebraicLaw::CategoricalAssociative
pub vyre_spec::AlgebraicLaw::CategoricalIdentity
pub vyre_spec::AlgebraicLaw::Commutative
pub vyre_spec::AlgebraicLaw::Complement
pub vyre_spec::AlgebraicLaw::Complement::complement_op: &'static str
pub vyre_spec::AlgebraicLaw::Complement::universe: u32
pub vyre_spec::AlgebraicLaw::Custom
pub vyre_spec::AlgebraicLaw::Custom::arity: usize
pub vyre_spec::AlgebraicLaw::Custom::check: vyre_spec::algebraic_law::LawCheckFn
pub vyre_spec::AlgebraicLaw::Custom::description: &'static str
pub vyre_spec::AlgebraicLaw::Custom::name: &'static str
pub vyre_spec::AlgebraicLaw::DeMorgan
pub vyre_spec::AlgebraicLaw::DeMorgan::dual_op: &'static str
pub vyre_spec::AlgebraicLaw::DeMorgan::inner_op: &'static str
pub vyre_spec::AlgebraicLaw::DistributiveOver
pub vyre_spec::AlgebraicLaw::DistributiveOver::over_op: &'static str
pub vyre_spec::AlgebraicLaw::Idempotent
pub vyre_spec::AlgebraicLaw::Identity
pub vyre_spec::AlgebraicLaw::Identity::element: u32
pub vyre_spec::AlgebraicLaw::InverseOf
pub vyre_spec::AlgebraicLaw::InverseOf::op: &'static str
pub vyre_spec::AlgebraicLaw::Involution
pub vyre_spec::AlgebraicLaw::LatticeAbsorption
pub vyre_spec::AlgebraicLaw::LatticeAbsorption::dual_op: &'static str
pub vyre_spec::AlgebraicLaw::LeftAbsorbing
pub vyre_spec::AlgebraicLaw::LeftAbsorbing::element: u32
pub vyre_spec::AlgebraicLaw::LeftIdentity
pub vyre_spec::AlgebraicLaw::LeftIdentity::element: u32
pub vyre_spec::AlgebraicLaw::Monotone
pub vyre_spec::AlgebraicLaw::Monotonic
pub vyre_spec::AlgebraicLaw::Monotonic::direction: vyre_spec::monotonic_direction::MonotonicDirection
pub vyre_spec::AlgebraicLaw::RightAbsorbing
pub vyre_spec::AlgebraicLaw::RightAbsorbing::element: u32
pub vyre_spec::AlgebraicLaw::RightIdentity
pub vyre_spec::AlgebraicLaw::RightIdentity::element: u32
pub vyre_spec::AlgebraicLaw::SelfInverse
pub vyre_spec::AlgebraicLaw::SelfInverse::result: u32
pub vyre_spec::AlgebraicLaw::Trichotomy
pub vyre_spec::AlgebraicLaw::Trichotomy::equal_op: &'static str
pub vyre_spec::AlgebraicLaw::Trichotomy::greater_op: &'static str
pub vyre_spec::AlgebraicLaw::Trichotomy::less_op: &'static str
pub vyre_spec::AlgebraicLaw::ZeroProduct
pub vyre_spec::AlgebraicLaw::ZeroProduct::holds: bool
impl vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::is_binary(&self) -> bool
pub fn vyre_spec::algebraic_law::AlgebraicLaw::is_unary(&self) -> bool
pub fn vyre_spec::algebraic_law::AlgebraicLaw::name(&self) -> &str
impl core::clone::Clone for vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::clone(&self) -> vyre_spec::algebraic_law::AlgebraicLaw
impl core::cmp::PartialEq for vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::eq(&self, other: &Self) -> bool
impl core::fmt::Debug for vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_spec::algebraic_law::AlgebraicLaw
impl core::marker::Send for vyre_spec::algebraic_law::AlgebraicLaw
impl core::marker::Sync for vyre_spec::algebraic_law::AlgebraicLaw
impl core::marker::Unpin for vyre_spec::algebraic_law::AlgebraicLaw
impl core::marker::UnsafeUnpin for vyre_spec::algebraic_law::AlgebraicLaw
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::algebraic_law::AlgebraicLaw
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::algebraic_law::AlgebraicLaw
impl<T, U> core::convert::Into<U> for vyre_spec::algebraic_law::AlgebraicLaw where U: core::convert::From<T>
pub fn vyre_spec::algebraic_law::AlgebraicLaw::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::algebraic_law::AlgebraicLaw where U: core::convert::Into<T>
pub type vyre_spec::algebraic_law::AlgebraicLaw::Error = core::convert::Infallible
pub fn vyre_spec::algebraic_law::AlgebraicLaw::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::algebraic_law::AlgebraicLaw where U: core::convert::TryFrom<T>
pub type vyre_spec::algebraic_law::AlgebraicLaw::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::algebraic_law::AlgebraicLaw::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::algebraic_law::AlgebraicLaw where T: core::clone::Clone
pub type vyre_spec::algebraic_law::AlgebraicLaw::Owned = T
pub fn vyre_spec::algebraic_law::AlgebraicLaw::clone_into(&self, target: &mut T)
pub fn vyre_spec::algebraic_law::AlgebraicLaw::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::algebraic_law::AlgebraicLaw where T: 'static + ?core::marker::Sized
pub fn vyre_spec::algebraic_law::AlgebraicLaw::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::algebraic_law::AlgebraicLaw where T: ?core::marker::Sized
pub fn vyre_spec::algebraic_law::AlgebraicLaw::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::algebraic_law::AlgebraicLaw where T: ?core::marker::Sized
pub fn vyre_spec::algebraic_law::AlgebraicLaw::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::algebraic_law::AlgebraicLaw where T: core::clone::Clone
pub unsafe fn vyre_spec::algebraic_law::AlgebraicLaw::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::algebraic_law::AlgebraicLaw
pub fn vyre_spec::algebraic_law::AlgebraicLaw::from(t: T) -> T
#[non_exhaustive] pub enum vyre_spec::AtomicOp
pub vyre_spec::AtomicOp::Add
pub vyre_spec::AtomicOp::And
pub vyre_spec::AtomicOp::CompareExchange
pub vyre_spec::AtomicOp::CompareExchangeWeak
pub vyre_spec::AtomicOp::Exchange
pub vyre_spec::AtomicOp::FetchNand
pub vyre_spec::AtomicOp::LruUpdate
pub vyre_spec::AtomicOp::Max
pub vyre_spec::AtomicOp::Min
pub vyre_spec::AtomicOp::Opaque(vyre_spec::extension::ExtensionAtomicOpId)
pub vyre_spec::AtomicOp::Or
pub vyre_spec::AtomicOp::Xor
impl vyre_spec::atomic_op::AtomicOp
pub const fn vyre_spec::atomic_op::AtomicOp::builtin_wire_tag(&self) -> core::option::Option<u8>
impl core::clone::Clone for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::clone(&self) -> vyre_spec::atomic_op::AtomicOp
impl core::cmp::Eq for vyre_spec::atomic_op::AtomicOp
impl core::cmp::PartialEq for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::eq(&self, other: &vyre_spec::atomic_op::AtomicOp) -> bool
impl core::fmt::Debug for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::atomic_op::AtomicOp
impl core::marker::StructuralPartialEq for vyre_spec::atomic_op::AtomicOp
impl serde_core::ser::Serialize for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::atomic_op::AtomicOp
impl core::marker::Send for vyre_spec::atomic_op::AtomicOp
impl core::marker::Sync for vyre_spec::atomic_op::AtomicOp
impl core::marker::Unpin for vyre_spec::atomic_op::AtomicOp
impl core::marker::UnsafeUnpin for vyre_spec::atomic_op::AtomicOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::atomic_op::AtomicOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::atomic_op::AtomicOp
impl<T, U> core::convert::Into<U> for vyre_spec::atomic_op::AtomicOp where U: core::convert::From<T>
pub fn vyre_spec::atomic_op::AtomicOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::atomic_op::AtomicOp where U: core::convert::Into<T>
pub type vyre_spec::atomic_op::AtomicOp::Error = core::convert::Infallible
pub fn vyre_spec::atomic_op::AtomicOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::atomic_op::AtomicOp where U: core::convert::TryFrom<T>
pub type vyre_spec::atomic_op::AtomicOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::atomic_op::AtomicOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::atomic_op::AtomicOp where T: core::clone::Clone
pub type vyre_spec::atomic_op::AtomicOp::Owned = T
pub fn vyre_spec::atomic_op::AtomicOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::atomic_op::AtomicOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::atomic_op::AtomicOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::atomic_op::AtomicOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::atomic_op::AtomicOp where T: ?core::marker::Sized
pub fn vyre_spec::atomic_op::AtomicOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::atomic_op::AtomicOp where T: ?core::marker::Sized
pub fn vyre_spec::atomic_op::AtomicOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::atomic_op::AtomicOp where T: core::clone::Clone
pub unsafe fn vyre_spec::atomic_op::AtomicOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::atomic_op::AtomicOp
pub fn vyre_spec::atomic_op::AtomicOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::atomic_op::AtomicOp where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::BinOp
pub vyre_spec::BinOp::AbsDiff
pub vyre_spec::BinOp::Add
pub vyre_spec::BinOp::And
pub vyre_spec::BinOp::Ballot
pub vyre_spec::BinOp::BitAnd
pub vyre_spec::BinOp::BitOr
pub vyre_spec::BinOp::BitXor
pub vyre_spec::BinOp::Div
pub vyre_spec::BinOp::Eq
pub vyre_spec::BinOp::Ge
pub vyre_spec::BinOp::Gt
pub vyre_spec::BinOp::Le
pub vyre_spec::BinOp::Lt
pub vyre_spec::BinOp::Max
pub vyre_spec::BinOp::Min
pub vyre_spec::BinOp::Mod
pub vyre_spec::BinOp::Mul
pub vyre_spec::BinOp::MulHigh
pub vyre_spec::BinOp::Ne
pub vyre_spec::BinOp::Opaque(vyre_spec::extension::ExtensionBinOpId)
pub vyre_spec::BinOp::Or
pub vyre_spec::BinOp::RotateLeft
pub vyre_spec::BinOp::RotateRight
pub vyre_spec::BinOp::SaturatingAdd
pub vyre_spec::BinOp::SaturatingMul
pub vyre_spec::BinOp::SaturatingSub
pub vyre_spec::BinOp::Shl
pub vyre_spec::BinOp::Shr
pub vyre_spec::BinOp::Shuffle
pub vyre_spec::BinOp::Sub
pub vyre_spec::BinOp::WaveBroadcast
pub vyre_spec::BinOp::WaveReduce
pub vyre_spec::BinOp::WrappingAdd
pub vyre_spec::BinOp::WrappingSub
impl vyre_spec::bin_op::BinOp
pub const fn vyre_spec::bin_op::BinOp::builtin_wire_tag(&self) -> core::option::Option<u8>
impl vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::intensity(&self) -> vyre_spec::bin_op::OpIntensity
impl core::clone::Clone for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::clone(&self) -> vyre_spec::bin_op::BinOp
impl core::cmp::Eq for vyre_spec::bin_op::BinOp
impl core::cmp::PartialEq for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::eq(&self, other: &vyre_spec::bin_op::BinOp) -> bool
impl core::fmt::Debug for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::bin_op::BinOp
impl core::marker::StructuralPartialEq for vyre_spec::bin_op::BinOp
impl serde_core::ser::Serialize for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::bin_op::BinOp
impl core::marker::Send for vyre_spec::bin_op::BinOp
impl core::marker::Sync for vyre_spec::bin_op::BinOp
impl core::marker::Unpin for vyre_spec::bin_op::BinOp
impl core::marker::UnsafeUnpin for vyre_spec::bin_op::BinOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::bin_op::BinOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::bin_op::BinOp
impl<T, U> core::convert::Into<U> for vyre_spec::bin_op::BinOp where U: core::convert::From<T>
pub fn vyre_spec::bin_op::BinOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::bin_op::BinOp where U: core::convert::Into<T>
pub type vyre_spec::bin_op::BinOp::Error = core::convert::Infallible
pub fn vyre_spec::bin_op::BinOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::bin_op::BinOp where U: core::convert::TryFrom<T>
pub type vyre_spec::bin_op::BinOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::bin_op::BinOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::bin_op::BinOp where T: core::clone::Clone
pub type vyre_spec::bin_op::BinOp::Owned = T
pub fn vyre_spec::bin_op::BinOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::bin_op::BinOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::bin_op::BinOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::bin_op::BinOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::bin_op::BinOp where T: ?core::marker::Sized
pub fn vyre_spec::bin_op::BinOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::bin_op::BinOp where T: ?core::marker::Sized
pub fn vyre_spec::bin_op::BinOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::bin_op::BinOp where T: core::clone::Clone
pub unsafe fn vyre_spec::bin_op::BinOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::bin_op::BinOp
pub fn vyre_spec::bin_op::BinOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::bin_op::BinOp where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::BufferAccess
pub vyre_spec::BufferAccess::ReadOnly
pub vyre_spec::BufferAccess::ReadWrite
pub vyre_spec::BufferAccess::Uniform
pub vyre_spec::BufferAccess::Workgroup
pub vyre_spec::BufferAccess::WriteOnly
impl core::clone::Clone for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::clone(&self) -> vyre_spec::buffer_access::BufferAccess
impl core::cmp::Eq for vyre_spec::buffer_access::BufferAccess
impl core::cmp::PartialEq for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::eq(&self, other: &vyre_spec::buffer_access::BufferAccess) -> bool
impl core::fmt::Debug for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::buffer_access::BufferAccess
impl serde_core::ser::Serialize for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::buffer_access::BufferAccess
impl core::marker::Send for vyre_spec::buffer_access::BufferAccess
impl core::marker::Sync for vyre_spec::buffer_access::BufferAccess
impl core::marker::Unpin for vyre_spec::buffer_access::BufferAccess
impl core::marker::UnsafeUnpin for vyre_spec::buffer_access::BufferAccess
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::buffer_access::BufferAccess
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::buffer_access::BufferAccess
impl<T, U> core::convert::Into<U> for vyre_spec::buffer_access::BufferAccess where U: core::convert::From<T>
pub fn vyre_spec::buffer_access::BufferAccess::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::buffer_access::BufferAccess where U: core::convert::Into<T>
pub type vyre_spec::buffer_access::BufferAccess::Error = core::convert::Infallible
pub fn vyre_spec::buffer_access::BufferAccess::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::buffer_access::BufferAccess where U: core::convert::TryFrom<T>
pub type vyre_spec::buffer_access::BufferAccess::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::buffer_access::BufferAccess::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::buffer_access::BufferAccess where T: core::clone::Clone
pub type vyre_spec::buffer_access::BufferAccess::Owned = T
pub fn vyre_spec::buffer_access::BufferAccess::clone_into(&self, target: &mut T)
pub fn vyre_spec::buffer_access::BufferAccess::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::buffer_access::BufferAccess where T: 'static + ?core::marker::Sized
pub fn vyre_spec::buffer_access::BufferAccess::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::buffer_access::BufferAccess where T: ?core::marker::Sized
pub fn vyre_spec::buffer_access::BufferAccess::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::buffer_access::BufferAccess where T: ?core::marker::Sized
pub fn vyre_spec::buffer_access::BufferAccess::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::buffer_access::BufferAccess where T: core::clone::Clone
pub unsafe fn vyre_spec::buffer_access::BufferAccess::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::buffer_access::BufferAccess
pub fn vyre_spec::buffer_access::BufferAccess::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::buffer_access::BufferAccess where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::Category
pub vyre_spec::Category::A
pub vyre_spec::Category::A::composition_of: alloc::vec::Vec<&'static str>
pub vyre_spec::Category::C
pub vyre_spec::Category::C::backend_availability: vyre_spec::category::BackendAvailabilityPredicate
pub vyre_spec::Category::C::hardware: &'static str
impl vyre_spec::category::Category
pub fn vyre_spec::category::Category::is_unclassified(&self) -> bool
pub fn vyre_spec::category::Category::unclassified() -> Self
impl core::clone::Clone for vyre_spec::category::Category
pub fn vyre_spec::category::Category::clone(&self) -> vyre_spec::category::Category
impl core::cmp::Eq for vyre_spec::category::Category
impl core::cmp::PartialEq for vyre_spec::category::Category
pub fn vyre_spec::category::Category::eq(&self, other: &Self) -> bool
impl core::fmt::Debug for vyre_spec::category::Category
pub fn vyre_spec::category::Category::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_spec::category::Category
impl core::marker::Send for vyre_spec::category::Category
impl core::marker::Sync for vyre_spec::category::Category
impl core::marker::Unpin for vyre_spec::category::Category
impl core::marker::UnsafeUnpin for vyre_spec::category::Category
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::category::Category
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::category::Category
impl<T, U> core::convert::Into<U> for vyre_spec::category::Category where U: core::convert::From<T>
pub fn vyre_spec::category::Category::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::category::Category where U: core::convert::Into<T>
pub type vyre_spec::category::Category::Error = core::convert::Infallible
pub fn vyre_spec::category::Category::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::category::Category where U: core::convert::TryFrom<T>
pub type vyre_spec::category::Category::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::category::Category::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::category::Category where T: core::clone::Clone
pub type vyre_spec::category::Category::Owned = T
pub fn vyre_spec::category::Category::clone_into(&self, target: &mut T)
pub fn vyre_spec::category::Category::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::category::Category where T: 'static + ?core::marker::Sized
pub fn vyre_spec::category::Category::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::category::Category where T: ?core::marker::Sized
pub fn vyre_spec::category::Category::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::category::Category where T: ?core::marker::Sized
pub fn vyre_spec::category::Category::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::category::Category where T: core::clone::Clone
pub unsafe fn vyre_spec::category::Category::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::category::Category
pub fn vyre_spec::category::Category::from(t: T) -> T
#[non_exhaustive] pub enum vyre_spec::CollectiveOp
pub vyre_spec::CollectiveOp::BitAnd
pub vyre_spec::CollectiveOp::BitOr
pub vyre_spec::CollectiveOp::BitXor
pub vyre_spec::CollectiveOp::Max
pub vyre_spec::CollectiveOp::Min
pub vyre_spec::CollectiveOp::Sum
impl vyre_spec::collective_op::CollectiveOp
pub const fn vyre_spec::collective_op::CollectiveOp::builtin_wire_tag(self) -> u8
pub fn vyre_spec::collective_op::CollectiveOp::from_wire_tag(tag: u8) -> core::result::Result<Self, alloc::string::String>
impl core::clone::Clone for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::clone(&self) -> vyre_spec::collective_op::CollectiveOp
impl core::cmp::Eq for vyre_spec::collective_op::CollectiveOp
impl core::cmp::PartialEq for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::eq(&self, other: &vyre_spec::collective_op::CollectiveOp) -> bool
impl core::fmt::Debug for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::collective_op::CollectiveOp
impl core::marker::StructuralPartialEq for vyre_spec::collective_op::CollectiveOp
impl serde_core::ser::Serialize for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::collective_op::CollectiveOp
impl core::marker::Send for vyre_spec::collective_op::CollectiveOp
impl core::marker::Sync for vyre_spec::collective_op::CollectiveOp
impl core::marker::Unpin for vyre_spec::collective_op::CollectiveOp
impl core::marker::UnsafeUnpin for vyre_spec::collective_op::CollectiveOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::collective_op::CollectiveOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::collective_op::CollectiveOp
impl<T, U> core::convert::Into<U> for vyre_spec::collective_op::CollectiveOp where U: core::convert::From<T>
pub fn vyre_spec::collective_op::CollectiveOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::collective_op::CollectiveOp where U: core::convert::Into<T>
pub type vyre_spec::collective_op::CollectiveOp::Error = core::convert::Infallible
pub fn vyre_spec::collective_op::CollectiveOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::collective_op::CollectiveOp where U: core::convert::TryFrom<T>
pub type vyre_spec::collective_op::CollectiveOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::collective_op::CollectiveOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::collective_op::CollectiveOp where T: core::clone::Clone
pub type vyre_spec::collective_op::CollectiveOp::Owned = T
pub fn vyre_spec::collective_op::CollectiveOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::collective_op::CollectiveOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::collective_op::CollectiveOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::collective_op::CollectiveOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::collective_op::CollectiveOp where T: ?core::marker::Sized
pub fn vyre_spec::collective_op::CollectiveOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::collective_op::CollectiveOp where T: ?core::marker::Sized
pub fn vyre_spec::collective_op::CollectiveOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::collective_op::CollectiveOp where T: core::clone::Clone
pub unsafe fn vyre_spec::collective_op::CollectiveOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::collective_op::CollectiveOp
pub fn vyre_spec::collective_op::CollectiveOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::collective_op::CollectiveOp where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::Convention
pub vyre_spec::Convention::V1
pub vyre_spec::Convention::V2
pub vyre_spec::Convention::V2::lookup_binding: u32
impl core::clone::Clone for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::clone(&self) -> vyre_spec::convention::Convention
impl core::cmp::Eq for vyre_spec::convention::Convention
impl core::cmp::PartialEq for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::eq(&self, other: &vyre_spec::convention::Convention) -> bool
impl core::default::Default for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::default() -> vyre_spec::convention::Convention
impl core::fmt::Debug for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::convention::Convention
impl serde_core::ser::Serialize for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::convention::Convention
impl core::marker::Send for vyre_spec::convention::Convention
impl core::marker::Sync for vyre_spec::convention::Convention
impl core::marker::Unpin for vyre_spec::convention::Convention
impl core::marker::UnsafeUnpin for vyre_spec::convention::Convention
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::convention::Convention
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::convention::Convention
impl<T, U> core::convert::Into<U> for vyre_spec::convention::Convention where U: core::convert::From<T>
pub fn vyre_spec::convention::Convention::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::convention::Convention where U: core::convert::Into<T>
pub type vyre_spec::convention::Convention::Error = core::convert::Infallible
pub fn vyre_spec::convention::Convention::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::convention::Convention where U: core::convert::TryFrom<T>
pub type vyre_spec::convention::Convention::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::convention::Convention::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::convention::Convention where T: core::clone::Clone
pub type vyre_spec::convention::Convention::Owned = T
pub fn vyre_spec::convention::Convention::clone_into(&self, target: &mut T)
pub fn vyre_spec::convention::Convention::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::convention::Convention where T: 'static + ?core::marker::Sized
pub fn vyre_spec::convention::Convention::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::convention::Convention where T: ?core::marker::Sized
pub fn vyre_spec::convention::Convention::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::convention::Convention where T: ?core::marker::Sized
pub fn vyre_spec::convention::Convention::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::convention::Convention where T: core::clone::Clone
pub unsafe fn vyre_spec::convention::Convention::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::convention::Convention
pub fn vyre_spec::convention::Convention::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::convention::Convention where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::CostHint
pub vyre_spec::CostHint::Cheap
pub vyre_spec::CostHint::Expensive
pub vyre_spec::CostHint::Medium
pub vyre_spec::CostHint::Unknown
impl core::clone::Clone for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::clone(&self) -> vyre_spec::op_contract::CostHint
impl core::cmp::Eq for vyre_spec::op_contract::CostHint
impl core::cmp::PartialEq for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::eq(&self, other: &vyre_spec::op_contract::CostHint) -> bool
impl core::fmt::Debug for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::op_contract::CostHint
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::CostHint
impl serde_core::ser::Serialize for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::CostHint
impl core::marker::Send for vyre_spec::op_contract::CostHint
impl core::marker::Sync for vyre_spec::op_contract::CostHint
impl core::marker::Unpin for vyre_spec::op_contract::CostHint
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::CostHint
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::CostHint
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::CostHint
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::CostHint where U: core::convert::From<T>
pub fn vyre_spec::op_contract::CostHint::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::CostHint where U: core::convert::Into<T>
pub type vyre_spec::op_contract::CostHint::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::CostHint::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::CostHint where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::CostHint::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::CostHint::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::CostHint where T: core::clone::Clone
pub type vyre_spec::op_contract::CostHint::Owned = T
pub fn vyre_spec::op_contract::CostHint::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::CostHint::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::CostHint where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::CostHint::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::CostHint where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::CostHint::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::CostHint where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::CostHint::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::CostHint where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::CostHint::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::CostHint
pub fn vyre_spec::op_contract::CostHint::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::CostHint where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::DataType
pub vyre_spec::DataType::Array
pub vyre_spec::DataType::Array::element_size: usize
pub vyre_spec::DataType::BF16
pub vyre_spec::DataType::Bool
pub vyre_spec::DataType::Bytes
pub vyre_spec::DataType::DeviceMesh
pub vyre_spec::DataType::DeviceMesh::axes: smallvec::SmallVec<[u32; 3]>
pub vyre_spec::DataType::F16
pub vyre_spec::DataType::F32
pub vyre_spec::DataType::F64
pub vyre_spec::DataType::F8E4M3
pub vyre_spec::DataType::F8E5M2
pub vyre_spec::DataType::FP4
pub vyre_spec::DataType::Handle(vyre_spec::data_type::TypeId)
pub vyre_spec::DataType::I16
pub vyre_spec::DataType::I32
pub vyre_spec::DataType::I4
pub vyre_spec::DataType::I64
pub vyre_spec::DataType::I8
pub vyre_spec::DataType::NF4
pub vyre_spec::DataType::Opaque(vyre_spec::extension::ExtensionDataTypeId)
pub vyre_spec::DataType::Quantized
pub vyre_spec::DataType::Quantized::scale: vyre_spec::data_type::QuantizationScale
pub vyre_spec::DataType::Quantized::storage: alloc::boxed::Box<Self>
pub vyre_spec::DataType::Quantized::zero_point: vyre_spec::data_type::QuantizationZeroPoint
pub vyre_spec::DataType::SparseBsr
pub vyre_spec::DataType::SparseBsr::block_cols: u32
pub vyre_spec::DataType::SparseBsr::block_rows: u32
pub vyre_spec::DataType::SparseBsr::element: alloc::boxed::Box<Self>
pub vyre_spec::DataType::SparseCoo
pub vyre_spec::DataType::SparseCoo::element: alloc::boxed::Box<Self>
pub vyre_spec::DataType::SparseCsr
pub vyre_spec::DataType::SparseCsr::element: alloc::boxed::Box<Self>
pub vyre_spec::DataType::Tensor
pub vyre_spec::DataType::TensorShaped
pub vyre_spec::DataType::TensorShaped::element: alloc::boxed::Box<Self>
pub vyre_spec::DataType::TensorShaped::shape: smallvec::SmallVec<[u32; 4]>
pub vyre_spec::DataType::U16
pub vyre_spec::DataType::U32
pub vyre_spec::DataType::U64
pub vyre_spec::DataType::U8
pub vyre_spec::DataType::Vec
pub vyre_spec::DataType::Vec::count: u8
pub vyre_spec::DataType::Vec::element: alloc::boxed::Box<Self>
pub vyre_spec::DataType::Vec2U32
pub vyre_spec::DataType::Vec4U32
impl vyre_spec::data_type::DataType
pub const fn vyre_spec::data_type::DataType::bit_width(&self) -> core::option::Option<usize>
pub const fn vyre_spec::data_type::DataType::element_size(&self) -> core::option::Option<usize>
pub const fn vyre_spec::data_type::DataType::max_bytes(&self) -> core::option::Option<usize>
pub const fn vyre_spec::data_type::DataType::min_bytes(&self) -> usize
pub fn vyre_spec::data_type::DataType::packed_size_bytes(&self, element_count: usize) -> core::result::Result<core::option::Option<usize>, alloc::string::String>
pub const fn vyre_spec::data_type::DataType::size_bytes(&self) -> core::option::Option<usize>
impl vyre_spec::data_type::DataType
pub const fn vyre_spec::data_type::DataType::builtin_wire_tag(&self) -> core::option::Option<u8>
pub const fn vyre_spec::data_type::DataType::is_float_family(&self) -> bool
pub const fn vyre_spec::data_type::DataType::is_quantized(&self) -> bool
pub const fn vyre_spec::data_type::DataType::is_quantized_storage(&self) -> bool
impl vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::validate_layout(&self) -> core::result::Result<(), alloc::string::String>
impl core::clone::Clone for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::clone(&self) -> vyre_spec::data_type::DataType
impl core::cmp::Eq for vyre_spec::data_type::DataType
impl core::cmp::PartialEq for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::eq(&self, other: &vyre_spec::data_type::DataType) -> bool
impl core::fmt::Debug for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::data_type::DataType
impl serde_core::ser::Serialize for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::data_type::DataType
impl core::marker::Send for vyre_spec::data_type::DataType
impl core::marker::Sync for vyre_spec::data_type::DataType
impl core::marker::Unpin for vyre_spec::data_type::DataType
impl core::marker::UnsafeUnpin for vyre_spec::data_type::DataType
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::data_type::DataType
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::data_type::DataType
impl<T, U> core::convert::Into<U> for vyre_spec::data_type::DataType where U: core::convert::From<T>
pub fn vyre_spec::data_type::DataType::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::data_type::DataType where U: core::convert::Into<T>
pub type vyre_spec::data_type::DataType::Error = core::convert::Infallible
pub fn vyre_spec::data_type::DataType::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::data_type::DataType where U: core::convert::TryFrom<T>
pub type vyre_spec::data_type::DataType::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::data_type::DataType::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::data_type::DataType where T: core::clone::Clone
pub type vyre_spec::data_type::DataType::Owned = T
pub fn vyre_spec::data_type::DataType::clone_into(&self, target: &mut T)
pub fn vyre_spec::data_type::DataType::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::data_type::DataType where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::data_type::DataType::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::data_type::DataType where T: 'static + ?core::marker::Sized
pub fn vyre_spec::data_type::DataType::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::data_type::DataType where T: ?core::marker::Sized
pub fn vyre_spec::data_type::DataType::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::data_type::DataType where T: ?core::marker::Sized
pub fn vyre_spec::data_type::DataType::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::data_type::DataType where T: core::clone::Clone
pub unsafe fn vyre_spec::data_type::DataType::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::data_type::DataType
pub fn vyre_spec::data_type::DataType::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::data_type::DataType where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::DeterminismClass
pub vyre_spec::DeterminismClass::Deterministic
pub vyre_spec::DeterminismClass::DeterministicModuloRounding
pub vyre_spec::DeterminismClass::NonDeterministic
impl core::clone::Clone for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::clone(&self) -> vyre_spec::op_contract::DeterminismClass
impl core::cmp::Eq for vyre_spec::op_contract::DeterminismClass
impl core::cmp::PartialEq for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::eq(&self, other: &vyre_spec::op_contract::DeterminismClass) -> bool
impl core::fmt::Debug for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::op_contract::DeterminismClass
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::DeterminismClass
impl serde_core::ser::Serialize for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::DeterminismClass
impl core::marker::Send for vyre_spec::op_contract::DeterminismClass
impl core::marker::Sync for vyre_spec::op_contract::DeterminismClass
impl core::marker::Unpin for vyre_spec::op_contract::DeterminismClass
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::DeterminismClass
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::DeterminismClass
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::DeterminismClass
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::DeterminismClass where U: core::convert::From<T>
pub fn vyre_spec::op_contract::DeterminismClass::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::DeterminismClass where U: core::convert::Into<T>
pub type vyre_spec::op_contract::DeterminismClass::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::DeterminismClass::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::DeterminismClass where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::DeterminismClass::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::DeterminismClass::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::DeterminismClass where T: core::clone::Clone
pub type vyre_spec::op_contract::DeterminismClass::Owned = T
pub fn vyre_spec::op_contract::DeterminismClass::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::DeterminismClass::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::DeterminismClass where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::DeterminismClass::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::DeterminismClass where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::DeterminismClass::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::DeterminismClass where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::DeterminismClass::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::DeterminismClass where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::DeterminismClass::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::DeterminismClass
pub fn vyre_spec::op_contract::DeterminismClass::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::DeterminismClass where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::EngineInvariant
pub vyre_spec::EngineInvariant::I1 = 1
pub vyre_spec::EngineInvariant::I10 = 10
pub vyre_spec::EngineInvariant::I11 = 11
pub vyre_spec::EngineInvariant::I12 = 12
pub vyre_spec::EngineInvariant::I13 = 13
pub vyre_spec::EngineInvariant::I14 = 14
pub vyre_spec::EngineInvariant::I15 = 15
pub vyre_spec::EngineInvariant::I2 = 2
pub vyre_spec::EngineInvariant::I3 = 3
pub vyre_spec::EngineInvariant::I4 = 4
pub vyre_spec::EngineInvariant::I5 = 5
pub vyre_spec::EngineInvariant::I6 = 6
pub vyre_spec::EngineInvariant::I7 = 7
pub vyre_spec::EngineInvariant::I8 = 8
pub vyre_spec::EngineInvariant::I9 = 9
impl vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::iter() -> impl core::iter::traits::iterator::Iterator<Item = Self>
pub const fn vyre_spec::engine_invariant::EngineInvariant::ordinal(&self) -> u8
impl core::clone::Clone for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::clone(&self) -> vyre_spec::engine_invariant::EngineInvariant
impl core::cmp::Eq for vyre_spec::engine_invariant::EngineInvariant
impl core::cmp::Ord for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::cmp(&self, other: &vyre_spec::engine_invariant::EngineInvariant) -> core::cmp::Ordering
impl core::cmp::PartialEq for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::eq(&self, other: &vyre_spec::engine_invariant::EngineInvariant) -> bool
impl core::cmp::PartialOrd for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::partial_cmp(&self, other: &vyre_spec::engine_invariant::EngineInvariant) -> core::option::Option<core::cmp::Ordering>
impl core::fmt::Debug for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::StructuralPartialEq for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::Freeze for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::Send for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::Sync for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::Unpin for vyre_spec::engine_invariant::EngineInvariant
impl core::marker::UnsafeUnpin for vyre_spec::engine_invariant::EngineInvariant
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::engine_invariant::EngineInvariant
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::engine_invariant::EngineInvariant
impl<T, U> core::convert::Into<U> for vyre_spec::engine_invariant::EngineInvariant where U: core::convert::From<T>
pub fn vyre_spec::engine_invariant::EngineInvariant::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::engine_invariant::EngineInvariant where U: core::convert::Into<T>
pub type vyre_spec::engine_invariant::EngineInvariant::Error = core::convert::Infallible
pub fn vyre_spec::engine_invariant::EngineInvariant::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::engine_invariant::EngineInvariant where U: core::convert::TryFrom<T>
pub type vyre_spec::engine_invariant::EngineInvariant::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::engine_invariant::EngineInvariant::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::engine_invariant::EngineInvariant where T: core::clone::Clone
pub type vyre_spec::engine_invariant::EngineInvariant::Owned = T
pub fn vyre_spec::engine_invariant::EngineInvariant::clone_into(&self, target: &mut T)
pub fn vyre_spec::engine_invariant::EngineInvariant::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::engine_invariant::EngineInvariant where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::engine_invariant::EngineInvariant::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::engine_invariant::EngineInvariant where T: 'static + ?core::marker::Sized
pub fn vyre_spec::engine_invariant::EngineInvariant::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::engine_invariant::EngineInvariant where T: ?core::marker::Sized
pub fn vyre_spec::engine_invariant::EngineInvariant::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::engine_invariant::EngineInvariant where T: ?core::marker::Sized
pub fn vyre_spec::engine_invariant::EngineInvariant::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::engine_invariant::EngineInvariant where T: core::clone::Clone
pub unsafe fn vyre_spec::engine_invariant::EngineInvariant::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::engine_invariant::EngineInvariant
pub fn vyre_spec::engine_invariant::EngineInvariant::from(t: T) -> T
#[non_exhaustive] pub enum vyre_spec::FloatType
pub vyre_spec::FloatType::BF16
pub vyre_spec::FloatType::F16
pub vyre_spec::FloatType::F32
impl core::clone::Clone for vyre_spec::float_type::FloatType
pub fn vyre_spec::float_type::FloatType::clone(&self) -> vyre_spec::float_type::FloatType
impl core::cmp::Eq for vyre_spec::float_type::FloatType
impl core::cmp::PartialEq for vyre_spec::float_type::FloatType
pub fn vyre_spec::float_type::FloatType::eq(&self, other: &vyre_spec::float_type::FloatType) -> bool
impl core::fmt::Debug for vyre_spec::float_type::FloatType
pub fn vyre_spec::float_type::FloatType::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::float_type::FloatType
impl core::marker::Freeze for vyre_spec::float_type::FloatType
impl core::marker::Send for vyre_spec::float_type::FloatType
impl core::marker::Sync for vyre_spec::float_type::FloatType
impl core::marker::Unpin for vyre_spec::float_type::FloatType
impl core::marker::UnsafeUnpin for vyre_spec::float_type::FloatType
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::float_type::FloatType
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::float_type::FloatType
impl<T, U> core::convert::Into<U> for vyre_spec::float_type::FloatType where U: core::convert::From<T>
pub fn vyre_spec::float_type::FloatType::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::float_type::FloatType where U: core::convert::Into<T>
pub type vyre_spec::float_type::FloatType::Error = core::convert::Infallible
pub fn vyre_spec::float_type::FloatType::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::float_type::FloatType where U: core::convert::TryFrom<T>
pub type vyre_spec::float_type::FloatType::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::float_type::FloatType::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::float_type::FloatType where T: core::clone::Clone
pub type vyre_spec::float_type::FloatType::Owned = T
pub fn vyre_spec::float_type::FloatType::clone_into(&self, target: &mut T)
pub fn vyre_spec::float_type::FloatType::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::float_type::FloatType where T: 'static + ?core::marker::Sized
pub fn vyre_spec::float_type::FloatType::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::float_type::FloatType where T: ?core::marker::Sized
pub fn vyre_spec::float_type::FloatType::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::float_type::FloatType where T: ?core::marker::Sized
pub fn vyre_spec::float_type::FloatType::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::float_type::FloatType where T: core::clone::Clone
pub unsafe fn vyre_spec::float_type::FloatType::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::float_type::FloatType
pub fn vyre_spec::float_type::FloatType::from(t: T) -> T
#[non_exhaustive] pub enum vyre_spec::InvariantCategory
pub vyre_spec::InvariantCategory::Algebra
pub vyre_spec::InvariantCategory::Execution
pub vyre_spec::InvariantCategory::Resource
pub vyre_spec::InvariantCategory::Stability
impl core::clone::Clone for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::clone(&self) -> vyre_spec::invariant_category::InvariantCategory
impl core::cmp::Eq for vyre_spec::invariant_category::InvariantCategory
impl core::cmp::PartialEq for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::eq(&self, other: &vyre_spec::invariant_category::InvariantCategory) -> bool
impl core::fmt::Debug for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::invariant_category::InvariantCategory
impl core::marker::StructuralPartialEq for vyre_spec::invariant_category::InvariantCategory
impl core::marker::Freeze for vyre_spec::invariant_category::InvariantCategory
impl core::marker::Send for vyre_spec::invariant_category::InvariantCategory
impl core::marker::Sync for vyre_spec::invariant_category::InvariantCategory
impl core::marker::Unpin for vyre_spec::invariant_category::InvariantCategory
impl core::marker::UnsafeUnpin for vyre_spec::invariant_category::InvariantCategory
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::invariant_category::InvariantCategory
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::invariant_category::InvariantCategory
impl<T, U> core::convert::Into<U> for vyre_spec::invariant_category::InvariantCategory where U: core::convert::From<T>
pub fn vyre_spec::invariant_category::InvariantCategory::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::invariant_category::InvariantCategory where U: core::convert::Into<T>
pub type vyre_spec::invariant_category::InvariantCategory::Error = core::convert::Infallible
pub fn vyre_spec::invariant_category::InvariantCategory::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::invariant_category::InvariantCategory where U: core::convert::TryFrom<T>
pub type vyre_spec::invariant_category::InvariantCategory::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::invariant_category::InvariantCategory::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::invariant_category::InvariantCategory where T: core::clone::Clone
pub type vyre_spec::invariant_category::InvariantCategory::Owned = T
pub fn vyre_spec::invariant_category::InvariantCategory::clone_into(&self, target: &mut T)
pub fn vyre_spec::invariant_category::InvariantCategory::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::invariant_category::InvariantCategory where T: 'static + ?core::marker::Sized
pub fn vyre_spec::invariant_category::InvariantCategory::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::invariant_category::InvariantCategory where T: ?core::marker::Sized
pub fn vyre_spec::invariant_category::InvariantCategory::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::invariant_category::InvariantCategory where T: ?core::marker::Sized
pub fn vyre_spec::invariant_category::InvariantCategory::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::invariant_category::InvariantCategory where T: core::clone::Clone
pub unsafe fn vyre_spec::invariant_category::InvariantCategory::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::invariant_category::InvariantCategory
pub fn vyre_spec::invariant_category::InvariantCategory::from(t: T) -> T
#[non_exhaustive] pub enum vyre_spec::Layer
pub vyre_spec::Layer::L0
pub vyre_spec::Layer::L1
pub vyre_spec::Layer::L2
pub vyre_spec::Layer::L3
pub vyre_spec::Layer::L4
pub vyre_spec::Layer::L5
impl vyre_spec::layer::Layer
pub const fn vyre_spec::layer::Layer::id(&self) -> &'static str
pub const fn vyre_spec::layer::Layer::layer_description(&self) -> &'static str
impl core::clone::Clone for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::clone(&self) -> vyre_spec::layer::Layer
impl core::cmp::Eq for vyre_spec::layer::Layer
impl core::cmp::PartialEq for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::eq(&self, other: &vyre_spec::layer::Layer) -> bool
impl core::fmt::Debug for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::layer::Layer
impl serde_core::ser::Serialize for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::layer::Layer
impl core::marker::Send for vyre_spec::layer::Layer
impl core::marker::Sync for vyre_spec::layer::Layer
impl core::marker::Unpin for vyre_spec::layer::Layer
impl core::marker::UnsafeUnpin for vyre_spec::layer::Layer
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::layer::Layer
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::layer::Layer
impl<T, U> core::convert::Into<U> for vyre_spec::layer::Layer where U: core::convert::From<T>
pub fn vyre_spec::layer::Layer::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::layer::Layer where U: core::convert::Into<T>
pub type vyre_spec::layer::Layer::Error = core::convert::Infallible
pub fn vyre_spec::layer::Layer::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::layer::Layer where U: core::convert::TryFrom<T>
pub type vyre_spec::layer::Layer::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::layer::Layer::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::layer::Layer where T: core::clone::Clone
pub type vyre_spec::layer::Layer::Owned = T
pub fn vyre_spec::layer::Layer::clone_into(&self, target: &mut T)
pub fn vyre_spec::layer::Layer::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::layer::Layer where T: 'static + ?core::marker::Sized
pub fn vyre_spec::layer::Layer::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::layer::Layer where T: ?core::marker::Sized
pub fn vyre_spec::layer::Layer::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::layer::Layer where T: ?core::marker::Sized
pub fn vyre_spec::layer::Layer::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::layer::Layer where T: core::clone::Clone
pub unsafe fn vyre_spec::layer::Layer::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::layer::Layer
pub fn vyre_spec::layer::Layer::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::layer::Layer where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::MetadataCategory
pub vyre_spec::MetadataCategory::A
pub vyre_spec::MetadataCategory::B
pub vyre_spec::MetadataCategory::C
pub vyre_spec::MetadataCategory::Unclassified
impl vyre_spec::metadata_category::MetadataCategory
pub const fn vyre_spec::metadata_category::MetadataCategory::category_id(&self) -> &'static str
impl core::clone::Clone for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::clone(&self) -> vyre_spec::metadata_category::MetadataCategory
impl core::cmp::Eq for vyre_spec::metadata_category::MetadataCategory
impl core::cmp::PartialEq for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::eq(&self, other: &vyre_spec::metadata_category::MetadataCategory) -> bool
impl core::fmt::Debug for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::metadata_category::MetadataCategory
impl serde_core::ser::Serialize for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::metadata_category::MetadataCategory
impl core::marker::Send for vyre_spec::metadata_category::MetadataCategory
impl core::marker::Sync for vyre_spec::metadata_category::MetadataCategory
impl core::marker::Unpin for vyre_spec::metadata_category::MetadataCategory
impl core::marker::UnsafeUnpin for vyre_spec::metadata_category::MetadataCategory
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::metadata_category::MetadataCategory
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::metadata_category::MetadataCategory
impl<T, U> core::convert::Into<U> for vyre_spec::metadata_category::MetadataCategory where U: core::convert::From<T>
pub fn vyre_spec::metadata_category::MetadataCategory::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::metadata_category::MetadataCategory where U: core::convert::Into<T>
pub type vyre_spec::metadata_category::MetadataCategory::Error = core::convert::Infallible
pub fn vyre_spec::metadata_category::MetadataCategory::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::metadata_category::MetadataCategory where U: core::convert::TryFrom<T>
pub type vyre_spec::metadata_category::MetadataCategory::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::metadata_category::MetadataCategory::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::metadata_category::MetadataCategory where T: core::clone::Clone
pub type vyre_spec::metadata_category::MetadataCategory::Owned = T
pub fn vyre_spec::metadata_category::MetadataCategory::clone_into(&self, target: &mut T)
pub fn vyre_spec::metadata_category::MetadataCategory::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::metadata_category::MetadataCategory where T: 'static + ?core::marker::Sized
pub fn vyre_spec::metadata_category::MetadataCategory::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::metadata_category::MetadataCategory where T: ?core::marker::Sized
pub fn vyre_spec::metadata_category::MetadataCategory::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::metadata_category::MetadataCategory where T: ?core::marker::Sized
pub fn vyre_spec::metadata_category::MetadataCategory::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::metadata_category::MetadataCategory where T: core::clone::Clone
pub unsafe fn vyre_spec::metadata_category::MetadataCategory::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::metadata_category::MetadataCategory
pub fn vyre_spec::metadata_category::MetadataCategory::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::metadata_category::MetadataCategory where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::MonotonicDirection
pub vyre_spec::MonotonicDirection::NonDecreasing
pub vyre_spec::MonotonicDirection::NonIncreasing
impl core::clone::Clone for vyre_spec::monotonic_direction::MonotonicDirection
pub fn vyre_spec::monotonic_direction::MonotonicDirection::clone(&self) -> vyre_spec::monotonic_direction::MonotonicDirection
impl core::cmp::Eq for vyre_spec::monotonic_direction::MonotonicDirection
impl core::cmp::PartialEq for vyre_spec::monotonic_direction::MonotonicDirection
pub fn vyre_spec::monotonic_direction::MonotonicDirection::eq(&self, other: &vyre_spec::monotonic_direction::MonotonicDirection) -> bool
impl core::fmt::Debug for vyre_spec::monotonic_direction::MonotonicDirection
pub fn vyre_spec::monotonic_direction::MonotonicDirection::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::StructuralPartialEq for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::Freeze for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::Send for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::Sync for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::Unpin for vyre_spec::monotonic_direction::MonotonicDirection
impl core::marker::UnsafeUnpin for vyre_spec::monotonic_direction::MonotonicDirection
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::monotonic_direction::MonotonicDirection
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::monotonic_direction::MonotonicDirection
impl<T, U> core::convert::Into<U> for vyre_spec::monotonic_direction::MonotonicDirection where U: core::convert::From<T>
pub fn vyre_spec::monotonic_direction::MonotonicDirection::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::monotonic_direction::MonotonicDirection where U: core::convert::Into<T>
pub type vyre_spec::monotonic_direction::MonotonicDirection::Error = core::convert::Infallible
pub fn vyre_spec::monotonic_direction::MonotonicDirection::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::monotonic_direction::MonotonicDirection where U: core::convert::TryFrom<T>
pub type vyre_spec::monotonic_direction::MonotonicDirection::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::monotonic_direction::MonotonicDirection::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::monotonic_direction::MonotonicDirection where T: core::clone::Clone
pub type vyre_spec::monotonic_direction::MonotonicDirection::Owned = T
pub fn vyre_spec::monotonic_direction::MonotonicDirection::clone_into(&self, target: &mut T)
pub fn vyre_spec::monotonic_direction::MonotonicDirection::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::monotonic_direction::MonotonicDirection where T: 'static + ?core::marker::Sized
pub fn vyre_spec::monotonic_direction::MonotonicDirection::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::monotonic_direction::MonotonicDirection where T: ?core::marker::Sized
pub fn vyre_spec::monotonic_direction::MonotonicDirection::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::monotonic_direction::MonotonicDirection where T: ?core::marker::Sized
pub fn vyre_spec::monotonic_direction::MonotonicDirection::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::monotonic_direction::MonotonicDirection where T: core::clone::Clone
pub unsafe fn vyre_spec::monotonic_direction::MonotonicDirection::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::monotonic_direction::MonotonicDirection
pub fn vyre_spec::monotonic_direction::MonotonicDirection::from(t: T) -> T
#[non_exhaustive] #[repr(u32)] pub enum vyre_spec::PgNodeKind
pub vyre_spec::PgNodeKind::AddrOf = 13
pub vyre_spec::PgNodeKind::ArrayAccess = 16
pub vyre_spec::PgNodeKind::Assignment = 3
pub vyre_spec::PgNodeKind::Binary = 4
pub vyre_spec::PgNodeKind::Cast = 14
pub vyre_spec::PgNodeKind::Comparison = 5
pub vyre_spec::PgNodeKind::Deref = 12
pub vyre_spec::PgNodeKind::ForStmt = 9
pub vyre_spec::PgNodeKind::FunctionCall = 6
pub vyre_spec::PgNodeKind::FunctionDef = 7
pub vyre_spec::PgNodeKind::IfStmt = 8
pub vyre_spec::PgNodeKind::LiteralFloat = 20
pub vyre_spec::PgNodeKind::LiteralInt = 18
pub vyre_spec::PgNodeKind::LiteralStr = 19
pub vyre_spec::PgNodeKind::MemberAccess = 15
pub vyre_spec::PgNodeKind::ReturnStmt = 11
pub vyre_spec::PgNodeKind::StructDecl = 17
pub vyre_spec::PgNodeKind::VariableDecl = 1
pub vyre_spec::PgNodeKind::VariableUse = 2
pub vyre_spec::PgNodeKind::WhileStmt = 10
impl vyre_spec::pg_node_kind::PgNodeKind
pub const fn vyre_spec::pg_node_kind::PgNodeKind::from_u32(value: u32) -> core::option::Option<Self>
impl core::clone::Clone for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::clone(&self) -> vyre_spec::pg_node_kind::PgNodeKind
impl core::cmp::Eq for vyre_spec::pg_node_kind::PgNodeKind
impl core::cmp::PartialEq for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::eq(&self, other: &vyre_spec::pg_node_kind::PgNodeKind) -> bool
impl core::fmt::Debug for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::StructuralPartialEq for vyre_spec::pg_node_kind::PgNodeKind
impl serde_core::ser::Serialize for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::Send for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::Sync for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::Unpin for vyre_spec::pg_node_kind::PgNodeKind
impl core::marker::UnsafeUnpin for vyre_spec::pg_node_kind::PgNodeKind
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::pg_node_kind::PgNodeKind
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::pg_node_kind::PgNodeKind
impl<T, U> core::convert::Into<U> for vyre_spec::pg_node_kind::PgNodeKind where U: core::convert::From<T>
pub fn vyre_spec::pg_node_kind::PgNodeKind::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::pg_node_kind::PgNodeKind where U: core::convert::Into<T>
pub type vyre_spec::pg_node_kind::PgNodeKind::Error = core::convert::Infallible
pub fn vyre_spec::pg_node_kind::PgNodeKind::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::pg_node_kind::PgNodeKind where U: core::convert::TryFrom<T>
pub type vyre_spec::pg_node_kind::PgNodeKind::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::pg_node_kind::PgNodeKind::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::pg_node_kind::PgNodeKind where T: core::clone::Clone
pub type vyre_spec::pg_node_kind::PgNodeKind::Owned = T
pub fn vyre_spec::pg_node_kind::PgNodeKind::clone_into(&self, target: &mut T)
pub fn vyre_spec::pg_node_kind::PgNodeKind::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::pg_node_kind::PgNodeKind where T: 'static + ?core::marker::Sized
pub fn vyre_spec::pg_node_kind::PgNodeKind::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::pg_node_kind::PgNodeKind where T: ?core::marker::Sized
pub fn vyre_spec::pg_node_kind::PgNodeKind::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::pg_node_kind::PgNodeKind where T: ?core::marker::Sized
pub fn vyre_spec::pg_node_kind::PgNodeKind::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::pg_node_kind::PgNodeKind where T: core::clone::Clone
pub unsafe fn vyre_spec::pg_node_kind::PgNodeKind::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::pg_node_kind::PgNodeKind
pub fn vyre_spec::pg_node_kind::PgNodeKind::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::pg_node_kind::PgNodeKind where T: for<'de> serde_core::de::Deserialize<'de>
pub enum vyre_spec::QuantizationScale
pub vyre_spec::QuantizationScale::PerChannel
pub vyre_spec::QuantizationScale::PerChannel::axis: u32
pub vyre_spec::QuantizationScale::PerGroup
pub vyre_spec::QuantizationScale::PerGroup::group_size: u32
pub vyre_spec::QuantizationScale::PerTensor
impl core::clone::Clone for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::clone(&self) -> vyre_spec::data_type::QuantizationScale
impl core::cmp::Eq for vyre_spec::data_type::QuantizationScale
impl core::cmp::PartialEq for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::eq(&self, other: &vyre_spec::data_type::QuantizationScale) -> bool
impl core::fmt::Debug for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::data_type::QuantizationScale
impl serde_core::ser::Serialize for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::data_type::QuantizationScale
impl core::marker::Send for vyre_spec::data_type::QuantizationScale
impl core::marker::Sync for vyre_spec::data_type::QuantizationScale
impl core::marker::Unpin for vyre_spec::data_type::QuantizationScale
impl core::marker::UnsafeUnpin for vyre_spec::data_type::QuantizationScale
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::data_type::QuantizationScale
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::data_type::QuantizationScale
impl<T, U> core::convert::Into<U> for vyre_spec::data_type::QuantizationScale where U: core::convert::From<T>
pub fn vyre_spec::data_type::QuantizationScale::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::data_type::QuantizationScale where U: core::convert::Into<T>
pub type vyre_spec::data_type::QuantizationScale::Error = core::convert::Infallible
pub fn vyre_spec::data_type::QuantizationScale::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::data_type::QuantizationScale where U: core::convert::TryFrom<T>
pub type vyre_spec::data_type::QuantizationScale::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::data_type::QuantizationScale::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::data_type::QuantizationScale where T: core::clone::Clone
pub type vyre_spec::data_type::QuantizationScale::Owned = T
pub fn vyre_spec::data_type::QuantizationScale::clone_into(&self, target: &mut T)
pub fn vyre_spec::data_type::QuantizationScale::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::data_type::QuantizationScale where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationScale::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::data_type::QuantizationScale where T: 'static + ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationScale::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::data_type::QuantizationScale where T: ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationScale::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::data_type::QuantizationScale where T: ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationScale::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::data_type::QuantizationScale where T: core::clone::Clone
pub unsafe fn vyre_spec::data_type::QuantizationScale::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::data_type::QuantizationScale
pub fn vyre_spec::data_type::QuantizationScale::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::data_type::QuantizationScale where T: for<'de> serde_core::de::Deserialize<'de>
pub enum vyre_spec::QuantizationZeroPoint
pub vyre_spec::QuantizationZeroPoint::Absent
pub vyre_spec::QuantizationZeroPoint::PerChannel
pub vyre_spec::QuantizationZeroPoint::PerChannel::axis: u32
pub vyre_spec::QuantizationZeroPoint::PerGroup
pub vyre_spec::QuantizationZeroPoint::PerGroup::group_size: u32
pub vyre_spec::QuantizationZeroPoint::PerTensor
impl core::clone::Clone for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::clone(&self) -> vyre_spec::data_type::QuantizationZeroPoint
impl core::cmp::Eq for vyre_spec::data_type::QuantizationZeroPoint
impl core::cmp::PartialEq for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::eq(&self, other: &vyre_spec::data_type::QuantizationZeroPoint) -> bool
impl core::fmt::Debug for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::data_type::QuantizationZeroPoint
impl serde_core::ser::Serialize for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::data_type::QuantizationZeroPoint
impl core::marker::Send for vyre_spec::data_type::QuantizationZeroPoint
impl core::marker::Sync for vyre_spec::data_type::QuantizationZeroPoint
impl core::marker::Unpin for vyre_spec::data_type::QuantizationZeroPoint
impl core::marker::UnsafeUnpin for vyre_spec::data_type::QuantizationZeroPoint
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::data_type::QuantizationZeroPoint
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::data_type::QuantizationZeroPoint
impl<T, U> core::convert::Into<U> for vyre_spec::data_type::QuantizationZeroPoint where U: core::convert::From<T>
pub fn vyre_spec::data_type::QuantizationZeroPoint::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::data_type::QuantizationZeroPoint where U: core::convert::Into<T>
pub type vyre_spec::data_type::QuantizationZeroPoint::Error = core::convert::Infallible
pub fn vyre_spec::data_type::QuantizationZeroPoint::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::data_type::QuantizationZeroPoint where U: core::convert::TryFrom<T>
pub type vyre_spec::data_type::QuantizationZeroPoint::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::data_type::QuantizationZeroPoint::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::data_type::QuantizationZeroPoint where T: core::clone::Clone
pub type vyre_spec::data_type::QuantizationZeroPoint::Owned = T
pub fn vyre_spec::data_type::QuantizationZeroPoint::clone_into(&self, target: &mut T)
pub fn vyre_spec::data_type::QuantizationZeroPoint::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::data_type::QuantizationZeroPoint where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationZeroPoint::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::data_type::QuantizationZeroPoint where T: 'static + ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationZeroPoint::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::data_type::QuantizationZeroPoint where T: ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationZeroPoint::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::data_type::QuantizationZeroPoint where T: ?core::marker::Sized
pub fn vyre_spec::data_type::QuantizationZeroPoint::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::data_type::QuantizationZeroPoint where T: core::clone::Clone
pub unsafe fn vyre_spec::data_type::QuantizationZeroPoint::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::data_type::QuantizationZeroPoint
pub fn vyre_spec::data_type::QuantizationZeroPoint::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::data_type::QuantizationZeroPoint where T: for<'de> serde_core::de::Deserialize<'de>
pub enum vyre_spec::Semiring
pub vyre_spec::Semiring::BoolAnd
pub vyre_spec::Semiring::BoolOr
pub vyre_spec::Semiring::Gf2
pub vyre_spec::Semiring::Lineage
pub vyre_spec::Semiring::MaxPlus
pub vyre_spec::Semiring::MaxTimes
pub vyre_spec::Semiring::MinPlus
pub vyre_spec::Semiring::Real
impl vyre_spec::semiring::Semiring
pub const fn vyre_spec::semiring::Semiring::identity(self) -> u32
impl core::clone::Clone for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::clone(&self) -> vyre_spec::semiring::Semiring
impl core::cmp::Eq for vyre_spec::semiring::Semiring
impl core::cmp::PartialEq for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::eq(&self, other: &vyre_spec::semiring::Semiring) -> bool
impl core::fmt::Debug for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::semiring::Semiring
impl core::marker::StructuralPartialEq for vyre_spec::semiring::Semiring
impl serde_core::ser::Serialize for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::semiring::Semiring
impl core::marker::Send for vyre_spec::semiring::Semiring
impl core::marker::Sync for vyre_spec::semiring::Semiring
impl core::marker::Unpin for vyre_spec::semiring::Semiring
impl core::marker::UnsafeUnpin for vyre_spec::semiring::Semiring
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::semiring::Semiring
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::semiring::Semiring
impl<T, U> core::convert::Into<U> for vyre_spec::semiring::Semiring where U: core::convert::From<T>
pub fn vyre_spec::semiring::Semiring::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::semiring::Semiring where U: core::convert::Into<T>
pub type vyre_spec::semiring::Semiring::Error = core::convert::Infallible
pub fn vyre_spec::semiring::Semiring::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::semiring::Semiring where U: core::convert::TryFrom<T>
pub type vyre_spec::semiring::Semiring::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::semiring::Semiring::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::semiring::Semiring where T: core::clone::Clone
pub type vyre_spec::semiring::Semiring::Owned = T
pub fn vyre_spec::semiring::Semiring::clone_into(&self, target: &mut T)
pub fn vyre_spec::semiring::Semiring::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::semiring::Semiring where T: 'static + ?core::marker::Sized
pub fn vyre_spec::semiring::Semiring::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::semiring::Semiring where T: ?core::marker::Sized
pub fn vyre_spec::semiring::Semiring::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::semiring::Semiring where T: ?core::marker::Sized
pub fn vyre_spec::semiring::Semiring::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::semiring::Semiring where T: core::clone::Clone
pub unsafe fn vyre_spec::semiring::Semiring::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::semiring::Semiring
pub fn vyre_spec::semiring::Semiring::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::semiring::Semiring where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::SideEffectClass
pub vyre_spec::SideEffectClass::Atomic
pub vyre_spec::SideEffectClass::Pure
pub vyre_spec::SideEffectClass::ReadsMemory
pub vyre_spec::SideEffectClass::Synchronizing
pub vyre_spec::SideEffectClass::WritesMemory
impl core::clone::Clone for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::clone(&self) -> vyre_spec::op_contract::SideEffectClass
impl core::cmp::Eq for vyre_spec::op_contract::SideEffectClass
impl core::cmp::PartialEq for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::eq(&self, other: &vyre_spec::op_contract::SideEffectClass) -> bool
impl core::fmt::Debug for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::op_contract::SideEffectClass
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::SideEffectClass
impl serde_core::ser::Serialize for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::SideEffectClass
impl core::marker::Send for vyre_spec::op_contract::SideEffectClass
impl core::marker::Sync for vyre_spec::op_contract::SideEffectClass
impl core::marker::Unpin for vyre_spec::op_contract::SideEffectClass
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::SideEffectClass
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::SideEffectClass
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::SideEffectClass
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::SideEffectClass where U: core::convert::From<T>
pub fn vyre_spec::op_contract::SideEffectClass::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::SideEffectClass where U: core::convert::Into<T>
pub type vyre_spec::op_contract::SideEffectClass::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::SideEffectClass::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::SideEffectClass where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::SideEffectClass::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::SideEffectClass::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::SideEffectClass where T: core::clone::Clone
pub type vyre_spec::op_contract::SideEffectClass::Owned = T
pub fn vyre_spec::op_contract::SideEffectClass::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::SideEffectClass::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::SideEffectClass where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::SideEffectClass::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::SideEffectClass where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::SideEffectClass::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::SideEffectClass where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::SideEffectClass::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::SideEffectClass where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::SideEffectClass::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::SideEffectClass
pub fn vyre_spec::op_contract::SideEffectClass::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::SideEffectClass where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::SubgroupReduceOp
pub vyre_spec::SubgroupReduceOp::Add
pub vyre_spec::SubgroupReduceOp::And
pub vyre_spec::SubgroupReduceOp::Max
pub vyre_spec::SubgroupReduceOp::Min
pub vyre_spec::SubgroupReduceOp::Mul
pub vyre_spec::SubgroupReduceOp::Or
pub vyre_spec::SubgroupReduceOp::Xor
impl vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub const vyre_spec::subgroup_reduce_op::SubgroupReduceOp::ALL: [Self; 7]
pub const fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::as_str(self) -> &'static str
pub const fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::builtin_wire_tag(self) -> u8
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::combine_f32(self, acc: f32, lane: f32) -> core::option::Option<f32>
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::f32_identity(self) -> core::option::Option<f32>
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::from_wire_tag(tag: u8) -> core::result::Result<Self, alloc::string::String>
pub const fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::is_bitwise(self) -> bool
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::reduce_u32(self, lanes: impl core::iter::traits::collect::IntoIterator<Item = u32>) -> u32
impl core::clone::Clone for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::clone(&self) -> vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::cmp::Eq for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::cmp::PartialEq for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::eq(&self, other: &vyre_spec::subgroup_reduce_op::SubgroupReduceOp) -> bool
impl core::fmt::Debug for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::StructuralPartialEq for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl serde_core::ser::Serialize for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::Send for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::Sync for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::Unpin for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::marker::UnsafeUnpin for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
impl<T, U> core::convert::Into<U> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where U: core::convert::From<T>
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where U: core::convert::Into<T>
pub type vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Error = core::convert::Infallible
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where U: core::convert::TryFrom<T>
pub type vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: core::clone::Clone
pub type vyre_spec::subgroup_reduce_op::SubgroupReduceOp::Owned = T
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: ?core::marker::Sized
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: ?core::marker::Sized
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: core::clone::Clone
pub unsafe fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::subgroup_reduce_op::SubgroupReduceOp
pub fn vyre_spec::subgroup_reduce_op::SubgroupReduceOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::subgroup_reduce_op::SubgroupReduceOp where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::TernaryOp
pub vyre_spec::TernaryOp::Fma
pub vyre_spec::TernaryOp::Opaque(vyre_spec::extension::ExtensionTernaryOpId)
pub vyre_spec::TernaryOp::Select
impl vyre_spec::ternary_op::TernaryOp
pub const fn vyre_spec::ternary_op::TernaryOp::builtin_wire_tag(&self) -> core::option::Option<u8>
impl core::clone::Clone for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::clone(&self) -> vyre_spec::ternary_op::TernaryOp
impl core::cmp::Eq for vyre_spec::ternary_op::TernaryOp
impl core::cmp::PartialEq for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::eq(&self, other: &vyre_spec::ternary_op::TernaryOp) -> bool
impl core::fmt::Debug for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::ternary_op::TernaryOp
impl core::marker::StructuralPartialEq for vyre_spec::ternary_op::TernaryOp
impl serde_core::ser::Serialize for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::ternary_op::TernaryOp
impl core::marker::Send for vyre_spec::ternary_op::TernaryOp
impl core::marker::Sync for vyre_spec::ternary_op::TernaryOp
impl core::marker::Unpin for vyre_spec::ternary_op::TernaryOp
impl core::marker::UnsafeUnpin for vyre_spec::ternary_op::TernaryOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::ternary_op::TernaryOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::ternary_op::TernaryOp
impl<T, U> core::convert::Into<U> for vyre_spec::ternary_op::TernaryOp where U: core::convert::From<T>
pub fn vyre_spec::ternary_op::TernaryOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::ternary_op::TernaryOp where U: core::convert::Into<T>
pub type vyre_spec::ternary_op::TernaryOp::Error = core::convert::Infallible
pub fn vyre_spec::ternary_op::TernaryOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::ternary_op::TernaryOp where U: core::convert::TryFrom<T>
pub type vyre_spec::ternary_op::TernaryOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::ternary_op::TernaryOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::ternary_op::TernaryOp where T: core::clone::Clone
pub type vyre_spec::ternary_op::TernaryOp::Owned = T
pub fn vyre_spec::ternary_op::TernaryOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::ternary_op::TernaryOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::ternary_op::TernaryOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::ternary_op::TernaryOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::ternary_op::TernaryOp where T: ?core::marker::Sized
pub fn vyre_spec::ternary_op::TernaryOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::ternary_op::TernaryOp where T: ?core::marker::Sized
pub fn vyre_spec::ternary_op::TernaryOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::ternary_op::TernaryOp where T: core::clone::Clone
pub unsafe fn vyre_spec::ternary_op::TernaryOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::ternary_op::TernaryOp
pub fn vyre_spec::ternary_op::TernaryOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::ternary_op::TernaryOp where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::UnOp
pub vyre_spec::UnOp::Abs
pub vyre_spec::UnOp::Acos
pub vyre_spec::UnOp::Asin
pub vyre_spec::UnOp::Atan
pub vyre_spec::UnOp::BitNot
pub vyre_spec::UnOp::Ceil
pub vyre_spec::UnOp::Clz
pub vyre_spec::UnOp::Cos
pub vyre_spec::UnOp::Cosh
pub vyre_spec::UnOp::Ctz
pub vyre_spec::UnOp::Exp
pub vyre_spec::UnOp::Exp2
pub vyre_spec::UnOp::Floor
pub vyre_spec::UnOp::InverseSqrt
pub vyre_spec::UnOp::IsFinite
pub vyre_spec::UnOp::IsInf
pub vyre_spec::UnOp::IsNan
pub vyre_spec::UnOp::Log
pub vyre_spec::UnOp::Log2
pub vyre_spec::UnOp::LogicalNot
pub vyre_spec::UnOp::Negate
pub vyre_spec::UnOp::Opaque(vyre_spec::extension::ExtensionUnOpId)
pub vyre_spec::UnOp::Popcount
pub vyre_spec::UnOp::Reciprocal
pub vyre_spec::UnOp::ReverseBits
pub vyre_spec::UnOp::Round
pub vyre_spec::UnOp::Sign
pub vyre_spec::UnOp::Sin
pub vyre_spec::UnOp::Sinh
pub vyre_spec::UnOp::Sqrt
pub vyre_spec::UnOp::Tan
pub vyre_spec::UnOp::Tanh
pub vyre_spec::UnOp::Trunc
pub vyre_spec::UnOp::Unpack4High
pub vyre_spec::UnOp::Unpack4Low
pub vyre_spec::UnOp::Unpack8High
pub vyre_spec::UnOp::Unpack8Low
impl vyre_spec::un_op::UnOp
pub const fn vyre_spec::un_op::UnOp::builtin_wire_tag(&self) -> core::option::Option<u8>
impl core::clone::Clone for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::clone(&self) -> vyre_spec::un_op::UnOp
impl core::cmp::Eq for vyre_spec::un_op::UnOp
impl core::cmp::PartialEq for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::eq(&self, other: &vyre_spec::un_op::UnOp) -> bool
impl core::fmt::Debug for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::un_op::UnOp
impl serde_core::ser::Serialize for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::un_op::UnOp
impl core::marker::Send for vyre_spec::un_op::UnOp
impl core::marker::Sync for vyre_spec::un_op::UnOp
impl core::marker::Unpin for vyre_spec::un_op::UnOp
impl core::marker::UnsafeUnpin for vyre_spec::un_op::UnOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::un_op::UnOp
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::un_op::UnOp
impl<T, U> core::convert::Into<U> for vyre_spec::un_op::UnOp where U: core::convert::From<T>
pub fn vyre_spec::un_op::UnOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::un_op::UnOp where U: core::convert::Into<T>
pub type vyre_spec::un_op::UnOp::Error = core::convert::Infallible
pub fn vyre_spec::un_op::UnOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::un_op::UnOp where U: core::convert::TryFrom<T>
pub type vyre_spec::un_op::UnOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::un_op::UnOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::un_op::UnOp where T: core::clone::Clone
pub type vyre_spec::un_op::UnOp::Owned = T
pub fn vyre_spec::un_op::UnOp::clone_into(&self, target: &mut T)
pub fn vyre_spec::un_op::UnOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::un_op::UnOp where T: 'static + ?core::marker::Sized
pub fn vyre_spec::un_op::UnOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::un_op::UnOp where T: ?core::marker::Sized
pub fn vyre_spec::un_op::UnOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::un_op::UnOp where T: ?core::marker::Sized
pub fn vyre_spec::un_op::UnOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::un_op::UnOp where T: core::clone::Clone
pub unsafe fn vyre_spec::un_op::UnOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::un_op::UnOp
pub fn vyre_spec::un_op::UnOp::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::un_op::UnOp where T: for<'de> serde_core::de::Deserialize<'de>
#[non_exhaustive] pub enum vyre_spec::Verification
pub vyre_spec::Verification::ExhaustiveFloat
pub vyre_spec::Verification::ExhaustiveFloat::typ: vyre_spec::float_type::FloatType
pub vyre_spec::Verification::ExhaustiveU16
pub vyre_spec::Verification::ExhaustiveU8
pub vyre_spec::Verification::WitnessedU32
pub vyre_spec::Verification::WitnessedU32::count: u64
pub vyre_spec::Verification::WitnessedU32::seed: u64
impl vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::witness_count(&self) -> core::option::Option<u64>
impl core::clone::Clone for vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::clone(&self) -> vyre_spec::verification::Verification
impl core::cmp::Eq for vyre_spec::verification::Verification
impl core::cmp::PartialEq for vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::eq(&self, other: &vyre_spec::verification::Verification) -> bool
impl core::fmt::Debug for vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::verification::Verification
impl core::marker::Freeze for vyre_spec::verification::Verification
impl core::marker::Send for vyre_spec::verification::Verification
impl core::marker::Sync for vyre_spec::verification::Verification
impl core::marker::Unpin for vyre_spec::verification::Verification
impl core::marker::UnsafeUnpin for vyre_spec::verification::Verification
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::verification::Verification
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::verification::Verification
impl<T, U> core::convert::Into<U> for vyre_spec::verification::Verification where U: core::convert::From<T>
pub fn vyre_spec::verification::Verification::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::verification::Verification where U: core::convert::Into<T>
pub type vyre_spec::verification::Verification::Error = core::convert::Infallible
pub fn vyre_spec::verification::Verification::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::verification::Verification where U: core::convert::TryFrom<T>
pub type vyre_spec::verification::Verification::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::verification::Verification::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::verification::Verification where T: core::clone::Clone
pub type vyre_spec::verification::Verification::Owned = T
pub fn vyre_spec::verification::Verification::clone_into(&self, target: &mut T)
pub fn vyre_spec::verification::Verification::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::verification::Verification where T: 'static + ?core::marker::Sized
pub fn vyre_spec::verification::Verification::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::verification::Verification where T: ?core::marker::Sized
pub fn vyre_spec::verification::Verification::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::verification::Verification where T: ?core::marker::Sized
pub fn vyre_spec::verification::Verification::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::verification::Verification where T: core::clone::Clone
pub unsafe fn vyre_spec::verification::Verification::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::verification::Verification
pub fn vyre_spec::verification::Verification::from(t: T) -> T
pub struct vyre_spec::AdversarialInput
pub vyre_spec::AdversarialInput::input: &'static [u8]
pub vyre_spec::AdversarialInput::reason: &'static str
impl core::clone::Clone for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::clone(&self) -> vyre_spec::adversarial_input::AdversarialInput
impl core::cmp::Eq for vyre_spec::adversarial_input::AdversarialInput
impl core::cmp::PartialEq for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::eq(&self, other: &vyre_spec::adversarial_input::AdversarialInput) -> bool
impl core::fmt::Debug for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::Freeze for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::Send for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::Sync for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::Unpin for vyre_spec::adversarial_input::AdversarialInput
impl core::marker::UnsafeUnpin for vyre_spec::adversarial_input::AdversarialInput
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::adversarial_input::AdversarialInput
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::adversarial_input::AdversarialInput
impl<T, U> core::convert::Into<U> for vyre_spec::adversarial_input::AdversarialInput where U: core::convert::From<T>
pub fn vyre_spec::adversarial_input::AdversarialInput::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::adversarial_input::AdversarialInput where U: core::convert::Into<T>
pub type vyre_spec::adversarial_input::AdversarialInput::Error = core::convert::Infallible
pub fn vyre_spec::adversarial_input::AdversarialInput::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::adversarial_input::AdversarialInput where U: core::convert::TryFrom<T>
pub type vyre_spec::adversarial_input::AdversarialInput::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::adversarial_input::AdversarialInput::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::adversarial_input::AdversarialInput where T: core::clone::Clone
pub type vyre_spec::adversarial_input::AdversarialInput::Owned = T
pub fn vyre_spec::adversarial_input::AdversarialInput::clone_into(&self, target: &mut T)
pub fn vyre_spec::adversarial_input::AdversarialInput::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::adversarial_input::AdversarialInput where T: 'static + ?core::marker::Sized
pub fn vyre_spec::adversarial_input::AdversarialInput::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::adversarial_input::AdversarialInput where T: ?core::marker::Sized
pub fn vyre_spec::adversarial_input::AdversarialInput::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::adversarial_input::AdversarialInput where T: ?core::marker::Sized
pub fn vyre_spec::adversarial_input::AdversarialInput::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::adversarial_input::AdversarialInput where T: core::clone::Clone
pub unsafe fn vyre_spec::adversarial_input::AdversarialInput::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::adversarial_input::AdversarialInput
pub fn vyre_spec::adversarial_input::AdversarialInput::from(t: T) -> T
pub struct vyre_spec::Backend
impl vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::id(&self) -> &str
pub fn vyre_spec::intrinsic_descriptor::Backend::name(&self) -> &str
pub fn vyre_spec::intrinsic_descriptor::Backend::named(id: impl core::convert::Into<vyre_spec::intrinsic_descriptor::BackendId>, name: impl core::convert::Into<alloc::sync::Arc<str>>) -> Self
pub fn vyre_spec::intrinsic_descriptor::Backend::new(id: impl core::convert::Into<vyre_spec::intrinsic_descriptor::BackendId>) -> Self
impl core::clone::Clone for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::clone(&self) -> vyre_spec::intrinsic_descriptor::Backend
impl core::cmp::Eq for vyre_spec::intrinsic_descriptor::Backend
impl core::cmp::PartialEq for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::eq(&self, other: &vyre_spec::intrinsic_descriptor::Backend) -> bool
impl core::convert::From<&str> for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::from(id: &str) -> Self
impl core::convert::From<&vyre_spec::intrinsic_descriptor::Backend> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(backend: &vyre_spec::intrinsic_descriptor::Backend) -> Self
impl core::convert::From<vyre_spec::intrinsic_descriptor::BackendId> for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::from(id: vyre_spec::intrinsic_descriptor::BackendId) -> Self
impl core::fmt::Debug for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::Freeze for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::Send for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::Sync for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::Unpin for vyre_spec::intrinsic_descriptor::Backend
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_descriptor::Backend
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_descriptor::Backend
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_descriptor::Backend
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_descriptor::Backend where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_descriptor::Backend::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_descriptor::Backend where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_descriptor::Backend::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_descriptor::Backend::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_descriptor::Backend where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_descriptor::Backend::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_descriptor::Backend::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_descriptor::Backend where T: core::clone::Clone
pub type vyre_spec::intrinsic_descriptor::Backend::Owned = T
pub fn vyre_spec::intrinsic_descriptor::Backend::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_descriptor::Backend::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::intrinsic_descriptor::Backend where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::Backend::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_descriptor::Backend where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::Backend::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_descriptor::Backend where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::Backend::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_descriptor::Backend where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_descriptor::Backend::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::from(t: T) -> T
pub struct vyre_spec::BackendAvailabilityPredicate
impl vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::available(&self, op: &str) -> bool
pub const fn vyre_spec::category::BackendAvailabilityPredicate::new(predicate: fn(&str) -> bool) -> Self
impl core::clone::Clone for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::clone(&self) -> vyre_spec::category::BackendAvailabilityPredicate
impl core::fmt::Debug for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_spec::category::BackendAvailabilityPredicate
impl vyre_spec::category::BackendAvailability for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::available(&self, op: &str) -> bool
impl core::marker::Freeze for vyre_spec::category::BackendAvailabilityPredicate
impl core::marker::Send for vyre_spec::category::BackendAvailabilityPredicate
impl core::marker::Sync for vyre_spec::category::BackendAvailabilityPredicate
impl core::marker::Unpin for vyre_spec::category::BackendAvailabilityPredicate
impl core::marker::UnsafeUnpin for vyre_spec::category::BackendAvailabilityPredicate
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::category::BackendAvailabilityPredicate
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::category::BackendAvailabilityPredicate
impl<T, U> core::convert::Into<U> for vyre_spec::category::BackendAvailabilityPredicate where U: core::convert::From<T>
pub fn vyre_spec::category::BackendAvailabilityPredicate::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::category::BackendAvailabilityPredicate where U: core::convert::Into<T>
pub type vyre_spec::category::BackendAvailabilityPredicate::Error = core::convert::Infallible
pub fn vyre_spec::category::BackendAvailabilityPredicate::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::category::BackendAvailabilityPredicate where U: core::convert::TryFrom<T>
pub type vyre_spec::category::BackendAvailabilityPredicate::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::category::BackendAvailabilityPredicate::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::category::BackendAvailabilityPredicate where T: core::clone::Clone
pub type vyre_spec::category::BackendAvailabilityPredicate::Owned = T
pub fn vyre_spec::category::BackendAvailabilityPredicate::clone_into(&self, target: &mut T)
pub fn vyre_spec::category::BackendAvailabilityPredicate::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::category::BackendAvailabilityPredicate where T: 'static + ?core::marker::Sized
pub fn vyre_spec::category::BackendAvailabilityPredicate::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::category::BackendAvailabilityPredicate where T: ?core::marker::Sized
pub fn vyre_spec::category::BackendAvailabilityPredicate::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::category::BackendAvailabilityPredicate where T: ?core::marker::Sized
pub fn vyre_spec::category::BackendAvailabilityPredicate::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::category::BackendAvailabilityPredicate where T: core::clone::Clone
pub unsafe fn vyre_spec::category::BackendAvailabilityPredicate::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::from(t: T) -> T
pub struct vyre_spec::BackendId(_)
impl vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::as_str(&self) -> &str
pub fn vyre_spec::intrinsic_descriptor::BackendId::new(name: impl core::convert::Into<alloc::sync::Arc<str>>) -> Self
impl core::clone::Clone for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::clone(&self) -> vyre_spec::intrinsic_descriptor::BackendId
impl core::cmp::Eq for vyre_spec::intrinsic_descriptor::BackendId
impl core::cmp::PartialEq for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::eq(&self, other: &Self) -> bool
impl core::convert::From<&str> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(name: &str) -> Self
impl core::convert::From<&vyre_spec::intrinsic_descriptor::Backend> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(backend: &vyre_spec::intrinsic_descriptor::Backend) -> Self
impl core::convert::From<alloc::string::String> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(name: alloc::string::String) -> Self
impl core::convert::From<vyre_spec::intrinsic_descriptor::BackendId> for vyre_spec::intrinsic_descriptor::Backend
pub fn vyre_spec::intrinsic_descriptor::Backend::from(id: vyre_spec::intrinsic_descriptor::BackendId) -> Self
impl core::fmt::Debug for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::hash<H: core::hash::Hasher>(&self, state: &mut H)
impl core::marker::Freeze for vyre_spec::intrinsic_descriptor::BackendId
impl core::marker::Send for vyre_spec::intrinsic_descriptor::BackendId
impl core::marker::Sync for vyre_spec::intrinsic_descriptor::BackendId
impl core::marker::Unpin for vyre_spec::intrinsic_descriptor::BackendId
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_descriptor::BackendId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_descriptor::BackendId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_descriptor::BackendId
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_descriptor::BackendId where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_descriptor::BackendId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_descriptor::BackendId where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_descriptor::BackendId::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_descriptor::BackendId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_descriptor::BackendId where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_descriptor::BackendId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_descriptor::BackendId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_descriptor::BackendId where T: core::clone::Clone
pub type vyre_spec::intrinsic_descriptor::BackendId::Owned = T
pub fn vyre_spec::intrinsic_descriptor::BackendId::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_descriptor::BackendId::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_spec::intrinsic_descriptor::BackendId where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::BackendId::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_spec::intrinsic_descriptor::BackendId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::BackendId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_descriptor::BackendId where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::BackendId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_descriptor::BackendId where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::BackendId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_descriptor::BackendId where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_descriptor::BackendId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_descriptor::BackendId
pub fn vyre_spec::intrinsic_descriptor::BackendId::from(t: T) -> T
pub struct vyre_spec::CapabilityId(pub alloc::string::String)
impl vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::as_str(&self) -> &str
pub fn vyre_spec::op_contract::CapabilityId::new(name: impl core::convert::Into<alloc::string::String>) -> Self
impl core::clone::Clone for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::clone(&self) -> vyre_spec::op_contract::CapabilityId
impl core::cmp::Eq for vyre_spec::op_contract::CapabilityId
impl core::cmp::PartialEq for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::eq(&self, other: &vyre_spec::op_contract::CapabilityId) -> bool
impl core::fmt::Debug for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::CapabilityId
impl serde_core::ser::Serialize for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::CapabilityId
impl core::marker::Send for vyre_spec::op_contract::CapabilityId
impl core::marker::Sync for vyre_spec::op_contract::CapabilityId
impl core::marker::Unpin for vyre_spec::op_contract::CapabilityId
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::CapabilityId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::CapabilityId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::CapabilityId
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::CapabilityId where U: core::convert::From<T>
pub fn vyre_spec::op_contract::CapabilityId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::CapabilityId where U: core::convert::Into<T>
pub type vyre_spec::op_contract::CapabilityId::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::CapabilityId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::CapabilityId where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::CapabilityId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::CapabilityId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::CapabilityId where T: core::clone::Clone
pub type vyre_spec::op_contract::CapabilityId::Owned = T
pub fn vyre_spec::op_contract::CapabilityId::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::CapabilityId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::CapabilityId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::CapabilityId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::CapabilityId where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::CapabilityId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::CapabilityId where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::CapabilityId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::CapabilityId where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::CapabilityId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::CapabilityId
pub fn vyre_spec::op_contract::CapabilityId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::CapabilityId where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::CommGroup(pub u32)
impl vyre_spec::collective_op::CommGroup
pub const vyre_spec::collective_op::CommGroup::WORLD: Self
pub const fn vyre_spec::collective_op::CommGroup::as_u32(self) -> u32
impl core::clone::Clone for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::clone(&self) -> vyre_spec::collective_op::CommGroup
impl core::cmp::Eq for vyre_spec::collective_op::CommGroup
impl core::cmp::PartialEq for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::eq(&self, other: &vyre_spec::collective_op::CommGroup) -> bool
impl core::fmt::Debug for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::collective_op::CommGroup
impl core::marker::StructuralPartialEq for vyre_spec::collective_op::CommGroup
impl serde_core::ser::Serialize for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::collective_op::CommGroup
impl core::marker::Send for vyre_spec::collective_op::CommGroup
impl core::marker::Sync for vyre_spec::collective_op::CommGroup
impl core::marker::Unpin for vyre_spec::collective_op::CommGroup
impl core::marker::UnsafeUnpin for vyre_spec::collective_op::CommGroup
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::collective_op::CommGroup
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::collective_op::CommGroup
impl<T, U> core::convert::Into<U> for vyre_spec::collective_op::CommGroup where U: core::convert::From<T>
pub fn vyre_spec::collective_op::CommGroup::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::collective_op::CommGroup where U: core::convert::Into<T>
pub type vyre_spec::collective_op::CommGroup::Error = core::convert::Infallible
pub fn vyre_spec::collective_op::CommGroup::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::collective_op::CommGroup where U: core::convert::TryFrom<T>
pub type vyre_spec::collective_op::CommGroup::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::collective_op::CommGroup::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::collective_op::CommGroup where T: core::clone::Clone
pub type vyre_spec::collective_op::CommGroup::Owned = T
pub fn vyre_spec::collective_op::CommGroup::clone_into(&self, target: &mut T)
pub fn vyre_spec::collective_op::CommGroup::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::collective_op::CommGroup where T: 'static + ?core::marker::Sized
pub fn vyre_spec::collective_op::CommGroup::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::collective_op::CommGroup where T: ?core::marker::Sized
pub fn vyre_spec::collective_op::CommGroup::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::collective_op::CommGroup where T: ?core::marker::Sized
pub fn vyre_spec::collective_op::CommGroup::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::collective_op::CommGroup where T: core::clone::Clone
pub unsafe fn vyre_spec::collective_op::CommGroup::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::collective_op::CommGroup
pub fn vyre_spec::collective_op::CommGroup::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::collective_op::CommGroup where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::GoldenSample
pub vyre_spec::GoldenSample::expected: &'static [u8]
pub vyre_spec::GoldenSample::input: &'static [u8]
pub vyre_spec::GoldenSample::op_id: &'static str
pub vyre_spec::GoldenSample::reason: &'static str
impl core::clone::Clone for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::clone(&self) -> vyre_spec::golden_sample::GoldenSample
impl core::cmp::Eq for vyre_spec::golden_sample::GoldenSample
impl core::cmp::PartialEq for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::eq(&self, other: &vyre_spec::golden_sample::GoldenSample) -> bool
impl core::fmt::Debug for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::golden_sample::GoldenSample
impl core::marker::Freeze for vyre_spec::golden_sample::GoldenSample
impl core::marker::Send for vyre_spec::golden_sample::GoldenSample
impl core::marker::Sync for vyre_spec::golden_sample::GoldenSample
impl core::marker::Unpin for vyre_spec::golden_sample::GoldenSample
impl core::marker::UnsafeUnpin for vyre_spec::golden_sample::GoldenSample
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::golden_sample::GoldenSample
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::golden_sample::GoldenSample
impl<T, U> core::convert::Into<U> for vyre_spec::golden_sample::GoldenSample where U: core::convert::From<T>
pub fn vyre_spec::golden_sample::GoldenSample::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::golden_sample::GoldenSample where U: core::convert::Into<T>
pub type vyre_spec::golden_sample::GoldenSample::Error = core::convert::Infallible
pub fn vyre_spec::golden_sample::GoldenSample::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::golden_sample::GoldenSample where U: core::convert::TryFrom<T>
pub type vyre_spec::golden_sample::GoldenSample::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::golden_sample::GoldenSample::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::golden_sample::GoldenSample where T: core::clone::Clone
pub type vyre_spec::golden_sample::GoldenSample::Owned = T
pub fn vyre_spec::golden_sample::GoldenSample::clone_into(&self, target: &mut T)
pub fn vyre_spec::golden_sample::GoldenSample::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::golden_sample::GoldenSample where T: 'static + ?core::marker::Sized
pub fn vyre_spec::golden_sample::GoldenSample::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::golden_sample::GoldenSample where T: ?core::marker::Sized
pub fn vyre_spec::golden_sample::GoldenSample::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::golden_sample::GoldenSample where T: ?core::marker::Sized
pub fn vyre_spec::golden_sample::GoldenSample::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::golden_sample::GoldenSample where T: core::clone::Clone
pub unsafe fn vyre_spec::golden_sample::GoldenSample::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::golden_sample::GoldenSample
pub fn vyre_spec::golden_sample::GoldenSample::from(t: T) -> T
#[non_exhaustive] pub struct vyre_spec::IntrinsicDescriptor
impl vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::contract(&self) -> core::option::Option<&vyre_spec::op_contract::OperationContract>
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::cpu_fn(&self) -> vyre_spec::intrinsic_descriptor::CpuFn
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::hardware(&self) -> &'static str
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::name(&self) -> &'static str
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::new(name: &'static str, hardware: &'static str, cpu_fn: vyre_spec::intrinsic_descriptor::CpuFn) -> Self
pub const fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::with_contract(name: &'static str, hardware: &'static str, cpu_fn: vyre_spec::intrinsic_descriptor::CpuFn, contract: vyre_spec::op_contract::OperationContract) -> Self
impl core::clone::Clone for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::clone(&self) -> vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::fmt::Debug for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::marker::Send for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::marker::Sync for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::marker::Unpin for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: core::clone::Clone
pub type vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::Owned = T
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_descriptor::IntrinsicDescriptor
pub fn vyre_spec::intrinsic_descriptor::IntrinsicDescriptor::from(t: T) -> T
pub struct vyre_spec::IntrinsicLowering
pub vyre_spec::IntrinsicLowering::backend: vyre_spec::intrinsic_descriptor::BackendId
pub vyre_spec::IntrinsicLowering::name: &'static str
impl vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::new(backend: impl core::convert::Into<vyre_spec::intrinsic_descriptor::BackendId>, name: &'static str) -> Self
impl core::clone::Clone for vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::clone(&self) -> vyre_spec::intrinsic_table::IntrinsicLowering
impl core::cmp::Eq for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::cmp::PartialEq for vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::eq(&self, other: &vyre_spec::intrinsic_table::IntrinsicLowering) -> bool
impl core::fmt::Debug for vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::Freeze for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::Send for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::Sync for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::Unpin for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_table::IntrinsicLowering
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_table::IntrinsicLowering
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_table::IntrinsicLowering where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_table::IntrinsicLowering where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_table::IntrinsicLowering::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_table::IntrinsicLowering where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_table::IntrinsicLowering::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_table::IntrinsicLowering where T: core::clone::Clone
pub type vyre_spec::intrinsic_table::IntrinsicLowering::Owned = T
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::intrinsic_table::IntrinsicLowering where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_table::IntrinsicLowering where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_table::IntrinsicLowering where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_table::IntrinsicLowering where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_table::IntrinsicLowering::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_table::IntrinsicLowering
pub fn vyre_spec::intrinsic_table::IntrinsicLowering::from(t: T) -> T
pub struct vyre_spec::IntrinsicTable
pub vyre_spec::IntrinsicTable::lowerings: alloc::vec::Vec<vyre_spec::intrinsic_table::IntrinsicLowering>
impl vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::has_backend(&self, backend: &vyre_spec::intrinsic_descriptor::BackendId) -> bool
pub fn vyre_spec::intrinsic_table::IntrinsicTable::missing_backends<'a>(&'a self, required: &'a [vyre_spec::intrinsic_descriptor::BackendId]) -> impl core::iter::traits::iterator::Iterator<Item = &'a str> + 'a
impl core::clone::Clone for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::clone(&self) -> vyre_spec::intrinsic_table::IntrinsicTable
impl core::cmp::Eq for vyre_spec::intrinsic_table::IntrinsicTable
impl core::cmp::PartialEq for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::eq(&self, other: &vyre_spec::intrinsic_table::IntrinsicTable) -> bool
impl core::default::Default for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::default() -> vyre_spec::intrinsic_table::IntrinsicTable
impl core::fmt::Debug for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::Freeze for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::Send for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::Sync for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::Unpin for vyre_spec::intrinsic_table::IntrinsicTable
impl core::marker::UnsafeUnpin for vyre_spec::intrinsic_table::IntrinsicTable
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::intrinsic_table::IntrinsicTable
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::intrinsic_table::IntrinsicTable
impl<T, U> core::convert::Into<U> for vyre_spec::intrinsic_table::IntrinsicTable where U: core::convert::From<T>
pub fn vyre_spec::intrinsic_table::IntrinsicTable::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::intrinsic_table::IntrinsicTable where U: core::convert::Into<T>
pub type vyre_spec::intrinsic_table::IntrinsicTable::Error = core::convert::Infallible
pub fn vyre_spec::intrinsic_table::IntrinsicTable::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::intrinsic_table::IntrinsicTable where U: core::convert::TryFrom<T>
pub type vyre_spec::intrinsic_table::IntrinsicTable::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::intrinsic_table::IntrinsicTable::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::intrinsic_table::IntrinsicTable where T: core::clone::Clone
pub type vyre_spec::intrinsic_table::IntrinsicTable::Owned = T
pub fn vyre_spec::intrinsic_table::IntrinsicTable::clone_into(&self, target: &mut T)
pub fn vyre_spec::intrinsic_table::IntrinsicTable::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::intrinsic_table::IntrinsicTable where T: 'static + ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicTable::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::intrinsic_table::IntrinsicTable where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicTable::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::intrinsic_table::IntrinsicTable where T: ?core::marker::Sized
pub fn vyre_spec::intrinsic_table::IntrinsicTable::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::intrinsic_table::IntrinsicTable where T: core::clone::Clone
pub unsafe fn vyre_spec::intrinsic_table::IntrinsicTable::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::intrinsic_table::IntrinsicTable
pub fn vyre_spec::intrinsic_table::IntrinsicTable::from(t: T) -> T
pub struct vyre_spec::Invariant
pub vyre_spec::Invariant::category: vyre_spec::invariant_category::InvariantCategory
pub vyre_spec::Invariant::description: &'static str
pub vyre_spec::Invariant::id: vyre_spec::engine_invariant::InvariantId
pub vyre_spec::Invariant::name: &'static str
pub vyre_spec::Invariant::test_family: fn() -> &'static [vyre_spec::test_descriptor::TestDescriptor]
impl core::clone::Clone for vyre_spec::invariant::Invariant
pub fn vyre_spec::invariant::Invariant::clone(&self) -> vyre_spec::invariant::Invariant
impl core::fmt::Debug for vyre_spec::invariant::Invariant
pub fn vyre_spec::invariant::Invariant::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_spec::invariant::Invariant
impl core::marker::Send for vyre_spec::invariant::Invariant
impl core::marker::Sync for vyre_spec::invariant::Invariant
impl core::marker::Unpin for vyre_spec::invariant::Invariant
impl core::marker::UnsafeUnpin for vyre_spec::invariant::Invariant
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::invariant::Invariant
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::invariant::Invariant
impl<T, U> core::convert::Into<U> for vyre_spec::invariant::Invariant where U: core::convert::From<T>
pub fn vyre_spec::invariant::Invariant::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::invariant::Invariant where U: core::convert::Into<T>
pub type vyre_spec::invariant::Invariant::Error = core::convert::Infallible
pub fn vyre_spec::invariant::Invariant::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::invariant::Invariant where U: core::convert::TryFrom<T>
pub type vyre_spec::invariant::Invariant::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::invariant::Invariant::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::invariant::Invariant where T: core::clone::Clone
pub type vyre_spec::invariant::Invariant::Owned = T
pub fn vyre_spec::invariant::Invariant::clone_into(&self, target: &mut T)
pub fn vyre_spec::invariant::Invariant::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::invariant::Invariant where T: 'static + ?core::marker::Sized
pub fn vyre_spec::invariant::Invariant::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::invariant::Invariant where T: ?core::marker::Sized
pub fn vyre_spec::invariant::Invariant::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::invariant::Invariant where T: ?core::marker::Sized
pub fn vyre_spec::invariant::Invariant::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::invariant::Invariant where T: core::clone::Clone
pub unsafe fn vyre_spec::invariant::Invariant::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::invariant::Invariant
pub fn vyre_spec::invariant::Invariant::from(t: T) -> T
pub struct vyre_spec::KatVector
pub vyre_spec::KatVector::expected: &'static [u8]
pub vyre_spec::KatVector::input: &'static [u8]
pub vyre_spec::KatVector::source: &'static str
impl core::clone::Clone for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::clone(&self) -> vyre_spec::kat_vector::KatVector
impl core::cmp::Eq for vyre_spec::kat_vector::KatVector
impl core::cmp::PartialEq for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::eq(&self, other: &vyre_spec::kat_vector::KatVector) -> bool
impl core::fmt::Debug for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::kat_vector::KatVector
impl core::marker::Freeze for vyre_spec::kat_vector::KatVector
impl core::marker::Send for vyre_spec::kat_vector::KatVector
impl core::marker::Sync for vyre_spec::kat_vector::KatVector
impl core::marker::Unpin for vyre_spec::kat_vector::KatVector
impl core::marker::UnsafeUnpin for vyre_spec::kat_vector::KatVector
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::kat_vector::KatVector
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::kat_vector::KatVector
impl<T, U> core::convert::Into<U> for vyre_spec::kat_vector::KatVector where U: core::convert::From<T>
pub fn vyre_spec::kat_vector::KatVector::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::kat_vector::KatVector where U: core::convert::Into<T>
pub type vyre_spec::kat_vector::KatVector::Error = core::convert::Infallible
pub fn vyre_spec::kat_vector::KatVector::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::kat_vector::KatVector where U: core::convert::TryFrom<T>
pub type vyre_spec::kat_vector::KatVector::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::kat_vector::KatVector::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::kat_vector::KatVector where T: core::clone::Clone
pub type vyre_spec::kat_vector::KatVector::Owned = T
pub fn vyre_spec::kat_vector::KatVector::clone_into(&self, target: &mut T)
pub fn vyre_spec::kat_vector::KatVector::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::kat_vector::KatVector where T: 'static + ?core::marker::Sized
pub fn vyre_spec::kat_vector::KatVector::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::kat_vector::KatVector where T: ?core::marker::Sized
pub fn vyre_spec::kat_vector::KatVector::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::kat_vector::KatVector where T: ?core::marker::Sized
pub fn vyre_spec::kat_vector::KatVector::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::kat_vector::KatVector where T: core::clone::Clone
pub unsafe fn vyre_spec::kat_vector::KatVector::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::kat_vector::KatVector
pub fn vyre_spec::kat_vector::KatVector::from(t: T) -> T
pub struct vyre_spec::OpMetadata
pub vyre_spec::OpMetadata::archetype_signature: &'static str
pub vyre_spec::OpMetadata::category: vyre_spec::metadata_category::MetadataCategory
pub vyre_spec::OpMetadata::contract: core::option::Option<vyre_spec::op_contract::OperationContract>
pub vyre_spec::OpMetadata::description: &'static str
pub vyre_spec::OpMetadata::id: &'static str
pub vyre_spec::OpMetadata::layer: vyre_spec::layer::Layer
pub vyre_spec::OpMetadata::signature: &'static str
pub vyre_spec::OpMetadata::strictness: &'static str
pub vyre_spec::OpMetadata::version: u32
impl core::clone::Clone for vyre_spec::op_metadata::OpMetadata
pub fn vyre_spec::op_metadata::OpMetadata::clone(&self) -> vyre_spec::op_metadata::OpMetadata
impl core::cmp::Eq for vyre_spec::op_metadata::OpMetadata
impl core::cmp::PartialEq for vyre_spec::op_metadata::OpMetadata
pub fn vyre_spec::op_metadata::OpMetadata::eq(&self, other: &vyre_spec::op_metadata::OpMetadata) -> bool
impl core::fmt::Debug for vyre_spec::op_metadata::OpMetadata
pub fn vyre_spec::op_metadata::OpMetadata::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::op_metadata::OpMetadata
impl core::marker::Freeze for vyre_spec::op_metadata::OpMetadata
impl core::marker::Send for vyre_spec::op_metadata::OpMetadata
impl core::marker::Sync for vyre_spec::op_metadata::OpMetadata
impl core::marker::Unpin for vyre_spec::op_metadata::OpMetadata
impl core::marker::UnsafeUnpin for vyre_spec::op_metadata::OpMetadata
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_metadata::OpMetadata
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_metadata::OpMetadata
impl<T, U> core::convert::Into<U> for vyre_spec::op_metadata::OpMetadata where U: core::convert::From<T>
pub fn vyre_spec::op_metadata::OpMetadata::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_metadata::OpMetadata where U: core::convert::Into<T>
pub type vyre_spec::op_metadata::OpMetadata::Error = core::convert::Infallible
pub fn vyre_spec::op_metadata::OpMetadata::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_metadata::OpMetadata where U: core::convert::TryFrom<T>
pub type vyre_spec::op_metadata::OpMetadata::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_metadata::OpMetadata::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_metadata::OpMetadata where T: core::clone::Clone
pub type vyre_spec::op_metadata::OpMetadata::Owned = T
pub fn vyre_spec::op_metadata::OpMetadata::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_metadata::OpMetadata::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_metadata::OpMetadata where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_metadata::OpMetadata::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_metadata::OpMetadata where T: ?core::marker::Sized
pub fn vyre_spec::op_metadata::OpMetadata::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_metadata::OpMetadata where T: ?core::marker::Sized
pub fn vyre_spec::op_metadata::OpMetadata::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_metadata::OpMetadata where T: core::clone::Clone
pub unsafe fn vyre_spec::op_metadata::OpMetadata::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_metadata::OpMetadata
pub fn vyre_spec::op_metadata::OpMetadata::from(t: T) -> T
pub struct vyre_spec::OpSignature
pub vyre_spec::OpSignature::contract: core::option::Option<vyre_spec::op_contract::OperationContract>
pub vyre_spec::OpSignature::input_params: core::option::Option<alloc::vec::Vec<vyre_spec::op_signature::SignatureParam>>
pub vyre_spec::OpSignature::inputs: alloc::vec::Vec<vyre_spec::data_type::DataType>
pub vyre_spec::OpSignature::output: vyre_spec::data_type::DataType
pub vyre_spec::OpSignature::output_params: core::option::Option<alloc::vec::Vec<vyre_spec::op_signature::SignatureParam>>
impl vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::min_input_bytes(&self) -> usize
impl core::clone::Clone for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::clone(&self) -> vyre_spec::op_signature::OpSignature
impl core::cmp::Eq for vyre_spec::op_signature::OpSignature
impl core::cmp::PartialEq for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::eq(&self, other: &vyre_spec::op_signature::OpSignature) -> bool
impl core::fmt::Debug for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::op_signature::OpSignature
impl serde_core::ser::Serialize for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_signature::OpSignature
impl core::marker::Send for vyre_spec::op_signature::OpSignature
impl core::marker::Sync for vyre_spec::op_signature::OpSignature
impl core::marker::Unpin for vyre_spec::op_signature::OpSignature
impl core::marker::UnsafeUnpin for vyre_spec::op_signature::OpSignature
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_signature::OpSignature
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_signature::OpSignature
impl<T, U> core::convert::Into<U> for vyre_spec::op_signature::OpSignature where U: core::convert::From<T>
pub fn vyre_spec::op_signature::OpSignature::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_signature::OpSignature where U: core::convert::Into<T>
pub type vyre_spec::op_signature::OpSignature::Error = core::convert::Infallible
pub fn vyre_spec::op_signature::OpSignature::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_signature::OpSignature where U: core::convert::TryFrom<T>
pub type vyre_spec::op_signature::OpSignature::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_signature::OpSignature::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_signature::OpSignature where T: core::clone::Clone
pub type vyre_spec::op_signature::OpSignature::Owned = T
pub fn vyre_spec::op_signature::OpSignature::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_signature::OpSignature::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_signature::OpSignature where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_signature::OpSignature::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_signature::OpSignature where T: ?core::marker::Sized
pub fn vyre_spec::op_signature::OpSignature::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_signature::OpSignature where T: ?core::marker::Sized
pub fn vyre_spec::op_signature::OpSignature::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_signature::OpSignature where T: core::clone::Clone
pub unsafe fn vyre_spec::op_signature::OpSignature::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_signature::OpSignature
pub fn vyre_spec::op_signature::OpSignature::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_signature::OpSignature where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::OperationContract
pub vyre_spec::OperationContract::capability_requirements: core::option::Option<smallvec::SmallVec<[vyre_spec::op_contract::CapabilityId; 4]>>
pub vyre_spec::OperationContract::cost_hint: core::option::Option<vyre_spec::op_contract::CostHint>
pub vyre_spec::OperationContract::determinism: core::option::Option<vyre_spec::op_contract::DeterminismClass>
pub vyre_spec::OperationContract::side_effect: core::option::Option<vyre_spec::op_contract::SideEffectClass>
impl vyre_spec::op_contract::OperationContract
pub const fn vyre_spec::op_contract::OperationContract::none() -> Self
impl core::clone::Clone for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::clone(&self) -> vyre_spec::op_contract::OperationContract
impl core::cmp::Eq for vyre_spec::op_contract::OperationContract
impl core::cmp::PartialEq for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::eq(&self, other: &vyre_spec::op_contract::OperationContract) -> bool
impl core::default::Default for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::default() -> Self
impl core::fmt::Debug for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_spec::op_contract::OperationContract
impl serde_core::ser::Serialize for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::op_contract::OperationContract
impl core::marker::Send for vyre_spec::op_contract::OperationContract
impl core::marker::Sync for vyre_spec::op_contract::OperationContract
impl core::marker::Unpin for vyre_spec::op_contract::OperationContract
impl core::marker::UnsafeUnpin for vyre_spec::op_contract::OperationContract
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::op_contract::OperationContract
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::op_contract::OperationContract
impl<T, U> core::convert::Into<U> for vyre_spec::op_contract::OperationContract where U: core::convert::From<T>
pub fn vyre_spec::op_contract::OperationContract::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::op_contract::OperationContract where U: core::convert::Into<T>
pub type vyre_spec::op_contract::OperationContract::Error = core::convert::Infallible
pub fn vyre_spec::op_contract::OperationContract::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::op_contract::OperationContract where U: core::convert::TryFrom<T>
pub type vyre_spec::op_contract::OperationContract::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::op_contract::OperationContract::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::op_contract::OperationContract where T: core::clone::Clone
pub type vyre_spec::op_contract::OperationContract::Owned = T
pub fn vyre_spec::op_contract::OperationContract::clone_into(&self, target: &mut T)
pub fn vyre_spec::op_contract::OperationContract::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::op_contract::OperationContract where T: 'static + ?core::marker::Sized
pub fn vyre_spec::op_contract::OperationContract::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::op_contract::OperationContract where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::OperationContract::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::op_contract::OperationContract where T: ?core::marker::Sized
pub fn vyre_spec::op_contract::OperationContract::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::op_contract::OperationContract where T: core::clone::Clone
pub unsafe fn vyre_spec::op_contract::OperationContract::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::op_contract::OperationContract
pub fn vyre_spec::op_contract::OperationContract::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::op_contract::OperationContract where T: for<'de> serde_core::de::Deserialize<'de>
pub struct vyre_spec::TestDescriptor
pub vyre_spec::TestDescriptor::invariant: vyre_spec::engine_invariant::InvariantId
pub vyre_spec::TestDescriptor::name: &'static str
pub vyre_spec::TestDescriptor::purpose: &'static str
impl core::clone::Clone for vyre_spec::test_descriptor::TestDescriptor
pub fn vyre_spec::test_descriptor::TestDescriptor::clone(&self) -> vyre_spec::test_descriptor::TestDescriptor
impl core::cmp::Eq for vyre_spec::test_descriptor::TestDescriptor
impl core::cmp::PartialEq for vyre_spec::test_descriptor::TestDescriptor
pub fn vyre_spec::test_descriptor::TestDescriptor::eq(&self, other: &vyre_spec::test_descriptor::TestDescriptor) -> bool
impl core::fmt::Debug for vyre_spec::test_descriptor::TestDescriptor
pub fn vyre_spec::test_descriptor::TestDescriptor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::Freeze for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::Send for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::Sync for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::Unpin for vyre_spec::test_descriptor::TestDescriptor
impl core::marker::UnsafeUnpin for vyre_spec::test_descriptor::TestDescriptor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::test_descriptor::TestDescriptor
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::test_descriptor::TestDescriptor
impl<T, U> core::convert::Into<U> for vyre_spec::test_descriptor::TestDescriptor where U: core::convert::From<T>
pub fn vyre_spec::test_descriptor::TestDescriptor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::test_descriptor::TestDescriptor where U: core::convert::Into<T>
pub type vyre_spec::test_descriptor::TestDescriptor::Error = core::convert::Infallible
pub fn vyre_spec::test_descriptor::TestDescriptor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::test_descriptor::TestDescriptor where U: core::convert::TryFrom<T>
pub type vyre_spec::test_descriptor::TestDescriptor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::test_descriptor::TestDescriptor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::test_descriptor::TestDescriptor where T: core::clone::Clone
pub type vyre_spec::test_descriptor::TestDescriptor::Owned = T
pub fn vyre_spec::test_descriptor::TestDescriptor::clone_into(&self, target: &mut T)
pub fn vyre_spec::test_descriptor::TestDescriptor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::test_descriptor::TestDescriptor where T: 'static + ?core::marker::Sized
pub fn vyre_spec::test_descriptor::TestDescriptor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::test_descriptor::TestDescriptor where T: ?core::marker::Sized
pub fn vyre_spec::test_descriptor::TestDescriptor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::test_descriptor::TestDescriptor where T: ?core::marker::Sized
pub fn vyre_spec::test_descriptor::TestDescriptor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::test_descriptor::TestDescriptor where T: core::clone::Clone
pub unsafe fn vyre_spec::test_descriptor::TestDescriptor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::test_descriptor::TestDescriptor
pub fn vyre_spec::test_descriptor::TestDescriptor::from(t: T) -> T
pub struct vyre_spec::TypeId(pub u32)
impl vyre_spec::data_type::TypeId
pub const fn vyre_spec::data_type::TypeId::as_u32(self) -> u32
impl core::clone::Clone for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::clone(&self) -> vyre_spec::data_type::TypeId
impl core::cmp::Eq for vyre_spec::data_type::TypeId
impl core::cmp::PartialEq for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::eq(&self, other: &vyre_spec::data_type::TypeId) -> bool
impl core::fmt::Debug for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_spec::data_type::TypeId
impl core::marker::StructuralPartialEq for vyre_spec::data_type::TypeId
impl serde_core::ser::Serialize for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::serialize<__S>(&self, __serializer: __S) -> core::result::Result<<__S as serde_core::ser::Serializer>::Ok, <__S as serde_core::ser::Serializer>::Error> where __S: serde_core::ser::Serializer
impl<'de> serde_core::de::Deserialize<'de> for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::deserialize<__D>(__deserializer: __D) -> core::result::Result<Self, <__D as serde_core::de::Deserializer>::Error> where __D: serde_core::de::Deserializer<'de>
impl core::marker::Freeze for vyre_spec::data_type::TypeId
impl core::marker::Send for vyre_spec::data_type::TypeId
impl core::marker::Sync for vyre_spec::data_type::TypeId
impl core::marker::Unpin for vyre_spec::data_type::TypeId
impl core::marker::UnsafeUnpin for vyre_spec::data_type::TypeId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_spec::data_type::TypeId
impl core::panic::unwind_safe::UnwindSafe for vyre_spec::data_type::TypeId
impl<T, U> core::convert::Into<U> for vyre_spec::data_type::TypeId where U: core::convert::From<T>
pub fn vyre_spec::data_type::TypeId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_spec::data_type::TypeId where U: core::convert::Into<T>
pub type vyre_spec::data_type::TypeId::Error = core::convert::Infallible
pub fn vyre_spec::data_type::TypeId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_spec::data_type::TypeId where U: core::convert::TryFrom<T>
pub type vyre_spec::data_type::TypeId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_spec::data_type::TypeId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_spec::data_type::TypeId where T: core::clone::Clone
pub type vyre_spec::data_type::TypeId::Owned = T
pub fn vyre_spec::data_type::TypeId::clone_into(&self, target: &mut T)
pub fn vyre_spec::data_type::TypeId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_spec::data_type::TypeId where T: 'static + ?core::marker::Sized
pub fn vyre_spec::data_type::TypeId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_spec::data_type::TypeId where T: ?core::marker::Sized
pub fn vyre_spec::data_type::TypeId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_spec::data_type::TypeId where T: ?core::marker::Sized
pub fn vyre_spec::data_type::TypeId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_spec::data_type::TypeId where T: core::clone::Clone
pub unsafe fn vyre_spec::data_type::TypeId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_spec::data_type::TypeId
pub fn vyre_spec::data_type::TypeId::from(t: T) -> T
impl<T> serde_core::de::DeserializeOwned for vyre_spec::data_type::TypeId where T: for<'de> serde_core::de::Deserialize<'de>
pub trait vyre_spec::BackendAvailability: core::marker::Send + core::marker::Sync
pub fn vyre_spec::BackendAvailability::available(&self, op: &str) -> bool
impl vyre_spec::category::BackendAvailability for vyre_spec::category::BackendAvailabilityPredicate
pub fn vyre_spec::category::BackendAvailabilityPredicate::available(&self, op: &str) -> bool
impl<F> vyre_spec::category::BackendAvailability for F where F: core::ops::function::Fn(&str) -> bool + core::marker::Send + core::marker::Sync
pub fn F::available(&self, op: &str) -> bool
pub fn vyre_spec::all_algebraic_laws() -> &'static [vyre_spec::algebraic_law::AlgebraicLaw]
pub fn vyre_spec::by_category(category: vyre_spec::invariant_category::InvariantCategory) -> impl core::iter::traits::iterator::Iterator<Item = &'static vyre_spec::invariant::Invariant>
pub fn vyre_spec::by_id(id: vyre_spec::engine_invariant::InvariantId) -> core::option::Option<&'static vyre_spec::invariant::Invariant>
pub fn vyre_spec::catalog_is_complete() -> bool
pub fn vyre_spec::empty_test_family() -> &'static [vyre_spec::test_descriptor::TestDescriptor]
pub fn vyre_spec::expr_variants() -> &'static [&'static str]
pub fn vyre_spec::invariants() -> &'static [vyre_spec::invariant::Invariant]
pub fn vyre_spec::law_catalog() -> &'static [&'static str]
pub type vyre_spec::CpuFn = fn(input: &[u8], output: &mut alloc::vec::Vec<u8>)
pub type vyre_spec::InvariantId = vyre_spec::engine_invariant::EngineInvariant
pub type vyre_spec::LawCheckFn = fn(fn(&[u8]) -> alloc::vec::Vec<u8>, &[u32]) -> bool

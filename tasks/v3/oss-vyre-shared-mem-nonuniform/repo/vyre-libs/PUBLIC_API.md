pub mod vyre_libs
pub use vyre_libs::Soundness
pub use vyre_libs::SoundnessTagged
pub mod vyre_libs::borrowck
pub mod vyre_libs::borrowck::gpu
pub const vyre_libs::borrowck::gpu::DEFAULT_SHARD_WORDS: usize
pub fn vyre_libs::borrowck::gpu::analyze_batched(dispatcher: &dyn vyre_self_substrate::optimizer::dispatcher::OptimizerDispatcher, facts: &vyre_libs::borrowck::BorrowFacts) -> core::result::Result<alloc::vec::Vec<vyre_libs::borrowck::Conflict>, vyre_self_substrate::optimizer::dispatcher::DispatchError>
pub fn vyre_libs::borrowck::gpu::analyze_crate_batched(dispatcher: &dyn vyre_self_substrate::optimizer::dispatcher::OptimizerDispatcher, functions: &[vyre_libs::borrowck::BorrowFacts]) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<vyre_libs::borrowck::Conflict>>, vyre_self_substrate::optimizer::dispatcher::DispatchError>
pub fn vyre_libs::borrowck::gpu::analyze_crate_batched_with_shard_cap(dispatcher: &dyn vyre_self_substrate::optimizer::dispatcher::OptimizerDispatcher, functions: &[vyre_libs::borrowck::BorrowFacts], max_shard_words: usize) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<vyre_libs::borrowck::Conflict>>, vyre_self_substrate::optimizer::dispatcher::DispatchError>
pub mod vyre_libs::borrowck::rustc_facts
pub struct vyre_libs::borrowck::rustc_facts::NllError
pub vyre_libs::borrowck::rustc_facts::NllError::loan: vyre_libs::borrowck::rustc_facts::Loan
pub vyre_libs::borrowck::rustc_facts::NllError::point: vyre_libs::borrowck::rustc_facts::Point
impl core::clone::Clone for vyre_libs::borrowck::rustc_facts::NllError
pub fn vyre_libs::borrowck::rustc_facts::NllError::clone(&self) -> vyre_libs::borrowck::rustc_facts::NllError
impl core::cmp::Eq for vyre_libs::borrowck::rustc_facts::NllError
impl core::cmp::PartialEq for vyre_libs::borrowck::rustc_facts::NllError
pub fn vyre_libs::borrowck::rustc_facts::NllError::eq(&self, other: &vyre_libs::borrowck::rustc_facts::NllError) -> bool
impl core::fmt::Debug for vyre_libs::borrowck::rustc_facts::NllError
pub fn vyre_libs::borrowck::rustc_facts::NllError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_libs::borrowck::rustc_facts::NllError
pub fn vyre_libs::borrowck::rustc_facts::NllError::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_libs::borrowck::rustc_facts::NllError
impl core::marker::StructuralPartialEq for vyre_libs::borrowck::rustc_facts::NllError
impl core::marker::Freeze for vyre_libs::borrowck::rustc_facts::NllError
impl core::marker::Send for vyre_libs::borrowck::rustc_facts::NllError
impl core::marker::Sync for vyre_libs::borrowck::rustc_facts::NllError
impl core::marker::Unpin for vyre_libs::borrowck::rustc_facts::NllError
impl core::marker::UnsafeUnpin for vyre_libs::borrowck::rustc_facts::NllError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::borrowck::rustc_facts::NllError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::borrowck::rustc_facts::NllError
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::borrowck::rustc_facts::NllError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::borrowck::rustc_facts::NllError::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::borrowck::rustc_facts::NllError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::borrowck::rustc_facts::NllError::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::borrowck::rustc_facts::NllError where U: core::convert::From<T>
pub fn vyre_libs::borrowck::rustc_facts::NllError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::borrowck::rustc_facts::NllError where U: core::convert::Into<T>
pub type vyre_libs::borrowck::rustc_facts::NllError::Error = core::convert::Infallible
pub fn vyre_libs::borrowck::rustc_facts::NllError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::borrowck::rustc_facts::NllError where U: core::convert::TryFrom<T>
pub type vyre_libs::borrowck::rustc_facts::NllError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::borrowck::rustc_facts::NllError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::borrowck::rustc_facts::NllError where T: core::clone::Clone
pub type vyre_libs::borrowck::rustc_facts::NllError::Owned = T
pub fn vyre_libs::borrowck::rustc_facts::NllError::clone_into(&self, target: &mut T)
pub fn vyre_libs::borrowck::rustc_facts::NllError::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::borrowck::rustc_facts::NllError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::borrowck::rustc_facts::NllError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::borrowck::rustc_facts::NllError where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::rustc_facts::NllError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::borrowck::rustc_facts::NllError where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::rustc_facts::NllError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::borrowck::rustc_facts::NllError where T: core::clone::Clone
pub unsafe fn vyre_libs::borrowck::rustc_facts::NllError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::borrowck::rustc_facts::NllError
pub fn vyre_libs::borrowck::rustc_facts::NllError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::borrowck::rustc_facts::NllError
pub type vyre_libs::borrowck::rustc_facts::NllError::Init = T
pub const vyre_libs::borrowck::rustc_facts::NllError::ALIGN: usize
pub unsafe fn vyre_libs::borrowck::rustc_facts::NllError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::borrowck::rustc_facts::NllError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::borrowck::rustc_facts::NllError::drop(ptr: usize)
pub unsafe fn vyre_libs::borrowck::rustc_facts::NllError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::borrowck::rustc_facts::NllError
impl<T> tracing::instrument::Instrument for vyre_libs::borrowck::rustc_facts::NllError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::borrowck::rustc_facts::NllError
impl<T> typenum::type_operators::Same for vyre_libs::borrowck::rustc_facts::NllError
pub type vyre_libs::borrowck::rustc_facts::NllError::Output = T
pub struct vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::cfg_edge: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Point, vyre_libs::borrowck::rustc_facts::Point)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::drop_of_var_derefs_origin: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Var, vyre_libs::borrowck::rustc_facts::Origin)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::known_placeholder_subset: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Origin, vyre_libs::borrowck::rustc_facts::Origin)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::loan_count: u32
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::loan_invalidated_at: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Point, vyre_libs::borrowck::rustc_facts::Loan)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::loan_issued_at: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Origin, vyre_libs::borrowck::rustc_facts::Loan, vyre_libs::borrowck::rustc_facts::Point)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::loan_killed_at: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Loan, vyre_libs::borrowck::rustc_facts::Point)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::loan_names: alloc::vec::Vec<alloc::string::String>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::origin_count: u32
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::placeholder: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Origin, vyre_libs::borrowck::rustc_facts::Loan)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::point_count: u32
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::point_names: alloc::vec::Vec<alloc::string::String>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::subset_base: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Origin, vyre_libs::borrowck::rustc_facts::Origin, vyre_libs::borrowck::rustc_facts::Point)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::universal_region: alloc::vec::Vec<vyre_libs::borrowck::rustc_facts::Origin>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::use_of_var_derefs_origin: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Var, vyre_libs::borrowck::rustc_facts::Origin)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::var_count: u32
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::var_defined_at: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Var, vyre_libs::borrowck::rustc_facts::Point)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::var_dropped_at: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Var, vyre_libs::borrowck::rustc_facts::Point)>
pub vyre_libs::borrowck::rustc_facts::RustcNllFacts::var_used_at: alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Var, vyre_libs::borrowck::rustc_facts::Point)>
impl vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::accepts(&self) -> bool
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::nll_errors(&self) -> alloc::vec::Vec<vyre_libs::borrowck::rustc_facts::NllError>
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::subset_errors(&self) -> alloc::vec::Vec<(vyre_libs::borrowck::rustc_facts::Origin, vyre_libs::borrowck::rustc_facts::Origin)>
impl core::clone::Clone for vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::clone(&self) -> vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl core::default::Default for vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::default() -> vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl core::fmt::Debug for vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl core::marker::Send for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl core::marker::Sync for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl core::marker::Unpin for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl core::marker::UnsafeUnpin for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl<T, U> core::convert::Into<U> for vyre_libs::borrowck::rustc_facts::RustcNllFacts where U: core::convert::From<T>
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::borrowck::rustc_facts::RustcNllFacts where U: core::convert::Into<T>
pub type vyre_libs::borrowck::rustc_facts::RustcNllFacts::Error = core::convert::Infallible
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::borrowck::rustc_facts::RustcNllFacts where U: core::convert::TryFrom<T>
pub type vyre_libs::borrowck::rustc_facts::RustcNllFacts::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::borrowck::rustc_facts::RustcNllFacts where T: core::clone::Clone
pub type vyre_libs::borrowck::rustc_facts::RustcNllFacts::Owned = T
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::clone_into(&self, target: &mut T)
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::borrowck::rustc_facts::RustcNllFacts where T: 'static + ?core::marker::Sized
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::borrowck::rustc_facts::RustcNllFacts where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::borrowck::rustc_facts::RustcNllFacts where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::borrowck::rustc_facts::RustcNllFacts where T: core::clone::Clone
pub unsafe fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub type vyre_libs::borrowck::rustc_facts::RustcNllFacts::Init = T
pub const vyre_libs::borrowck::rustc_facts::RustcNllFacts::ALIGN: usize
pub unsafe fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::drop(ptr: usize)
pub unsafe fn vyre_libs::borrowck::rustc_facts::RustcNllFacts::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl<T> tracing::instrument::Instrument for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl<T> tracing::instrument::WithSubscriber for vyre_libs::borrowck::rustc_facts::RustcNllFacts
impl<T> typenum::type_operators::Same for vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub type vyre_libs::borrowck::rustc_facts::RustcNllFacts::Output = T
pub fn vyre_libs::borrowck::rustc_facts::load_facts(read: impl core::ops::function::Fn(&str) -> alloc::string::String) -> vyre_libs::borrowck::rustc_facts::RustcNllFacts
pub type vyre_libs::borrowck::rustc_facts::Loan = u32
pub type vyre_libs::borrowck::rustc_facts::Origin = u32
pub type vyre_libs::borrowck::rustc_facts::Point = u32
pub type vyre_libs::borrowck::rustc_facts::Var = u32
pub enum vyre_libs::borrowck::ConflictKind
pub vyre_libs::borrowck::ConflictKind::MutableAndShared
pub vyre_libs::borrowck::ConflictKind::TwoMutable
impl core::clone::Clone for vyre_libs::borrowck::ConflictKind
pub fn vyre_libs::borrowck::ConflictKind::clone(&self) -> vyre_libs::borrowck::ConflictKind
impl core::cmp::Eq for vyre_libs::borrowck::ConflictKind
impl core::cmp::PartialEq for vyre_libs::borrowck::ConflictKind
pub fn vyre_libs::borrowck::ConflictKind::eq(&self, other: &vyre_libs::borrowck::ConflictKind) -> bool
impl core::fmt::Debug for vyre_libs::borrowck::ConflictKind
pub fn vyre_libs::borrowck::ConflictKind::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::borrowck::ConflictKind
impl core::marker::StructuralPartialEq for vyre_libs::borrowck::ConflictKind
impl core::marker::Freeze for vyre_libs::borrowck::ConflictKind
impl core::marker::Send for vyre_libs::borrowck::ConflictKind
impl core::marker::Sync for vyre_libs::borrowck::ConflictKind
impl core::marker::Unpin for vyre_libs::borrowck::ConflictKind
impl core::marker::UnsafeUnpin for vyre_libs::borrowck::ConflictKind
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::borrowck::ConflictKind
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::borrowck::ConflictKind
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::borrowck::ConflictKind where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::borrowck::ConflictKind::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::borrowck::ConflictKind where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::borrowck::ConflictKind::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::borrowck::ConflictKind where U: core::convert::From<T>
pub fn vyre_libs::borrowck::ConflictKind::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::borrowck::ConflictKind where U: core::convert::Into<T>
pub type vyre_libs::borrowck::ConflictKind::Error = core::convert::Infallible
pub fn vyre_libs::borrowck::ConflictKind::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::borrowck::ConflictKind where U: core::convert::TryFrom<T>
pub type vyre_libs::borrowck::ConflictKind::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::borrowck::ConflictKind::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::borrowck::ConflictKind where T: core::clone::Clone
pub type vyre_libs::borrowck::ConflictKind::Owned = T
pub fn vyre_libs::borrowck::ConflictKind::clone_into(&self, target: &mut T)
pub fn vyre_libs::borrowck::ConflictKind::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::borrowck::ConflictKind where T: 'static + ?core::marker::Sized
pub fn vyre_libs::borrowck::ConflictKind::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::borrowck::ConflictKind where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::ConflictKind::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::borrowck::ConflictKind where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::ConflictKind::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::borrowck::ConflictKind where T: core::clone::Clone
pub unsafe fn vyre_libs::borrowck::ConflictKind::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::borrowck::ConflictKind
pub fn vyre_libs::borrowck::ConflictKind::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::borrowck::ConflictKind
pub type vyre_libs::borrowck::ConflictKind::Init = T
pub const vyre_libs::borrowck::ConflictKind::ALIGN: usize
pub unsafe fn vyre_libs::borrowck::ConflictKind::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::borrowck::ConflictKind::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::borrowck::ConflictKind::drop(ptr: usize)
pub unsafe fn vyre_libs::borrowck::ConflictKind::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::borrowck::ConflictKind
impl<T> tracing::instrument::Instrument for vyre_libs::borrowck::ConflictKind
impl<T> tracing::instrument::WithSubscriber for vyre_libs::borrowck::ConflictKind
impl<T> typenum::type_operators::Same for vyre_libs::borrowck::ConflictKind
pub type vyre_libs::borrowck::ConflictKind::Output = T
pub enum vyre_libs::borrowck::LoanKind
pub vyre_libs::borrowck::LoanKind::Mut
pub vyre_libs::borrowck::LoanKind::Shared
impl core::clone::Clone for vyre_libs::borrowck::LoanKind
pub fn vyre_libs::borrowck::LoanKind::clone(&self) -> vyre_libs::borrowck::LoanKind
impl core::cmp::Eq for vyre_libs::borrowck::LoanKind
impl core::cmp::PartialEq for vyre_libs::borrowck::LoanKind
pub fn vyre_libs::borrowck::LoanKind::eq(&self, other: &vyre_libs::borrowck::LoanKind) -> bool
impl core::fmt::Debug for vyre_libs::borrowck::LoanKind
pub fn vyre_libs::borrowck::LoanKind::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::borrowck::LoanKind
impl core::marker::StructuralPartialEq for vyre_libs::borrowck::LoanKind
impl core::marker::Freeze for vyre_libs::borrowck::LoanKind
impl core::marker::Send for vyre_libs::borrowck::LoanKind
impl core::marker::Sync for vyre_libs::borrowck::LoanKind
impl core::marker::Unpin for vyre_libs::borrowck::LoanKind
impl core::marker::UnsafeUnpin for vyre_libs::borrowck::LoanKind
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::borrowck::LoanKind
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::borrowck::LoanKind
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::borrowck::LoanKind where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::borrowck::LoanKind::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::borrowck::LoanKind where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::borrowck::LoanKind::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::borrowck::LoanKind where U: core::convert::From<T>
pub fn vyre_libs::borrowck::LoanKind::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::borrowck::LoanKind where U: core::convert::Into<T>
pub type vyre_libs::borrowck::LoanKind::Error = core::convert::Infallible
pub fn vyre_libs::borrowck::LoanKind::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::borrowck::LoanKind where U: core::convert::TryFrom<T>
pub type vyre_libs::borrowck::LoanKind::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::borrowck::LoanKind::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::borrowck::LoanKind where T: core::clone::Clone
pub type vyre_libs::borrowck::LoanKind::Owned = T
pub fn vyre_libs::borrowck::LoanKind::clone_into(&self, target: &mut T)
pub fn vyre_libs::borrowck::LoanKind::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::borrowck::LoanKind where T: 'static + ?core::marker::Sized
pub fn vyre_libs::borrowck::LoanKind::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::borrowck::LoanKind where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::LoanKind::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::borrowck::LoanKind where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::LoanKind::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::borrowck::LoanKind where T: core::clone::Clone
pub unsafe fn vyre_libs::borrowck::LoanKind::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::borrowck::LoanKind
pub fn vyre_libs::borrowck::LoanKind::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::borrowck::LoanKind
pub type vyre_libs::borrowck::LoanKind::Init = T
pub const vyre_libs::borrowck::LoanKind::ALIGN: usize
pub unsafe fn vyre_libs::borrowck::LoanKind::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::borrowck::LoanKind::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::borrowck::LoanKind::drop(ptr: usize)
pub unsafe fn vyre_libs::borrowck::LoanKind::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::borrowck::LoanKind
impl<T> tracing::instrument::Instrument for vyre_libs::borrowck::LoanKind
impl<T> tracing::instrument::WithSubscriber for vyre_libs::borrowck::LoanKind
impl<T> typenum::type_operators::Same for vyre_libs::borrowck::LoanKind
pub type vyre_libs::borrowck::LoanKind::Output = T
pub struct vyre_libs::borrowck::BorrowFacts
pub vyre_libs::borrowck::BorrowFacts::cfg_edges: alloc::vec::Vec<(vyre_libs::borrowck::Point, vyre_libs::borrowck::Point)>
pub vyre_libs::borrowck::BorrowFacts::loan_issued_at: alloc::vec::Vec<vyre_libs::borrowck::Point>
pub vyre_libs::borrowck::BorrowFacts::loan_kind: alloc::vec::Vec<vyre_libs::borrowck::LoanKind>
pub vyre_libs::borrowck::BorrowFacts::loan_offset: alloc::vec::Vec<u32>
pub vyre_libs::borrowck::BorrowFacts::loan_place: alloc::vec::Vec<vyre_libs::borrowck::Place>
pub vyre_libs::borrowck::BorrowFacts::loan_used_at: alloc::vec::Vec<(vyre_libs::borrowck::Loan, vyre_libs::borrowck::Point)>
pub vyre_libs::borrowck::BorrowFacts::point_count: u32
impl vyre_libs::borrowck::BorrowFacts
pub fn vyre_libs::borrowck::BorrowFacts::loan_count(&self) -> usize
impl core::clone::Clone for vyre_libs::borrowck::BorrowFacts
pub fn vyre_libs::borrowck::BorrowFacts::clone(&self) -> vyre_libs::borrowck::BorrowFacts
impl core::default::Default for vyre_libs::borrowck::BorrowFacts
pub fn vyre_libs::borrowck::BorrowFacts::default() -> vyre_libs::borrowck::BorrowFacts
impl core::fmt::Debug for vyre_libs::borrowck::BorrowFacts
pub fn vyre_libs::borrowck::BorrowFacts::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::borrowck::BorrowFacts
impl core::marker::Send for vyre_libs::borrowck::BorrowFacts
impl core::marker::Sync for vyre_libs::borrowck::BorrowFacts
impl core::marker::Unpin for vyre_libs::borrowck::BorrowFacts
impl core::marker::UnsafeUnpin for vyre_libs::borrowck::BorrowFacts
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::borrowck::BorrowFacts
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::borrowck::BorrowFacts
impl<T, U> core::convert::Into<U> for vyre_libs::borrowck::BorrowFacts where U: core::convert::From<T>
pub fn vyre_libs::borrowck::BorrowFacts::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::borrowck::BorrowFacts where U: core::convert::Into<T>
pub type vyre_libs::borrowck::BorrowFacts::Error = core::convert::Infallible
pub fn vyre_libs::borrowck::BorrowFacts::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::borrowck::BorrowFacts where U: core::convert::TryFrom<T>
pub type vyre_libs::borrowck::BorrowFacts::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::borrowck::BorrowFacts::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::borrowck::BorrowFacts where T: core::clone::Clone
pub type vyre_libs::borrowck::BorrowFacts::Owned = T
pub fn vyre_libs::borrowck::BorrowFacts::clone_into(&self, target: &mut T)
pub fn vyre_libs::borrowck::BorrowFacts::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::borrowck::BorrowFacts where T: 'static + ?core::marker::Sized
pub fn vyre_libs::borrowck::BorrowFacts::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::borrowck::BorrowFacts where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::BorrowFacts::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::borrowck::BorrowFacts where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::BorrowFacts::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::borrowck::BorrowFacts where T: core::clone::Clone
pub unsafe fn vyre_libs::borrowck::BorrowFacts::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::borrowck::BorrowFacts
pub fn vyre_libs::borrowck::BorrowFacts::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::borrowck::BorrowFacts
pub type vyre_libs::borrowck::BorrowFacts::Init = T
pub const vyre_libs::borrowck::BorrowFacts::ALIGN: usize
pub unsafe fn vyre_libs::borrowck::BorrowFacts::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::borrowck::BorrowFacts::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::borrowck::BorrowFacts::drop(ptr: usize)
pub unsafe fn vyre_libs::borrowck::BorrowFacts::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::borrowck::BorrowFacts
impl<T> tracing::instrument::Instrument for vyre_libs::borrowck::BorrowFacts
impl<T> tracing::instrument::WithSubscriber for vyre_libs::borrowck::BorrowFacts
impl<T> typenum::type_operators::Same for vyre_libs::borrowck::BorrowFacts
pub type vyre_libs::borrowck::BorrowFacts::Output = T
pub struct vyre_libs::borrowck::Conflict
pub vyre_libs::borrowck::Conflict::first: vyre_libs::borrowck::Loan
pub vyre_libs::borrowck::Conflict::kind: vyre_libs::borrowck::ConflictKind
pub vyre_libs::borrowck::Conflict::offset: u32
pub vyre_libs::borrowck::Conflict::second: vyre_libs::borrowck::Loan
impl core::clone::Clone for vyre_libs::borrowck::Conflict
pub fn vyre_libs::borrowck::Conflict::clone(&self) -> vyre_libs::borrowck::Conflict
impl core::cmp::Eq for vyre_libs::borrowck::Conflict
impl core::cmp::PartialEq for vyre_libs::borrowck::Conflict
pub fn vyre_libs::borrowck::Conflict::eq(&self, other: &vyre_libs::borrowck::Conflict) -> bool
impl core::fmt::Debug for vyre_libs::borrowck::Conflict
pub fn vyre_libs::borrowck::Conflict::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::borrowck::Conflict
impl core::marker::StructuralPartialEq for vyre_libs::borrowck::Conflict
impl core::marker::Freeze for vyre_libs::borrowck::Conflict
impl core::marker::Send for vyre_libs::borrowck::Conflict
impl core::marker::Sync for vyre_libs::borrowck::Conflict
impl core::marker::Unpin for vyre_libs::borrowck::Conflict
impl core::marker::UnsafeUnpin for vyre_libs::borrowck::Conflict
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::borrowck::Conflict
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::borrowck::Conflict
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::borrowck::Conflict where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::borrowck::Conflict::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::borrowck::Conflict where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::borrowck::Conflict::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::borrowck::Conflict where U: core::convert::From<T>
pub fn vyre_libs::borrowck::Conflict::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::borrowck::Conflict where U: core::convert::Into<T>
pub type vyre_libs::borrowck::Conflict::Error = core::convert::Infallible
pub fn vyre_libs::borrowck::Conflict::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::borrowck::Conflict where U: core::convert::TryFrom<T>
pub type vyre_libs::borrowck::Conflict::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::borrowck::Conflict::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::borrowck::Conflict where T: core::clone::Clone
pub type vyre_libs::borrowck::Conflict::Owned = T
pub fn vyre_libs::borrowck::Conflict::clone_into(&self, target: &mut T)
pub fn vyre_libs::borrowck::Conflict::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::borrowck::Conflict where T: 'static + ?core::marker::Sized
pub fn vyre_libs::borrowck::Conflict::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::borrowck::Conflict where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::Conflict::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::borrowck::Conflict where T: ?core::marker::Sized
pub fn vyre_libs::borrowck::Conflict::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::borrowck::Conflict where T: core::clone::Clone
pub unsafe fn vyre_libs::borrowck::Conflict::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::borrowck::Conflict
pub fn vyre_libs::borrowck::Conflict::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::borrowck::Conflict
pub type vyre_libs::borrowck::Conflict::Init = T
pub const vyre_libs::borrowck::Conflict::ALIGN: usize
pub unsafe fn vyre_libs::borrowck::Conflict::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::borrowck::Conflict::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::borrowck::Conflict::drop(ptr: usize)
pub unsafe fn vyre_libs::borrowck::Conflict::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::borrowck::Conflict
impl<T> tracing::instrument::Instrument for vyre_libs::borrowck::Conflict
impl<T> tracing::instrument::WithSubscriber for vyre_libs::borrowck::Conflict
impl<T> typenum::type_operators::Same for vyre_libs::borrowck::Conflict
pub type vyre_libs::borrowck::Conflict::Output = T
pub fn vyre_libs::borrowck::analyze(facts: &vyre_libs::borrowck::BorrowFacts) -> alloc::vec::Vec<vyre_libs::borrowck::Conflict>
pub type vyre_libs::borrowck::Loan = u32
pub type vyre_libs::borrowck::Place = u32
pub type vyre_libs::borrowck::Point = u32
pub mod vyre_libs::buffer_names
pub fn vyre_libs::buffer_names::scoped_generic_name(family_prefix: &str, role: &str, requested: &str, generic_aliases: &[&str]) -> alloc::string::String
pub mod vyre_libs::builder
#[non_exhaustive] pub struct vyre_libs::builder::BuildOptions
pub vyre_libs::builder::BuildOptions::region_generator: core::option::Option<&'static str>
pub vyre_libs::builder::BuildOptions::tenant_id: core::option::Option<u32>
pub vyre_libs::builder::BuildOptions::workgroup_size: core::option::Option<[u32; 3]>
impl vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::new() -> Self
pub fn vyre_libs::builder::BuildOptions::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::builder::BuildOptions::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::builder::BuildOptions::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::clone(&self) -> vyre_libs::builder::BuildOptions
impl core::default::Default for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::default() -> vyre_libs::builder::BuildOptions
impl core::fmt::Debug for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::builder::BuildOptions
impl core::marker::Send for vyre_libs::builder::BuildOptions
impl core::marker::Sync for vyre_libs::builder::BuildOptions
impl core::marker::Unpin for vyre_libs::builder::BuildOptions
impl core::marker::UnsafeUnpin for vyre_libs::builder::BuildOptions
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::builder::BuildOptions
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::builder::BuildOptions
impl<T, U> core::convert::Into<U> for vyre_libs::builder::BuildOptions where U: core::convert::From<T>
pub fn vyre_libs::builder::BuildOptions::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::builder::BuildOptions where U: core::convert::Into<T>
pub type vyre_libs::builder::BuildOptions::Error = core::convert::Infallible
pub fn vyre_libs::builder::BuildOptions::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::builder::BuildOptions where U: core::convert::TryFrom<T>
pub type vyre_libs::builder::BuildOptions::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::builder::BuildOptions::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::builder::BuildOptions where T: core::clone::Clone
pub type vyre_libs::builder::BuildOptions::Owned = T
pub fn vyre_libs::builder::BuildOptions::clone_into(&self, target: &mut T)
pub fn vyre_libs::builder::BuildOptions::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::builder::BuildOptions where T: 'static + ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::builder::BuildOptions where T: ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::builder::BuildOptions where T: ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::builder::BuildOptions where T: core::clone::Clone
pub unsafe fn vyre_libs::builder::BuildOptions::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::builder::BuildOptions
pub type vyre_libs::builder::BuildOptions::Init = T
pub const vyre_libs::builder::BuildOptions::ALIGN: usize
pub unsafe fn vyre_libs::builder::BuildOptions::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::builder::BuildOptions::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::builder::BuildOptions::drop(ptr: usize)
pub unsafe fn vyre_libs::builder::BuildOptions::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::builder::BuildOptions
impl<T> tracing::instrument::Instrument for vyre_libs::builder::BuildOptions
impl<T> tracing::instrument::WithSubscriber for vyre_libs::builder::BuildOptions
impl<T> typenum::type_operators::Same for vyre_libs::builder::BuildOptions
pub type vyre_libs::builder::BuildOptions::Output = T
pub fn vyre_libs::builder::check_tensors(op: &'static str, tensors: &[(&vyre_libs::tensor_ref::TensorRef, vyre_spec::data_type::DataType)]) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub mod vyre_libs::compat_aliases
pub struct vyre_libs::compat_aliases::CompatibilityAlias
pub vyre_libs::compat_aliases::CompatibilityAlias::canonical_owner: &'static str
pub vyre_libs::compat_aliases::CompatibilityAlias::canonical_path: &'static str
pub vyre_libs::compat_aliases::CompatibilityAlias::deprecated_path: &'static str
pub vyre_libs::compat_aliases::CompatibilityAlias::removal_condition: &'static str
impl core::clone::Clone for vyre_libs::compat_aliases::CompatibilityAlias
pub fn vyre_libs::compat_aliases::CompatibilityAlias::clone(&self) -> vyre_libs::compat_aliases::CompatibilityAlias
impl core::cmp::Eq for vyre_libs::compat_aliases::CompatibilityAlias
impl core::cmp::PartialEq for vyre_libs::compat_aliases::CompatibilityAlias
pub fn vyre_libs::compat_aliases::CompatibilityAlias::eq(&self, other: &vyre_libs::compat_aliases::CompatibilityAlias) -> bool
impl core::fmt::Debug for vyre_libs::compat_aliases::CompatibilityAlias
pub fn vyre_libs::compat_aliases::CompatibilityAlias::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::compat_aliases::CompatibilityAlias
impl core::marker::StructuralPartialEq for vyre_libs::compat_aliases::CompatibilityAlias
impl core::marker::Freeze for vyre_libs::compat_aliases::CompatibilityAlias
impl core::marker::Send for vyre_libs::compat_aliases::CompatibilityAlias
impl core::marker::Sync for vyre_libs::compat_aliases::CompatibilityAlias
impl core::marker::Unpin for vyre_libs::compat_aliases::CompatibilityAlias
impl core::marker::UnsafeUnpin for vyre_libs::compat_aliases::CompatibilityAlias
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::compat_aliases::CompatibilityAlias
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::compat_aliases::CompatibilityAlias
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::compat_aliases::CompatibilityAlias where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::compat_aliases::CompatibilityAlias::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::compat_aliases::CompatibilityAlias where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::compat_aliases::CompatibilityAlias::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::compat_aliases::CompatibilityAlias where U: core::convert::From<T>
pub fn vyre_libs::compat_aliases::CompatibilityAlias::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::compat_aliases::CompatibilityAlias where U: core::convert::Into<T>
pub type vyre_libs::compat_aliases::CompatibilityAlias::Error = core::convert::Infallible
pub fn vyre_libs::compat_aliases::CompatibilityAlias::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::compat_aliases::CompatibilityAlias where U: core::convert::TryFrom<T>
pub type vyre_libs::compat_aliases::CompatibilityAlias::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::compat_aliases::CompatibilityAlias::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::compat_aliases::CompatibilityAlias where T: core::clone::Clone
pub type vyre_libs::compat_aliases::CompatibilityAlias::Owned = T
pub fn vyre_libs::compat_aliases::CompatibilityAlias::clone_into(&self, target: &mut T)
pub fn vyre_libs::compat_aliases::CompatibilityAlias::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::compat_aliases::CompatibilityAlias where T: 'static + ?core::marker::Sized
pub fn vyre_libs::compat_aliases::CompatibilityAlias::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::compat_aliases::CompatibilityAlias where T: ?core::marker::Sized
pub fn vyre_libs::compat_aliases::CompatibilityAlias::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::compat_aliases::CompatibilityAlias where T: ?core::marker::Sized
pub fn vyre_libs::compat_aliases::CompatibilityAlias::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::compat_aliases::CompatibilityAlias where T: core::clone::Clone
pub unsafe fn vyre_libs::compat_aliases::CompatibilityAlias::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::compat_aliases::CompatibilityAlias
pub fn vyre_libs::compat_aliases::CompatibilityAlias::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::compat_aliases::CompatibilityAlias
pub type vyre_libs::compat_aliases::CompatibilityAlias::Init = T
pub const vyre_libs::compat_aliases::CompatibilityAlias::ALIGN: usize
pub unsafe fn vyre_libs::compat_aliases::CompatibilityAlias::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::compat_aliases::CompatibilityAlias::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::compat_aliases::CompatibilityAlias::drop(ptr: usize)
pub unsafe fn vyre_libs::compat_aliases::CompatibilityAlias::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::compat_aliases::CompatibilityAlias
impl<T> tracing::instrument::Instrument for vyre_libs::compat_aliases::CompatibilityAlias
impl<T> tracing::instrument::WithSubscriber for vyre_libs::compat_aliases::CompatibilityAlias
impl<T> typenum::type_operators::Same for vyre_libs::compat_aliases::CompatibilityAlias
pub type vyre_libs::compat_aliases::CompatibilityAlias::Output = T
pub const vyre_libs::compat_aliases::COMPATIBILITY_ALIASES: &[vyre_libs::compat_aliases::CompatibilityAlias]
pub const vyre_libs::compat_aliases::MATCHING_ALIAS: vyre_libs::compat_aliases::CompatibilityAlias
pub const vyre_libs::compat_aliases::MATCHING_SUBSTRING_ALIAS: vyre_libs::compat_aliases::CompatibilityAlias
pub mod vyre_libs::contracts
pub const vyre_libs::contracts::PURE_DETERMINISTIC_CHEAP: vyre_spec::op_contract::OperationContract
pub const vyre_libs::contracts::RULE_PREDICATE_CHEAP: vyre_spec::op_contract::OperationContract
pub mod vyre_libs::dataflow
pub use vyre_libs::dataflow::PrecisionContract
pub use vyre_libs::dataflow::PrimitiveSoundness
pub use vyre_libs::dataflow::Soundness
pub use vyre_libs::dataflow::SoundnessTagged
pub use vyre_libs::dataflow::SoundnessViolation
pub use vyre_libs::dataflow::validate_pipeline
pub use vyre_libs::dataflow::validate_primitive
pub mod vyre_libs::decode
pub use vyre_libs::decode::hex_decode_table
pub use vyre_libs::decode::ziftsieve_reference_extract_literals
pub mod vyre_libs::decode::base64
pub const vyre_libs::decode::base64::BASE64_DECODE_TABLE_BUFFER: &str
pub fn vyre_libs::decode::base64::base64_decode(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::base64::base64_decode_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::decode::encodex
pub use vyre_libs::decode::encodex::ENC_ASCII
pub use vyre_libs::decode::encodex::ENC_BINARY
pub use vyre_libs::decode::encodex::ENC_ISO8859_1
pub use vyre_libs::decode::encodex::ENC_UTF16BE
pub use vyre_libs::decode::encodex::ENC_UTF16LE
pub use vyre_libs::decode::encodex::ENC_UTF8
pub use vyre_libs::decode::encodex::classify_from_histogram
pub use vyre_libs::decode::encodex::encoding_classify_child
pub fn vyre_libs::decode::encodex::encodex_gpu(input: &str, output: &str, count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::encodex::encodex_reference(input: &[u8]) -> u32
pub mod vyre_libs::decode::hex
pub use vyre_libs::decode::hex::hex_decode_table
pub use vyre_libs::decode::hex::hex_decode_table_ref
pub const vyre_libs::decode::hex::HEX_DECODE_TABLE_BUFFER: &str
pub fn vyre_libs::decode::hex::hex_decode(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::hex::hex_decode_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::decode::inflate
pub fn vyre_libs::decode::inflate::inflate(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate::inflate_stored_block(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate::inflate_stored_block_buffered_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate::inflate_stored_block_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate::inflate_stored_block_tiled_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32, tile_width: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate::inflate_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::decode::streaming
pub enum vyre_libs::decode::streaming::DecodeScanFuseError
pub vyre_libs::decode::streaming::DecodeScanFuseError::Fusion(vyre_foundation::execution_plan::fusion::FusionError)
pub vyre_libs::decode::streaming::DecodeScanFuseError::HandoffBufferMissing
pub vyre_libs::decode::streaming::DecodeScanFuseError::HandoffBufferMissing::handoff: alloc::string::String
pub vyre_libs::decode::streaming::DecodeScanFuseError::ZeroHandoff
pub vyre_libs::decode::streaming::DecodeScanFuseError::ZeroHandoff::handoff: alloc::string::String
impl core::convert::From<vyre_foundation::execution_plan::fusion::FusionError> for vyre_libs::decode::streaming::DecodeScanFuseError
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::from(source: vyre_foundation::execution_plan::fusion::FusionError) -> Self
impl core::error::Error for vyre_libs::decode::streaming::DecodeScanFuseError
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::source(&self) -> core::option::Option<&(dyn core::error::Error + 'static)>
impl core::fmt::Debug for vyre_libs::decode::streaming::DecodeScanFuseError
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::decode::streaming::DecodeScanFuseError
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::fmt(&self, __formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::decode::streaming::DecodeScanFuseError
impl core::marker::Send for vyre_libs::decode::streaming::DecodeScanFuseError
impl core::marker::Sync for vyre_libs::decode::streaming::DecodeScanFuseError
impl core::marker::Unpin for vyre_libs::decode::streaming::DecodeScanFuseError
impl core::marker::UnsafeUnpin for vyre_libs::decode::streaming::DecodeScanFuseError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::decode::streaming::DecodeScanFuseError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::decode::streaming::DecodeScanFuseError
impl<T, U> core::convert::Into<U> for vyre_libs::decode::streaming::DecodeScanFuseError where U: core::convert::From<T>
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::decode::streaming::DecodeScanFuseError where U: core::convert::Into<T>
pub type vyre_libs::decode::streaming::DecodeScanFuseError::Error = core::convert::Infallible
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::decode::streaming::DecodeScanFuseError where U: core::convert::TryFrom<T>
pub type vyre_libs::decode::streaming::DecodeScanFuseError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::string::ToString for vyre_libs::decode::streaming::DecodeScanFuseError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::decode::streaming::DecodeScanFuseError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::decode::streaming::DecodeScanFuseError where T: ?core::marker::Sized
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::decode::streaming::DecodeScanFuseError where T: ?core::marker::Sized
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::decode::streaming::DecodeScanFuseError
pub fn vyre_libs::decode::streaming::DecodeScanFuseError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::decode::streaming::DecodeScanFuseError
pub type vyre_libs::decode::streaming::DecodeScanFuseError::Init = T
pub const vyre_libs::decode::streaming::DecodeScanFuseError::ALIGN: usize
pub unsafe fn vyre_libs::decode::streaming::DecodeScanFuseError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::decode::streaming::DecodeScanFuseError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::decode::streaming::DecodeScanFuseError::drop(ptr: usize)
pub unsafe fn vyre_libs::decode::streaming::DecodeScanFuseError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::decode::streaming::DecodeScanFuseError
impl<T> tracing::instrument::Instrument for vyre_libs::decode::streaming::DecodeScanFuseError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::decode::streaming::DecodeScanFuseError
impl<T> typenum::type_operators::Same for vyre_libs::decode::streaming::DecodeScanFuseError
pub type vyre_libs::decode::streaming::DecodeScanFuseError::Output = T
pub fn vyre_libs::decode::streaming::dram_bytes_saved(handoff_byte_count: u32, invocations: u32) -> u64
pub fn vyre_libs::decode::streaming::fuse_decode_scan(decoder: vyre_foundation::ir_inner::model::program::core::Program, scanner: vyre_foundation::ir_inner::model::program::core::Program, handoff_buf: &str, handoff_byte_count: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::decode::streaming::DecodeScanFuseError>
pub mod vyre_libs::decode::ziftsieve
pub use vyre_libs::decode::ziftsieve::ziftsieve_reference_extract_literals
pub const vyre_libs::decode::ziftsieve::NOTE_ZIFTSIEVE_GPU_DESIGN: &str
pub fn vyre_libs::decode::ziftsieve::ziftsieve_gpu(input: &str, output: &str, seq_literal_start: &str, seq_literal_len: &str, seq_literal_offset: &str, input_len: u32, seq_count: u32, max_output: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub const vyre_libs::decode::BASE64_DECODE_TABLE_BUFFER: &str
pub const vyre_libs::decode::HEX_DECODE_TABLE_BUFFER: &str
pub fn vyre_libs::decode::base64_decode(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::base64_decode_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::encodex_gpu(input: &str, output: &str, count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::encodex_reference(input: &[u8]) -> u32
pub fn vyre_libs::decode::hex_decode(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::hex_decode_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate_stored_block(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate_stored_block_buffered_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate_stored_block_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate_stored_block_tiled_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32, tile_width: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::inflate_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::decode::ziftsieve_gpu(input: &str, output: &str, seq_literal_start: &str, seq_literal_len: &str, seq_literal_offset: &str, input_len: u32, seq_count: u32, max_output: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::descriptor
#[non_exhaustive] pub struct vyre_libs::descriptor::BufferDescriptor
pub vyre_libs::descriptor::BufferDescriptor::access: vyre_spec::buffer_access::BufferAccess
pub vyre_libs::descriptor::BufferDescriptor::count: u32
pub vyre_libs::descriptor::BufferDescriptor::dtype: vyre_spec::data_type::DataType
pub vyre_libs::descriptor::BufferDescriptor::name: alloc::string::String
impl vyre_libs::descriptor::BufferDescriptor
pub fn vyre_libs::descriptor::BufferDescriptor::new(name: alloc::string::String, access: vyre_spec::buffer_access::BufferAccess, dtype: vyre_spec::data_type::DataType, count: u32) -> Self
impl core::clone::Clone for vyre_libs::descriptor::BufferDescriptor
pub fn vyre_libs::descriptor::BufferDescriptor::clone(&self) -> vyre_libs::descriptor::BufferDescriptor
impl core::fmt::Debug for vyre_libs::descriptor::BufferDescriptor
pub fn vyre_libs::descriptor::BufferDescriptor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::descriptor::BufferDescriptor
impl core::marker::Send for vyre_libs::descriptor::BufferDescriptor
impl core::marker::Sync for vyre_libs::descriptor::BufferDescriptor
impl core::marker::Unpin for vyre_libs::descriptor::BufferDescriptor
impl core::marker::UnsafeUnpin for vyre_libs::descriptor::BufferDescriptor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::descriptor::BufferDescriptor
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::descriptor::BufferDescriptor
impl<T, U> core::convert::Into<U> for vyre_libs::descriptor::BufferDescriptor where U: core::convert::From<T>
pub fn vyre_libs::descriptor::BufferDescriptor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::descriptor::BufferDescriptor where U: core::convert::Into<T>
pub type vyre_libs::descriptor::BufferDescriptor::Error = core::convert::Infallible
pub fn vyre_libs::descriptor::BufferDescriptor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::descriptor::BufferDescriptor where U: core::convert::TryFrom<T>
pub type vyre_libs::descriptor::BufferDescriptor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::descriptor::BufferDescriptor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::descriptor::BufferDescriptor where T: core::clone::Clone
pub type vyre_libs::descriptor::BufferDescriptor::Owned = T
pub fn vyre_libs::descriptor::BufferDescriptor::clone_into(&self, target: &mut T)
pub fn vyre_libs::descriptor::BufferDescriptor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::descriptor::BufferDescriptor where T: 'static + ?core::marker::Sized
pub fn vyre_libs::descriptor::BufferDescriptor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::descriptor::BufferDescriptor where T: ?core::marker::Sized
pub fn vyre_libs::descriptor::BufferDescriptor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::descriptor::BufferDescriptor where T: ?core::marker::Sized
pub fn vyre_libs::descriptor::BufferDescriptor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::descriptor::BufferDescriptor where T: core::clone::Clone
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::descriptor::BufferDescriptor
pub fn vyre_libs::descriptor::BufferDescriptor::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::descriptor::BufferDescriptor
pub type vyre_libs::descriptor::BufferDescriptor::Init = T
pub const vyre_libs::descriptor::BufferDescriptor::ALIGN: usize
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::drop(ptr: usize)
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::descriptor::BufferDescriptor
impl<T> tracing::instrument::Instrument for vyre_libs::descriptor::BufferDescriptor
impl<T> tracing::instrument::WithSubscriber for vyre_libs::descriptor::BufferDescriptor
impl<T> typenum::type_operators::Same for vyre_libs::descriptor::BufferDescriptor
pub type vyre_libs::descriptor::BufferDescriptor::Output = T
#[non_exhaustive] pub struct vyre_libs::descriptor::ProgramDescriptor
pub vyre_libs::descriptor::ProgramDescriptor::buffer_count: usize
pub vyre_libs::descriptor::ProgramDescriptor::buffers: alloc::vec::Vec<vyre_libs::descriptor::BufferDescriptor>
pub vyre_libs::descriptor::ProgramDescriptor::entry_node_count: usize
pub vyre_libs::descriptor::ProgramDescriptor::rw_bytes_lower_bound: usize
pub vyre_libs::descriptor::ProgramDescriptor::workgroup_size: [u32; 3]
impl vyre_libs::descriptor::ProgramDescriptor
pub fn vyre_libs::descriptor::ProgramDescriptor::from_program(program: &vyre_foundation::ir_inner::model::program::core::Program) -> Self
pub fn vyre_libs::descriptor::ProgramDescriptor::new(buffer_count: usize, workgroup_size: [u32; 3], buffers: alloc::vec::Vec<vyre_libs::descriptor::BufferDescriptor>, rw_bytes_lower_bound: usize, entry_node_count: usize) -> Self
impl core::clone::Clone for vyre_libs::descriptor::ProgramDescriptor
pub fn vyre_libs::descriptor::ProgramDescriptor::clone(&self) -> vyre_libs::descriptor::ProgramDescriptor
impl core::fmt::Debug for vyre_libs::descriptor::ProgramDescriptor
pub fn vyre_libs::descriptor::ProgramDescriptor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::descriptor::ProgramDescriptor
impl core::marker::Send for vyre_libs::descriptor::ProgramDescriptor
impl core::marker::Sync for vyre_libs::descriptor::ProgramDescriptor
impl core::marker::Unpin for vyre_libs::descriptor::ProgramDescriptor
impl core::marker::UnsafeUnpin for vyre_libs::descriptor::ProgramDescriptor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::descriptor::ProgramDescriptor
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::descriptor::ProgramDescriptor
impl<T, U> core::convert::Into<U> for vyre_libs::descriptor::ProgramDescriptor where U: core::convert::From<T>
pub fn vyre_libs::descriptor::ProgramDescriptor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::descriptor::ProgramDescriptor where U: core::convert::Into<T>
pub type vyre_libs::descriptor::ProgramDescriptor::Error = core::convert::Infallible
pub fn vyre_libs::descriptor::ProgramDescriptor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::descriptor::ProgramDescriptor where U: core::convert::TryFrom<T>
pub type vyre_libs::descriptor::ProgramDescriptor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::descriptor::ProgramDescriptor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::descriptor::ProgramDescriptor where T: core::clone::Clone
pub type vyre_libs::descriptor::ProgramDescriptor::Owned = T
pub fn vyre_libs::descriptor::ProgramDescriptor::clone_into(&self, target: &mut T)
pub fn vyre_libs::descriptor::ProgramDescriptor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::descriptor::ProgramDescriptor where T: 'static + ?core::marker::Sized
pub fn vyre_libs::descriptor::ProgramDescriptor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::descriptor::ProgramDescriptor where T: ?core::marker::Sized
pub fn vyre_libs::descriptor::ProgramDescriptor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::descriptor::ProgramDescriptor where T: ?core::marker::Sized
pub fn vyre_libs::descriptor::ProgramDescriptor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::descriptor::ProgramDescriptor where T: core::clone::Clone
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::descriptor::ProgramDescriptor
pub fn vyre_libs::descriptor::ProgramDescriptor::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::descriptor::ProgramDescriptor
pub type vyre_libs::descriptor::ProgramDescriptor::Init = T
pub const vyre_libs::descriptor::ProgramDescriptor::ALIGN: usize
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::drop(ptr: usize)
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::descriptor::ProgramDescriptor
impl<T> tracing::instrument::Instrument for vyre_libs::descriptor::ProgramDescriptor
impl<T> tracing::instrument::WithSubscriber for vyre_libs::descriptor::ProgramDescriptor
impl<T> typenum::type_operators::Same for vyre_libs::descriptor::ProgramDescriptor
pub type vyre_libs::descriptor::ProgramDescriptor::Output = T
pub mod vyre_libs::graph
pub mod vyre_libs::graph::ast_walk_postorder
pub fn vyre_libs::graph::ast_walk_postorder::ast_walk_postorder(out: &str, node_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::graph::ast_walk_postorder::ast_walk_postorder_nodes(nodes: &str, out: &str, node_count: u32, out_cap: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::graph::ast_walk_preorder
pub fn vyre_libs::graph::ast_walk_preorder::ast_walk_preorder(nodes: &str, out: &str, node_count: u32, out_cap: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::graph::ast_walk_postorder(out: &str, node_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::graph::ast_walk_postorder_nodes(nodes: &str, out: &str, node_count: u32, out_cap: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::graph::ast_walk_preorder(nodes: &str, out: &str, node_count: u32, out_cap: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::hash
pub mod vyre_libs::hash::adler32
pub fn vyre_libs::hash::adler32::adler32(input: &str, out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::hash::blake3_compress
pub fn vyre_libs::hash::blake3_compress::blake3_compress(chaining_in: &str, message: &str, params: &str, chaining_out: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::hash::crc32
pub fn vyre_libs::hash::crc32::crc32(input: &str, out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::hash::fnv1a32
pub fn vyre_libs::hash::fnv1a32::fnv1a32(input: &str, out: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::hash::fnv1a32::fnv1a32_n(input: &str, out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::hash::fnv1a64
pub fn vyre_libs::hash::fnv1a64::fnv1a64(input: &str, out: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::hash::fnv1a64::fnv1a64_n(input: &str, out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::hash::multi_hash
pub fn vyre_libs::hash::multi_hash::multi_hash(input: &str, out_crc32: &str, out_fnv1a32: &str, out_adler32: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::hash::adler32(input: &str, out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::hash::blake3_compress(chaining_in: &str, message: &str, params: &str, chaining_out: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::hash::crc32(input: &str, out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::hash::fnv1a32(input: &str, out: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::hash::fnv1a64(input: &str, out: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::hash::multi_hash(input: &str, out_crc32: &str, out_fnv1a32: &str, out_adler32: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::matching
pub use vyre_libs::matching::CompiledDfa
pub use vyre_libs::matching::DEFAULT_DFA_BUDGET_BYTES
pub use vyre_libs::matching::DfaCompileError
pub use vyre_libs::matching::FusionError
pub use vyre_libs::matching::LiteralMatch
pub use vyre_libs::matching::RegionTriple
pub use vyre_libs::matching::dedup_regions_flag_program
pub use vyre_libs::matching::dedup_regions_inplace
pub use vyre_libs::matching::dfa_compile
pub use vyre_libs::matching::dfa_compile_with_budget
pub use vyre_libs::matching::fuse_programs
pub use vyre_libs::matching::fuse_programs_vec
pub mod vyre_libs::matching::classic_ac
pub struct vyre_libs::matching::classic_ac::ClassicAcAutomaton
pub vyre_libs::matching::classic_ac::ClassicAcAutomaton::dfa: vyre_primitives::matching::dfa_compile::CompiledDfa
impl core::clone::Clone for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::clone(&self) -> vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::fmt::Debug for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::marker::Send for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::marker::Sync for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::marker::Unpin for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::marker::UnsafeUnpin for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl<T, U> core::convert::Into<U> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where U: core::convert::From<T>
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where U: core::convert::Into<T>
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Error = core::convert::Infallible
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: core::clone::Clone
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Owned = T
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: ?core::marker::Sized
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: ?core::marker::Sized
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Init = T
pub const vyre_libs::scan::classic_ac::ClassicAcAutomaton::ALIGN: usize
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl<T> tracing::instrument::Instrument for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl<T> typenum::type_operators::Same for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Output = T
pub fn vyre_libs::matching::classic_ac::build_ac_bounded_count_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::build_ac_bounded_count_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::build_ac_bounded_ranges_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::build_ac_bounded_ranges_prefilter_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::build_ac_bounded_ranges_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::build_ac_bounded_ranges_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::build_ac_bounded_ranges_suffix3_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::build_ac_bounded_ranges_suffix3_prefilter_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_count_prefilter_program(haystack: &str, transitions: &str, output_offsets: &str, candidate_end_mask: &str, haystack_len: &str, match_count: &str, state_count: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_count_program(haystack: &str, transitions: &str, output_offsets: &str, haystack_len: &str, match_count: &str, state_count: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_ranges_prefilter_program(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, candidate_end_mask: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_ranges_prefilter_program_ext(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, candidate_end_mask: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_ranges_program(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_ranges_program_ext(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_ranges_scan(ac: &vyre_libs::scan::classic_ac::ClassicAcAutomaton, pattern_lengths: &[u32], haystack: &[u8]) -> alloc::vec::Vec<(u32, u32, u32)>
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_ranges_suffix3_prefilter_program(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, candidate_end_mask: &str, candidate_suffix2_mask: &str, candidate_suffix3_bloom: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_bounded_ranges_suffix3_prefilter_program_ext(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, candidate_end_mask: &str, candidate_suffix2_mask: &str, candidate_suffix3_bloom: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_candidate_end_byte_mask_words(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> [u32; 8]
pub fn vyre_libs::matching::classic_ac::classic_ac_compile(patterns: &[&[u8]]) -> vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub fn vyre_libs::matching::classic_ac::classic_ac_program(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, match_count: &str, matches: &str, haystack_len: u32, state_count: u32, output_records_len: u32, max_matches: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::classic_ac::classic_ac_scan(ac: &vyre_libs::scan::classic_ac::ClassicAcAutomaton, haystack: &[u8]) -> alloc::vec::Vec<(u32, u32)>
pub fn vyre_libs::matching::classic_ac::classic_ac_scan_counts(ac: &vyre_libs::scan::classic_ac::ClassicAcAutomaton, haystack: &[u8]) -> alloc::vec::Vec<u32>
pub mod vyre_libs::matching::dispatch_io
pub fn vyre_libs::matching::dispatch_io::byte_scan_dispatch_config(haystack_len: u32, workgroup_x: u32) -> vyre_driver::backend::dispatch_config::DispatchConfig
pub fn vyre_libs::matching::dispatch_io::candidate_start_dispatch_config(haystack_len: u32) -> vyre_driver::backend::dispatch_config::DispatchConfig
pub fn vyre_libs::matching::dispatch_io::haystack_len_u32(haystack: &[u8], context: &str) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::matching::dispatch_io::pack_haystack_u32(haystack: &[u8]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::matching::dispatch_io::pack_u32_slice(words: &[u32]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::matching::dispatch_io::scan_guard(haystack: &[u8], context: &str, max_bytes: u32) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::matching::dispatch_io::u32_words_as_le_bytes(words: &[u32]) -> alloc::borrow::Cow<'_, [u8]>
pub fn vyre_libs::matching::dispatch_io::unpack_match_triples(triples_bytes: &[u8], count: u32) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::matching::dispatch_io::unpack_match_triples_into(triples_bytes: &[u8], count: u32, results: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>)
pub mod vyre_libs::matching::substring
pub mod vyre_libs::matching::substring::substring
pub const vyre_libs::matching::substring::substring::CANONICAL_SUBSTRING_MODULE: &str
pub const vyre_libs::matching::substring::substring::LEGACY_SUBSTRING_MODULE: &str
pub fn vyre_libs::matching::substring::substring::substring_search(haystack: &str, needle: &str, matches: &str, haystack_len: u32, needle_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::substring::substring_search(haystack: &str, needle: &str, matches: &str, haystack_len: u32, needle_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub enum vyre_libs::matching::ApiKind
pub vyre_libs::matching::ApiKind::Const
pub vyre_libs::matching::ApiKind::Enum
pub vyre_libs::matching::ApiKind::Function
pub vyre_libs::matching::ApiKind::Struct
pub vyre_libs::matching::ApiKind::Trait
pub vyre_libs::matching::ApiKind::TypeAlias
impl core::clone::Clone for vyre_libs::scan::ApiKind
pub fn vyre_libs::scan::ApiKind::clone(&self) -> vyre_libs::scan::ApiKind
impl core::cmp::Eq for vyre_libs::scan::ApiKind
impl core::cmp::PartialEq for vyre_libs::scan::ApiKind
pub fn vyre_libs::scan::ApiKind::eq(&self, other: &vyre_libs::scan::ApiKind) -> bool
impl core::fmt::Debug for vyre_libs::scan::ApiKind
pub fn vyre_libs::scan::ApiKind::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::scan::ApiKind
impl core::marker::StructuralPartialEq for vyre_libs::scan::ApiKind
impl core::marker::Freeze for vyre_libs::scan::ApiKind
impl core::marker::Send for vyre_libs::scan::ApiKind
impl core::marker::Sync for vyre_libs::scan::ApiKind
impl core::marker::Unpin for vyre_libs::scan::ApiKind
impl core::marker::UnsafeUnpin for vyre_libs::scan::ApiKind
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::ApiKind
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::ApiKind
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::scan::ApiKind where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::scan::ApiKind where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::scan::ApiKind where U: core::convert::From<T>
pub fn vyre_libs::scan::ApiKind::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::ApiKind where U: core::convert::Into<T>
pub type vyre_libs::scan::ApiKind::Error = core::convert::Infallible
pub fn vyre_libs::scan::ApiKind::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::ApiKind where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::ApiKind::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::ApiKind::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::ApiKind where T: core::clone::Clone
pub type vyre_libs::scan::ApiKind::Owned = T
pub fn vyre_libs::scan::ApiKind::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::ApiKind::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::ApiKind where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::ApiKind where T: ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::ApiKind where T: ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::ApiKind where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::ApiKind::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::ApiKind
pub fn vyre_libs::scan::ApiKind::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::ApiKind
pub type vyre_libs::scan::ApiKind::Init = T
pub const vyre_libs::scan::ApiKind::ALIGN: usize
pub unsafe fn vyre_libs::scan::ApiKind::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::ApiKind::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::ApiKind::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::ApiKind::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::ApiKind
impl<T> tracing::instrument::Instrument for vyre_libs::scan::ApiKind
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::ApiKind
impl<T> typenum::type_operators::Same for vyre_libs::scan::ApiKind
pub type vyre_libs::scan::ApiKind::Output = T
#[non_exhaustive] pub enum vyre_libs::matching::LiteralSetWireError
pub vyre_libs::matching::LiteralSetWireError::InvalidDfa(vyre_primitives::matching::dfa_compile::DfaWireError)
pub vyre_libs::matching::LiteralSetWireError::InvalidProgram(alloc::string::String)
pub vyre_libs::matching::LiteralSetWireError::WireFraming(vyre_foundation::serial::envelope::EnvelopeError)
impl core::error::Error for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::string::ToString for vyre_libs::scan::literal_set::LiteralSetWireError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetWireError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetWireError where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetWireError where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetWireError
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetWireError::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetWireError
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Output = T
pub enum vyre_libs::matching::PostProcessError
pub vyre_libs::matching::PostProcessError::InvalidRange
pub vyre_libs::matching::PostProcessError::InvalidRange::end: u32
pub vyre_libs::matching::PostProcessError::InvalidRange::haystack_len: usize
pub vyre_libs::matching::PostProcessError::InvalidRange::pattern_id: u32
pub vyre_libs::matching::PostProcessError::InvalidRange::start: u32
impl core::clone::Clone for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::clone(&self) -> vyre_libs::scan::post_process::PostProcessError
impl core::cmp::Eq for vyre_libs::scan::post_process::PostProcessError
impl core::cmp::PartialEq for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::eq(&self, other: &vyre_libs::scan::post_process::PostProcessError) -> bool
impl core::error::Error for vyre_libs::scan::post_process::PostProcessError
impl core::fmt::Debug for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::scan::post_process::PostProcessError
impl core::marker::StructuralPartialEq for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Freeze for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Send for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Sync for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Unpin for vyre_libs::scan::post_process::PostProcessError
impl core::marker::UnsafeUnpin for vyre_libs::scan::post_process::PostProcessError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::post_process::PostProcessError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::post_process::PostProcessError
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::scan::post_process::PostProcessError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::scan::post_process::PostProcessError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::From<T>
pub fn vyre_libs::scan::post_process::PostProcessError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::Into<T>
pub type vyre_libs::scan::post_process::PostProcessError::Error = core::convert::Infallible
pub fn vyre_libs::scan::post_process::PostProcessError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::post_process::PostProcessError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::post_process::PostProcessError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::post_process::PostProcessError where T: core::clone::Clone
pub type vyre_libs::scan::post_process::PostProcessError::Owned = T
pub fn vyre_libs::scan::post_process::PostProcessError::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::post_process::PostProcessError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_libs::scan::post_process::PostProcessError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::scan::post_process::PostProcessError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::post_process::PostProcessError where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::post_process::PostProcessError where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::post_process::PostProcessError where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::post_process::PostProcessError
pub type vyre_libs::scan::post_process::PostProcessError::Init = T
pub const vyre_libs::scan::post_process::PostProcessError::ALIGN: usize
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::post_process::PostProcessError
impl<T> tracing::instrument::Instrument for vyre_libs::scan::post_process::PostProcessError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::post_process::PostProcessError
impl<T> typenum::type_operators::Same for vyre_libs::scan::post_process::PostProcessError
pub type vyre_libs::scan::post_process::PostProcessError::Output = T
pub struct vyre_libs::matching::DirectGpuScanner
impl vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::compile(patterns: &[&[u8]]) -> Self
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::literal_set_cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::program(&self) -> &vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl !core::marker::Freeze for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Send for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Sync for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Unpin for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::UnsafeUnpin for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T, U> core::convert::Into<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::From<T>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::Into<T>
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Error = core::convert::Infallible
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Init = T
pub const vyre_libs::scan::direct_gpu::DirectGpuScanner::ALIGN: usize
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> tracing::instrument::Instrument for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> typenum::type_operators::Same for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Output = T
pub struct vyre_libs::matching::GpuLiteralSet
pub vyre_libs::matching::GpuLiteralSet::dfa: vyre_primitives::matching::dfa_compile::CompiledDfa
pub vyre_libs::matching::GpuLiteralSet::pattern_bytes: alloc::vec::Vec<u32>
pub vyre_libs::matching::GpuLiteralSet::pattern_lengths: alloc::vec::Vec<u32>
pub vyre_libs::matching::GpuLiteralSet::pattern_offsets: alloc::vec::Vec<u32>
pub vyre_libs::matching::GpuLiteralSet::program: vyre_foundation::ir_inner::model::program::core::Program
impl vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::compile(patterns: &[&[u8]]) -> Self
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::count<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8]) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::count_with_literal_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, vyre_libs::scan::literal_set::LiteralSetWireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_count_dispatch(&self, haystack: &[u8]) -> core::result::Result<vyre_libs::scan::literal_set::LiteralSetPreparedCount, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_count_scratch(&self, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_literal_scratch(&self, max_matches: u32, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_scan_dispatch(&self, haystack: &[u8], max_matches: u32) -> core::result::Result<vyre_libs::scan::literal_set::LiteralSetPreparedScan, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into_with_literal_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into_with_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, scratch: &mut vyre_libs::scan::dispatch_io::ScanDispatchScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_presence<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<u32>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_presence_with_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], scratch: &mut vyre_libs::scan::dispatch_io::ScanDispatchScratch) -> core::result::Result<alloc::vec::Vec<u32>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, vyre_libs::scan::literal_set::LiteralSetWireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_compile(patterns: &[&[u8]]) -> core::result::Result<Self, vyre_libs::scan::literal_set::LiteralSetCompileError>
impl vyre_libs::scan::engine::MatchEngineCache for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::WireError = vyre_libs::scan::literal_set::LiteralSetWireError
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_VERSION: u32
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl !core::marker::Freeze for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Send for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Sync for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Unpin for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::GpuLiteralSet
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::GpuLiteralSet
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::literal_set::GpuLiteralSet where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::GpuLiteralSet where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::GpuLiteralSet where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Init = T
pub const vyre_libs::scan::literal_set::GpuLiteralSet::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Output = T
pub struct vyre_libs::matching::Pipeline<E>
pub vyre_libs::matching::Pipeline::engine: E
pub vyre_libs::matching::Pipeline::post_process: vyre_libs::scan::pipeline::PostProcessFn
impl<E: vyre_libs::scan::engine::MatchScan> vyre_libs::scan::pipeline::Pipeline<E>
pub const fn vyre_libs::scan::pipeline::Pipeline<E>::new(engine: E) -> Self
pub fn vyre_libs::scan::pipeline::Pipeline<E>::reference_scan_processed(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::scan_processed(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_reference_scan_processed(&self, haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub const fn vyre_libs::scan::pipeline::Pipeline<E>::with_post_process(engine: E, post_process: vyre_libs::scan::pipeline::PostProcessFn) -> Self
impl<E: core::clone::Clone> core::clone::Clone for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::clone(&self) -> Self
impl<E: core::fmt::Debug> core::fmt::Debug for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl<E> core::marker::Freeze for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Freeze
impl<E> core::marker::Send for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Send
impl<E> core::marker::Sync for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Sync
impl<E> core::marker::Unpin for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Unpin
impl<E> core::marker::UnsafeUnpin for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::UnsafeUnpin
impl<E> core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::pipeline::Pipeline<E> where E: core::panic::unwind_safe::RefUnwindSafe
impl<E> core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::pipeline::Pipeline<E> where E: core::panic::unwind_safe::UnwindSafe
impl<T, U> core::convert::Into<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::From<T>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::Into<T>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Error = core::convert::Infallible
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::pipeline::Pipeline<E> where T: core::clone::Clone
pub type vyre_libs::scan::pipeline::Pipeline<E>::Owned = T
pub fn vyre_libs::scan::pipeline::Pipeline<E>::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::pipeline::Pipeline<E>::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::pipeline::Pipeline<E> where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::pipeline::Pipeline<E> where T: ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::pipeline::Pipeline<E> where T: ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::pipeline::Pipeline<E> where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::pipeline::Pipeline<E>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Init = T
pub const vyre_libs::scan::pipeline::Pipeline<E>::ALIGN: usize
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> tracing::instrument::Instrument for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> typenum::type_operators::Same for vyre_libs::scan::pipeline::Pipeline<E>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Output = T
pub struct vyre_libs::matching::PostProcessedMatch
pub vyre_libs::matching::PostProcessedMatch::confidence: f32
pub vyre_libs::matching::PostProcessedMatch::end: u32
pub vyre_libs::matching::PostProcessedMatch::entropy_bits_per_byte: f32
pub vyre_libs::matching::PostProcessedMatch::pattern_id: u32
pub vyre_libs::matching::PostProcessedMatch::start: u32
impl core::clone::Clone for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::clone(&self) -> vyre_libs::scan::post_process::PostProcessedMatch
impl core::cmp::PartialEq for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::eq(&self, other: &vyre_libs::scan::post_process::PostProcessedMatch) -> bool
impl core::fmt::Debug for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::StructuralPartialEq for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Freeze for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Send for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Sync for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Unpin for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::UnsafeUnpin for vyre_libs::scan::post_process::PostProcessedMatch
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::post_process::PostProcessedMatch
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::post_process::PostProcessedMatch
impl<T, U> core::convert::Into<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::From<T>
pub fn vyre_libs::scan::post_process::PostProcessedMatch::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::Into<T>
pub type vyre_libs::scan::post_process::PostProcessedMatch::Error = core::convert::Infallible
pub fn vyre_libs::scan::post_process::PostProcessedMatch::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::post_process::PostProcessedMatch::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::post_process::PostProcessedMatch::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::post_process::PostProcessedMatch where T: core::clone::Clone
pub type vyre_libs::scan::post_process::PostProcessedMatch::Owned = T
pub fn vyre_libs::scan::post_process::PostProcessedMatch::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::post_process::PostProcessedMatch::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::post_process::PostProcessedMatch where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::post_process::PostProcessedMatch where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::post_process::PostProcessedMatch where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::post_process::PostProcessedMatch where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::post_process::PostProcessedMatch
pub type vyre_libs::scan::post_process::PostProcessedMatch::Init = T
pub const vyre_libs::scan::post_process::PostProcessedMatch::ALIGN: usize
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> tracing::instrument::Instrument for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> typenum::type_operators::Same for vyre_libs::scan::post_process::PostProcessedMatch
pub type vyre_libs::scan::post_process::PostProcessedMatch::Output = T
pub struct vyre_libs::matching::ScanResult
pub vyre_libs::matching::ScanResult::cache_hit: bool
pub vyre_libs::matching::ScanResult::elapsed: core::time::Duration
pub vyre_libs::matching::ScanResult::matches: alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub vyre_libs::matching::ScanResult::truncated: bool
impl vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::from_matches(matches: alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> Self
pub fn vyre_libs::scan::engine::ScanResult::is_empty(&self) -> bool
pub fn vyre_libs::scan::engine::ScanResult::len(&self) -> usize
impl core::clone::Clone for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::clone(&self) -> vyre_libs::scan::engine::ScanResult
impl core::default::Default for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::default() -> vyre_libs::scan::engine::ScanResult
impl core::fmt::Debug for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::engine::ScanResult
impl core::marker::Send for vyre_libs::scan::engine::ScanResult
impl core::marker::Sync for vyre_libs::scan::engine::ScanResult
impl core::marker::Unpin for vyre_libs::scan::engine::ScanResult
impl core::marker::UnsafeUnpin for vyre_libs::scan::engine::ScanResult
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::engine::ScanResult
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::engine::ScanResult
impl<T, U> core::convert::Into<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::From<T>
pub fn vyre_libs::scan::engine::ScanResult::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::Into<T>
pub type vyre_libs::scan::engine::ScanResult::Error = core::convert::Infallible
pub fn vyre_libs::scan::engine::ScanResult::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::engine::ScanResult::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::engine::ScanResult::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::engine::ScanResult where T: core::clone::Clone
pub type vyre_libs::scan::engine::ScanResult::Owned = T
pub fn vyre_libs::scan::engine::ScanResult::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::engine::ScanResult::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::engine::ScanResult where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::engine::ScanResult where T: ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::engine::ScanResult where T: ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::engine::ScanResult where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::engine::ScanResult::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::engine::ScanResult
pub type vyre_libs::scan::engine::ScanResult::Init = T
pub const vyre_libs::scan::engine::ScanResult::ALIGN: usize
pub unsafe fn vyre_libs::scan::engine::ScanResult::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::engine::ScanResult::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::engine::ScanResult::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::engine::ScanResult::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::engine::ScanResult
impl<T> tracing::instrument::Instrument for vyre_libs::scan::engine::ScanResult
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::engine::ScanResult
impl<T> typenum::type_operators::Same for vyre_libs::scan::engine::ScanResult
pub type vyre_libs::scan::engine::ScanResult::Output = T
pub const vyre_libs::matching::API_INDEX: &[(&str, vyre_libs::scan::ApiKind, core::option::Option<&str>)]
pub const vyre_libs::matching::DEFAULT_MAX_SCAN_BYTES: u32
pub const vyre_libs::matching::HIT_BUFFER_LIVE_LENGTH: &str
pub const vyre_libs::matching::HIT_BUFFER_OVERFLOW_COUNT: &str
pub trait vyre_libs::matching::MatchEngineCache: core::marker::Sized
pub type vyre_libs::matching::MatchEngineCache::WireError: core::fmt::Display + core::fmt::Debug
pub const vyre_libs::matching::MatchEngineCache::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::matching::MatchEngineCache::WIRE_VERSION: u32
pub fn vyre_libs::matching::MatchEngineCache::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::matching::MatchEngineCache::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
impl vyre_libs::scan::engine::MatchEngineCache for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::WireError = vyre_libs::scan::literal_set::LiteralSetWireError
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_VERSION: u32
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
pub trait vyre_libs::matching::MatchScan
pub fn vyre_libs::matching::MatchScan::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::matching::MatchScan::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::matching::MatchScan::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::matching::aho_corasick(haystack: &str, transitions: &str, accept: &str, matches: &str, haystack_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::byte_scan_dispatch_config(haystack_len: u32, workgroup_x: u32) -> vyre_driver::backend::dispatch_config::DispatchConfig
pub fn vyre_libs::matching::cached_load_or_compile<E, F>(cache_dir: &std::path::Path, cache_key: &str, compile: F) -> E where E: vyre_libs::scan::engine::MatchEngineCache, F: core::ops::function::FnOnce() -> E
pub fn vyre_libs::matching::candidate_start_dispatch_config(haystack_len: u32) -> vyre_driver::backend::dispatch_config::DispatchConfig
pub fn vyre_libs::matching::compact_hits(out_hits: &str, out_cursor: &str, max_capacity: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::compact_hits_with_layout(out_hits: &str, out_cursor: &str, hit_capacity: u32, max_capacity: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::dedup_regions_reference(input: alloc::vec::Vec<vyre_primitives::matching::region::RegionTriple>) -> alloc::vec::Vec<vyre_primitives::matching::region::RegionTriple>
pub fn vyre_libs::matching::emit_hit(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::emit_hit_then_compact(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::execution_plan::fusion::FusionError>
pub fn vyre_libs::matching::emit_hit_then_compact_with_layout(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str, lane_count: u32, max_hits: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::execution_plan::fusion::FusionError>
pub fn vyre_libs::matching::emit_hit_with_layout(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str, lane_count: u32, max_hits: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::engine_cache_path(cache_dir: &std::path::Path, cache_key: &str) -> core::option::Option<std::path::PathBuf>
pub fn vyre_libs::matching::haystack_len_u32(haystack: &[u8], context: &str) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::matching::pack_haystack_u32(haystack: &[u8]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::matching::pack_u32_slice(words: &[u32]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::matching::scan_guard(haystack: &[u8], context: &str, max_bytes: u32) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::matching::shannon_entropy_bits_per_byte(bytes: &[u8]) -> f32
pub fn vyre_libs::matching::substring_search(haystack: &str, needle: &str, matches: &str, haystack_len: u32, needle_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matching::try_reference_post_process(matches: &[vyre_foundation::runtime::match_result::Match], haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub fn vyre_libs::matching::try_reference_post_process_into(matches: &[vyre_foundation::runtime::match_result::Match], haystack: &[u8], triples: &mut alloc::vec::Vec<vyre_primitives::matching::region::RegionTriple>, out: &mut alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>) -> core::result::Result<(), vyre_libs::scan::post_process::PostProcessError>
pub fn vyre_libs::matching::u32_words_as_le_bytes(words: &[u32]) -> alloc::borrow::Cow<'_, [u8]>
pub fn vyre_libs::matching::unpack_match_triples(triples_bytes: &[u8], count: u32) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub type vyre_libs::matching::PostProcessFn = fn(&[vyre_foundation::runtime::match_result::Match], &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub mod vyre_libs::math
pub mod vyre_libs::math::atomic
pub mod vyre_libs::math::atomic::atomic_add
pub fn vyre_libs::math::atomic::atomic_add::atomic_add_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::atomic::atomic_and
pub fn vyre_libs::math::atomic::atomic_and::atomic_and_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::atomic::atomic_compare_exchange
pub fn vyre_libs::math::atomic::atomic_compare_exchange::atomic_compare_exchange_u32(expected: &str, desired: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::atomic::atomic_exchange
pub fn vyre_libs::math::atomic::atomic_exchange::atomic_exchange_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::atomic::atomic_lru_update
pub fn vyre_libs::math::atomic::atomic_lru_update::atomic_lru_update_u32(buffer: &str, index: vyre_foundation::ir_inner::model::generated::Expr, timestamp: vyre_foundation::ir_inner::model::generated::Expr) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::atomic::atomic_max
pub fn vyre_libs::math::atomic::atomic_max::atomic_max_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::atomic::atomic_min
pub fn vyre_libs::math::atomic::atomic_min::atomic_min_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::atomic::atomic_or
pub fn vyre_libs::math::atomic::atomic_or::atomic_or_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::atomic::atomic_xor
pub fn vyre_libs::math::atomic::atomic_xor::atomic_xor_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_add_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_and_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_compare_exchange_u32(expected: &str, desired: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_exchange_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_lru_update_u32(buffer: &str, index: vyre_foundation::ir_inner::model::generated::Expr, timestamp: vyre_foundation::ir_inner::model::generated::Expr) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_max_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_min_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_or_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic::atomic_xor_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::avg_floor
pub fn vyre_libs::math::avg_floor::avg_floor(a: &str, b: &str, out: &str, size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::broadcast
pub fn vyre_libs::math::broadcast::broadcast(src: &str, dst: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::clamp_u32
pub fn vyre_libs::math::clamp_u32::clamp_u32(input: &str, lo: &str, hi: &str, out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::conv
pub mod vyre_libs::math::conv::conv2d
pub fn vyre_libs::math::conv::conv2d::conv2d_3x3_direct(input: &str, kernel: &str, output: &str, h: u32, w: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::math::conv::im2col
pub fn vyre_libs::math::conv::im2col::im2col_3x3(input: &str, output: &str, h: u32, w: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::math::conv::conv2d_3x3_decision(input: &str, kernel: &str, output: &str, h: u32, w: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::math::conv::conv2d_3x3_direct(input: &str, kernel: &str, output: &str, h: u32, w: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::math::conv::im2col_3x3(input: &str, output: &str, h: u32, w: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::math::fft
pub mod vyre_libs::math::fft::convolution
pub fn vyre_libs::math::fft::convolution::fft_convolve_circular_complex(signal: &str, kernel: &str, signal_freq: &str, kernel_freq: &str, product_freq: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::math::fft::fft4
pub fn vyre_libs::math::fft::fft4::fft4_complex(input: &str, output: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::fft::fft_radix2
pub fn vyre_libs::math::fft::fft_radix2::fft_radix2_complex(input: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::math::fft::fft4_complex(input: &str, output: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::fft::fft_convolve_circular_complex(signal: &str, kernel: &str, signal_freq: &str, kernel_freq: &str, product_freq: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::math::fft::fft_radix2_complex(input: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::math::linalg
pub struct vyre_libs::math::linalg::Dot
impl vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::math::Dot::new(lhs: vyre_libs::tensor_ref::TensorRef, rhs: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::math::Dot::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::math::Dot::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::clone(&self) -> vyre_libs::math::Dot
impl core::fmt::Debug for vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::math::Dot
impl core::marker::Send for vyre_libs::math::Dot
impl core::marker::Sync for vyre_libs::math::Dot
impl core::marker::Unpin for vyre_libs::math::Dot
impl core::marker::UnsafeUnpin for vyre_libs::math::Dot
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::math::Dot
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::math::Dot
impl<T, U> core::convert::Into<U> for vyre_libs::math::Dot where U: core::convert::From<T>
pub fn vyre_libs::math::Dot::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::math::Dot where U: core::convert::Into<T>
pub type vyre_libs::math::Dot::Error = core::convert::Infallible
pub fn vyre_libs::math::Dot::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::math::Dot where U: core::convert::TryFrom<T>
pub type vyre_libs::math::Dot::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::math::Dot::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::math::Dot where T: core::clone::Clone
pub type vyre_libs::math::Dot::Owned = T
pub fn vyre_libs::math::Dot::clone_into(&self, target: &mut T)
pub fn vyre_libs::math::Dot::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::math::Dot where T: 'static + ?core::marker::Sized
pub fn vyre_libs::math::Dot::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::math::Dot where T: ?core::marker::Sized
pub fn vyre_libs::math::Dot::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::math::Dot where T: ?core::marker::Sized
pub fn vyre_libs::math::Dot::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::math::Dot where T: core::clone::Clone
pub unsafe fn vyre_libs::math::Dot::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::math::Dot
pub type vyre_libs::math::Dot::Init = T
pub const vyre_libs::math::Dot::ALIGN: usize
pub unsafe fn vyre_libs::math::Dot::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::math::Dot::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::math::Dot::drop(ptr: usize)
pub unsafe fn vyre_libs::math::Dot::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::math::Dot
impl<T> tracing::instrument::Instrument for vyre_libs::math::Dot
impl<T> tracing::instrument::WithSubscriber for vyre_libs::math::Dot
impl<T> typenum::type_operators::Same for vyre_libs::math::Dot
pub type vyre_libs::math::Dot::Output = T
pub struct vyre_libs::math::linalg::Matmul
impl vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::math::Matmul::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::math::Matmul::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::math::Matmul::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::clone(&self) -> vyre_libs::math::Matmul
impl core::fmt::Debug for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::math::Matmul
impl core::marker::Send for vyre_libs::math::Matmul
impl core::marker::Sync for vyre_libs::math::Matmul
impl core::marker::Unpin for vyre_libs::math::Matmul
impl core::marker::UnsafeUnpin for vyre_libs::math::Matmul
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::math::Matmul
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::math::Matmul
impl<T, U> core::convert::Into<U> for vyre_libs::math::Matmul where U: core::convert::From<T>
pub fn vyre_libs::math::Matmul::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::math::Matmul where U: core::convert::Into<T>
pub type vyre_libs::math::Matmul::Error = core::convert::Infallible
pub fn vyre_libs::math::Matmul::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::math::Matmul where U: core::convert::TryFrom<T>
pub type vyre_libs::math::Matmul::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::math::Matmul::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::math::Matmul where T: core::clone::Clone
pub type vyre_libs::math::Matmul::Owned = T
pub fn vyre_libs::math::Matmul::clone_into(&self, target: &mut T)
pub fn vyre_libs::math::Matmul::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::math::Matmul where T: 'static + ?core::marker::Sized
pub fn vyre_libs::math::Matmul::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::math::Matmul where T: ?core::marker::Sized
pub fn vyre_libs::math::Matmul::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::math::Matmul where T: ?core::marker::Sized
pub fn vyre_libs::math::Matmul::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::math::Matmul where T: core::clone::Clone
pub unsafe fn vyre_libs::math::Matmul::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::math::Matmul
pub type vyre_libs::math::Matmul::Init = T
pub const vyre_libs::math::Matmul::ALIGN: usize
pub unsafe fn vyre_libs::math::Matmul::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::math::Matmul::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::math::Matmul::drop(ptr: usize)
pub unsafe fn vyre_libs::math::Matmul::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::math::Matmul
impl<T> tracing::instrument::Instrument for vyre_libs::math::Matmul
impl<T> tracing::instrument::WithSubscriber for vyre_libs::math::Matmul
impl<T> typenum::type_operators::Same for vyre_libs::math::Matmul
pub type vyre_libs::math::Matmul::Output = T
pub struct vyre_libs::math::linalg::MatmulBias
impl vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBias::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBias::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBias::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::clone(&self) -> vyre_libs::MatmulBias
impl core::fmt::Debug for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBias
impl core::marker::Send for vyre_libs::MatmulBias
impl core::marker::Sync for vyre_libs::MatmulBias
impl core::marker::Unpin for vyre_libs::MatmulBias
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBias
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBias
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBias
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBias where U: core::convert::From<T>
pub fn vyre_libs::MatmulBias::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBias where U: core::convert::Into<T>
pub type vyre_libs::MatmulBias::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBias::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBias where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBias::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBias::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBias where T: core::clone::Clone
pub type vyre_libs::MatmulBias::Owned = T
pub fn vyre_libs::MatmulBias::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBias::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBias where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBias::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBias where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBias::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBias where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBias::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBias where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBias::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBias
pub type vyre_libs::MatmulBias::Init = T
pub const vyre_libs::MatmulBias::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBias::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBias::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBias::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBias::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBias
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBias
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBias
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBias
pub type vyre_libs::MatmulBias::Output = T
pub struct vyre_libs::math::linalg::MatmulBiasTiled
impl vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulBiasTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBiasTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::clone(&self) -> vyre_libs::MatmulBiasTiled
impl core::fmt::Debug for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBiasTiled
impl core::marker::Send for vyre_libs::MatmulBiasTiled
impl core::marker::Sync for vyre_libs::MatmulBiasTiled
impl core::marker::Unpin for vyre_libs::MatmulBiasTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBiasTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBiasTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulBiasTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBiasTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulBiasTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBiasTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBiasTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBiasTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBiasTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub type vyre_libs::MatmulBiasTiled::Owned = T
pub fn vyre_libs::MatmulBiasTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBiasTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBiasTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBiasTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Init = T
pub const vyre_libs::MatmulBiasTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBiasTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBiasTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBiasTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBiasTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBiasTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Output = T
pub struct vyre_libs::math::linalg::MatmulTiled
impl vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::clone(&self) -> vyre_libs::MatmulTiled
impl core::fmt::Debug for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulTiled
impl core::marker::Send for vyre_libs::MatmulTiled
impl core::marker::Sync for vyre_libs::MatmulTiled
impl core::marker::Unpin for vyre_libs::MatmulTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulTiled where T: core::clone::Clone
pub type vyre_libs::MatmulTiled::Owned = T
pub fn vyre_libs::MatmulTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Init = T
pub const vyre_libs::MatmulTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Output = T
pub fn vyre_libs::math::linalg::dot(lhs: &str, rhs: &str, out: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::math::linalg::matmul(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::linalg::matmul_bias(a: &str, b: &str, bias: &str, out: &str, m: u32, k: u32, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::linalg::matmul_bias_tiled(a: &str, b: &str, bias: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::linalg::matmul_strassen_2x2(a: &str, b: &str, c: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::linalg::matmul_strassen_one_level(a: &str, b: &str, c: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::math::linalg::matmul_tiled(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::lzcnt_u32
pub fn vyre_libs::math::lzcnt_u32::lzcnt_u32(input: &str, out: &str, size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::matmul_tiled
pub struct vyre_libs::math::matmul_tiled::MatmulBiasTiled
impl vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulBiasTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBiasTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::clone(&self) -> vyre_libs::MatmulBiasTiled
impl core::fmt::Debug for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBiasTiled
impl core::marker::Send for vyre_libs::MatmulBiasTiled
impl core::marker::Sync for vyre_libs::MatmulBiasTiled
impl core::marker::Unpin for vyre_libs::MatmulBiasTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBiasTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBiasTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulBiasTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBiasTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulBiasTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBiasTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBiasTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBiasTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBiasTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub type vyre_libs::MatmulBiasTiled::Owned = T
pub fn vyre_libs::MatmulBiasTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBiasTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBiasTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBiasTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Init = T
pub const vyre_libs::MatmulBiasTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBiasTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBiasTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBiasTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBiasTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBiasTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Output = T
pub struct vyre_libs::math::matmul_tiled::MatmulTiled
impl vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::clone(&self) -> vyre_libs::MatmulTiled
impl core::fmt::Debug for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulTiled
impl core::marker::Send for vyre_libs::MatmulTiled
impl core::marker::Sync for vyre_libs::MatmulTiled
impl core::marker::Unpin for vyre_libs::MatmulTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulTiled where T: core::clone::Clone
pub type vyre_libs::MatmulTiled::Owned = T
pub fn vyre_libs::MatmulTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Init = T
pub const vyre_libs::MatmulTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Output = T
pub fn vyre_libs::math::matmul_tiled::matmul_bias_tiled(a: &str, b: &str, bias: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::matmul_tiled::matmul_tiled(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::reduce_mean
pub fn vyre_libs::math::reduce_mean::reduce_mean(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::reduce_mean::try_reduce_mean(input: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, &'static str>
pub mod vyre_libs::math::reduce_variance
pub fn vyre_libs::math::reduce_variance::reduce_variance(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::reduce_variance::try_reduce_variance(input: &str, output: &str, n: u32, bessel: bool) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, &'static str>
pub mod vyre_libs::math::scan
pub fn vyre_libs::math::scan::scan_prefix_sum(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::square
pub fn vyre_libs::math::square::square(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::tzcnt_u32
pub fn vyre_libs::math::tzcnt_u32::tzcnt_u32(input: &str, out: &str, size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::weighted_sum
pub fn vyre_libs::math::weighted_sum::weighted_sum_fma_f32(weights: &str, values: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::math::welford
pub fn vyre_libs::math::welford::welford_sum_of_squares(input: &str, sum_out: &str, sum_sq_out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::math::wrapping_neg
pub fn vyre_libs::math::wrapping_neg::wrapping_neg(a: &str, out: &str, size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub struct vyre_libs::math::Dot
impl vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::math::Dot::new(lhs: vyre_libs::tensor_ref::TensorRef, rhs: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::math::Dot::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::math::Dot::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::clone(&self) -> vyre_libs::math::Dot
impl core::fmt::Debug for vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::math::Dot
impl core::marker::Send for vyre_libs::math::Dot
impl core::marker::Sync for vyre_libs::math::Dot
impl core::marker::Unpin for vyre_libs::math::Dot
impl core::marker::UnsafeUnpin for vyre_libs::math::Dot
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::math::Dot
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::math::Dot
impl<T, U> core::convert::Into<U> for vyre_libs::math::Dot where U: core::convert::From<T>
pub fn vyre_libs::math::Dot::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::math::Dot where U: core::convert::Into<T>
pub type vyre_libs::math::Dot::Error = core::convert::Infallible
pub fn vyre_libs::math::Dot::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::math::Dot where U: core::convert::TryFrom<T>
pub type vyre_libs::math::Dot::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::math::Dot::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::math::Dot where T: core::clone::Clone
pub type vyre_libs::math::Dot::Owned = T
pub fn vyre_libs::math::Dot::clone_into(&self, target: &mut T)
pub fn vyre_libs::math::Dot::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::math::Dot where T: 'static + ?core::marker::Sized
pub fn vyre_libs::math::Dot::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::math::Dot where T: ?core::marker::Sized
pub fn vyre_libs::math::Dot::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::math::Dot where T: ?core::marker::Sized
pub fn vyre_libs::math::Dot::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::math::Dot where T: core::clone::Clone
pub unsafe fn vyre_libs::math::Dot::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::math::Dot
pub fn vyre_libs::math::Dot::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::math::Dot
pub type vyre_libs::math::Dot::Init = T
pub const vyre_libs::math::Dot::ALIGN: usize
pub unsafe fn vyre_libs::math::Dot::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::math::Dot::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::math::Dot::drop(ptr: usize)
pub unsafe fn vyre_libs::math::Dot::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::math::Dot
impl<T> tracing::instrument::Instrument for vyre_libs::math::Dot
impl<T> tracing::instrument::WithSubscriber for vyre_libs::math::Dot
impl<T> typenum::type_operators::Same for vyre_libs::math::Dot
pub type vyre_libs::math::Dot::Output = T
pub struct vyre_libs::math::Matmul
impl vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::math::Matmul::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::math::Matmul::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::math::Matmul::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::clone(&self) -> vyre_libs::math::Matmul
impl core::fmt::Debug for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::math::Matmul
impl core::marker::Send for vyre_libs::math::Matmul
impl core::marker::Sync for vyre_libs::math::Matmul
impl core::marker::Unpin for vyre_libs::math::Matmul
impl core::marker::UnsafeUnpin for vyre_libs::math::Matmul
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::math::Matmul
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::math::Matmul
impl<T, U> core::convert::Into<U> for vyre_libs::math::Matmul where U: core::convert::From<T>
pub fn vyre_libs::math::Matmul::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::math::Matmul where U: core::convert::Into<T>
pub type vyre_libs::math::Matmul::Error = core::convert::Infallible
pub fn vyre_libs::math::Matmul::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::math::Matmul where U: core::convert::TryFrom<T>
pub type vyre_libs::math::Matmul::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::math::Matmul::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::math::Matmul where T: core::clone::Clone
pub type vyre_libs::math::Matmul::Owned = T
pub fn vyre_libs::math::Matmul::clone_into(&self, target: &mut T)
pub fn vyre_libs::math::Matmul::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::math::Matmul where T: 'static + ?core::marker::Sized
pub fn vyre_libs::math::Matmul::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::math::Matmul where T: ?core::marker::Sized
pub fn vyre_libs::math::Matmul::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::math::Matmul where T: ?core::marker::Sized
pub fn vyre_libs::math::Matmul::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::math::Matmul where T: core::clone::Clone
pub unsafe fn vyre_libs::math::Matmul::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::math::Matmul
pub type vyre_libs::math::Matmul::Init = T
pub const vyre_libs::math::Matmul::ALIGN: usize
pub unsafe fn vyre_libs::math::Matmul::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::math::Matmul::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::math::Matmul::drop(ptr: usize)
pub unsafe fn vyre_libs::math::Matmul::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::math::Matmul
impl<T> tracing::instrument::Instrument for vyre_libs::math::Matmul
impl<T> tracing::instrument::WithSubscriber for vyre_libs::math::Matmul
impl<T> typenum::type_operators::Same for vyre_libs::math::Matmul
pub type vyre_libs::math::Matmul::Output = T
pub struct vyre_libs::math::MatmulBias
impl vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBias::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBias::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBias::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::clone(&self) -> vyre_libs::MatmulBias
impl core::fmt::Debug for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBias
impl core::marker::Send for vyre_libs::MatmulBias
impl core::marker::Sync for vyre_libs::MatmulBias
impl core::marker::Unpin for vyre_libs::MatmulBias
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBias
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBias
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBias
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBias where U: core::convert::From<T>
pub fn vyre_libs::MatmulBias::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBias where U: core::convert::Into<T>
pub type vyre_libs::MatmulBias::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBias::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBias where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBias::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBias::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBias where T: core::clone::Clone
pub type vyre_libs::MatmulBias::Owned = T
pub fn vyre_libs::MatmulBias::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBias::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBias where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBias::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBias where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBias::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBias where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBias::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBias where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBias::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBias
pub type vyre_libs::MatmulBias::Init = T
pub const vyre_libs::MatmulBias::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBias::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBias::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBias::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBias::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBias
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBias
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBias
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBias
pub type vyre_libs::MatmulBias::Output = T
pub struct vyre_libs::math::MatmulBiasTiled
impl vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulBiasTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBiasTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::clone(&self) -> vyre_libs::MatmulBiasTiled
impl core::fmt::Debug for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBiasTiled
impl core::marker::Send for vyre_libs::MatmulBiasTiled
impl core::marker::Sync for vyre_libs::MatmulBiasTiled
impl core::marker::Unpin for vyre_libs::MatmulBiasTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBiasTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBiasTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulBiasTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBiasTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulBiasTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBiasTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBiasTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBiasTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBiasTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub type vyre_libs::MatmulBiasTiled::Owned = T
pub fn vyre_libs::MatmulBiasTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBiasTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBiasTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBiasTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Init = T
pub const vyre_libs::MatmulBiasTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBiasTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBiasTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBiasTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBiasTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBiasTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Output = T
pub struct vyre_libs::math::MatmulTiled
impl vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::clone(&self) -> vyre_libs::MatmulTiled
impl core::fmt::Debug for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulTiled
impl core::marker::Send for vyre_libs::MatmulTiled
impl core::marker::Sync for vyre_libs::MatmulTiled
impl core::marker::Unpin for vyre_libs::MatmulTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulTiled where T: core::clone::Clone
pub type vyre_libs::MatmulTiled::Owned = T
pub fn vyre_libs::MatmulTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Init = T
pub const vyre_libs::MatmulTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Output = T
pub fn vyre_libs::math::atomic_add_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic_and_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic_compare_exchange_u32(expected: &str, desired: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic_exchange_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic_max_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic_min_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic_or_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::atomic_xor_u32(values: &str, state: &str, trace: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::broadcast(src: &str, dst: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::clamp_u32(input: &str, lo: &str, hi: &str, out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::dot(lhs: &str, rhs: &str, out: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::math::lzcnt_u32(input: &str, out: &str, size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::matmul(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::matmul_bias(a: &str, b: &str, bias: &str, out: &str, m: u32, k: u32, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::matmul_bias_tiled(a: &str, b: &str, bias: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::matmul_tiled(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::reduce_mean(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::reduce_variance(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::scan_prefix_sum(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::square(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::tzcnt_u32(input: &str, out: &str, size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::math::welford_sum_of_squares(input: &str, sum_out: &str, sum_sq_out: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::matmul_tiled
pub struct vyre_libs::matmul_tiled::MatmulBiasTiled
impl vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulBiasTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBiasTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::clone(&self) -> vyre_libs::MatmulBiasTiled
impl core::fmt::Debug for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBiasTiled
impl core::marker::Send for vyre_libs::MatmulBiasTiled
impl core::marker::Sync for vyre_libs::MatmulBiasTiled
impl core::marker::Unpin for vyre_libs::MatmulBiasTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBiasTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBiasTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulBiasTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBiasTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulBiasTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBiasTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBiasTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBiasTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBiasTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub type vyre_libs::MatmulBiasTiled::Owned = T
pub fn vyre_libs::MatmulBiasTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBiasTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBiasTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBiasTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Init = T
pub const vyre_libs::MatmulBiasTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBiasTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBiasTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBiasTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBiasTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBiasTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Output = T
pub struct vyre_libs::matmul_tiled::MatmulTiled
impl vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::clone(&self) -> vyre_libs::MatmulTiled
impl core::fmt::Debug for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulTiled
impl core::marker::Send for vyre_libs::MatmulTiled
impl core::marker::Sync for vyre_libs::MatmulTiled
impl core::marker::Unpin for vyre_libs::MatmulTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulTiled where T: core::clone::Clone
pub type vyre_libs::MatmulTiled::Owned = T
pub fn vyre_libs::MatmulTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Init = T
pub const vyre_libs::MatmulTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Output = T
pub fn vyre_libs::matmul_tiled::matmul_bias_tiled(a: &str, b: &str, bias: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matmul_tiled::matmul_tiled(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn
pub mod vyre_libs::nn::activation
pub mod vyre_libs::nn::activation::cross_entropy
pub fn vyre_libs::nn::activation::cross_entropy::cross_entropy(logits: &str, targets: &str, loss_out: &str, n: u32, vocab_size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::cross_entropy::try_cross_entropy(logits: &str, targets: &str, loss_out: &str, n: u32, vocab_size: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub mod vyre_libs::nn::activation::embedding
pub fn vyre_libs::nn::activation::embedding::embedding(embed_table: &str, tokens: &str, output: &str, n: u32, embed_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::activation::gelu
pub fn vyre_libs::nn::activation::gelu::gelu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::activation::leaky_relu_sq
pub fn vyre_libs::nn::activation::leaky_relu_sq::leaky_relu_sq(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::activation::logit_softcap
pub fn vyre_libs::nn::activation::logit_softcap::logit_softcap(input: &str, output: &str, n: u32, cap: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::activation::mlp_4x_leaky_sq
pub fn vyre_libs::nn::activation::mlp_4x_leaky_sq::mlp_4x_leaky_sq(x: &str, w1: &str, b1: &str, w2: &str, b2: &str, output: &str, model_dim: u32, hidden_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::nn::activation::parallel_residual_block
pub fn vyre_libs::nn::activation::parallel_residual_block::parallel_residual_block(x: &str, attn_out: &str, mlp_out: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::nn::activation::relu
pub fn vyre_libs::nn::activation::relu::relu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::activation::silu
pub fn vyre_libs::nn::activation::silu::silu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::activation::skip_gate
pub fn vyre_libs::nn::activation::skip_gate::skip_gate(gate: &str, branch: &str, skip: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::activation::swiglu
pub fn vyre_libs::nn::activation::swiglu::swiglu(gate: &str, up: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::cross_entropy(logits: &str, targets: &str, loss_out: &str, n: u32, vocab_size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::embedding(embed_table: &str, tokens: &str, output: &str, n: u32, embed_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::gelu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::leaky_relu_sq(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::logit_softcap(input: &str, output: &str, n: u32, cap: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::mlp_4x_leaky_sq(x: &str, w1: &str, b1: &str, w2: &str, b2: &str, output: &str, model_dim: u32, hidden_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::activation::parallel_residual_block(x: &str, attn_out: &str, mlp_out: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::activation::relu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::silu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::skip_gate(gate: &str, branch: &str, skip: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::swiglu(gate: &str, up: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::activation::try_cross_entropy(logits: &str, targets: &str, loss_out: &str, n: u32, vocab_size: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub mod vyre_libs::nn::backward
pub mod vyre_libs::nn::backward::leaky_relu_sq_backward
pub fn vyre_libs::nn::backward::leaky_relu_sq_backward::leaky_relu_sq_backward(input: &str, grad_out: &str, grad_in: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::backward::ln_scale_backward
pub fn vyre_libs::nn::backward::ln_scale_backward::ln_scale_backward(input: &str, scale: &str, grad_out: &str, grad_x: &str, grad_scale: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::backward::logit_softcap_backward
pub fn vyre_libs::nn::backward::logit_softcap_backward::logit_softcap_backward(input: &str, grad_out: &str, grad_in: &str, n: u32, cap: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::backward::mlp_backward
pub fn vyre_libs::nn::backward::mlp_backward::mlp_backward(x: &str, w1: &str, b1: &str, w2: &str, grad_out: &str, grad_x: &str, model_dim: u32, hidden_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::backward::partial_rope_backward
pub fn vyre_libs::nn::backward::partial_rope_backward::partial_rope_backward(grad_out: &str, cos_table: &str, sin_table: &str, grad_in: &str, num_heads: u32, seq_len: u32, head_dim: u32, rope_dims: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::backward::qk_gain_backward
pub fn vyre_libs::nn::backward::qk_gain_backward::qk_gain_backward(gain: &str, grad_out: &str, grad_q: &str, num_heads: u32, seq_len: u32, head_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::backward::residual_block_backward
pub fn vyre_libs::nn::backward::residual_block_backward::residual_block_backward(grad_out: &str, grad_x: &str, grad_attn: &str, grad_mlp: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::backward::skip_gate_backward
pub fn vyre_libs::nn::backward::skip_gate_backward::skip_gate_backward(gate: &str, branch: &str, skip: &str, grad_out: &str, grad_gate: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::backward::leaky_relu_sq_backward(input: &str, grad_out: &str, grad_in: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::backward::ln_scale_backward(input: &str, scale: &str, grad_out: &str, grad_x: &str, grad_scale: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::backward::logit_softcap_backward(input: &str, grad_out: &str, grad_in: &str, n: u32, cap: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::backward::mlp_backward(x: &str, w1: &str, b1: &str, w2: &str, grad_out: &str, grad_x: &str, model_dim: u32, hidden_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::backward::partial_rope_backward(grad_out: &str, cos_table: &str, sin_table: &str, grad_in: &str, num_heads: u32, seq_len: u32, head_dim: u32, rope_dims: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::backward::qk_gain_backward(gain: &str, grad_out: &str, grad_q: &str, num_heads: u32, seq_len: u32, head_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::backward::residual_block_backward(grad_out: &str, grad_x: &str, grad_attn: &str, grad_mlp: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::backward::skip_gate_backward(gate: &str, branch: &str, skip: &str, grad_out: &str, grad_gate: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::leaky_relu_sq_backward
pub fn vyre_libs::nn::leaky_relu_sq_backward::leaky_relu_sq_backward(input: &str, grad_out: &str, grad_in: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::linear
pub struct vyre_libs::nn::linear::Linear
impl vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::nn::Linear::new(x: vyre_libs::tensor_ref::TensorRef, w: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::nn::Linear::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::nn::Linear::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::clone(&self) -> vyre_libs::nn::Linear
impl core::fmt::Debug for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::nn::Linear
impl core::marker::Send for vyre_libs::nn::Linear
impl core::marker::Sync for vyre_libs::nn::Linear
impl core::marker::Unpin for vyre_libs::nn::Linear
impl core::marker::UnsafeUnpin for vyre_libs::nn::Linear
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::nn::Linear
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::nn::Linear
impl<T, U> core::convert::Into<U> for vyre_libs::nn::Linear where U: core::convert::From<T>
pub fn vyre_libs::nn::Linear::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::nn::Linear where U: core::convert::Into<T>
pub type vyre_libs::nn::Linear::Error = core::convert::Infallible
pub fn vyre_libs::nn::Linear::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::nn::Linear where U: core::convert::TryFrom<T>
pub type vyre_libs::nn::Linear::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::nn::Linear::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::nn::Linear where T: core::clone::Clone
pub type vyre_libs::nn::Linear::Owned = T
pub fn vyre_libs::nn::Linear::clone_into(&self, target: &mut T)
pub fn vyre_libs::nn::Linear::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::nn::Linear where T: 'static + ?core::marker::Sized
pub fn vyre_libs::nn::Linear::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::nn::Linear where T: ?core::marker::Sized
pub fn vyre_libs::nn::Linear::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::nn::Linear where T: ?core::marker::Sized
pub fn vyre_libs::nn::Linear::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::nn::Linear where T: core::clone::Clone
pub unsafe fn vyre_libs::nn::Linear::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::nn::Linear
pub type vyre_libs::nn::Linear::Init = T
pub const vyre_libs::nn::Linear::ALIGN: usize
pub unsafe fn vyre_libs::nn::Linear::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::nn::Linear::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::nn::Linear::drop(ptr: usize)
pub unsafe fn vyre_libs::nn::Linear::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::nn::Linear
impl<T> tracing::instrument::Instrument for vyre_libs::nn::Linear
impl<T> tracing::instrument::WithSubscriber for vyre_libs::nn::Linear
impl<T> typenum::type_operators::Same for vyre_libs::nn::Linear
pub type vyre_libs::nn::Linear::Output = T
pub fn vyre_libs::nn::linear::batch_matmul(a: &str, b: &str, out: &str, batch: u32, m: u32, k: u32, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear::linear(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear::linear_relu(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear::linear_silu(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear::linear_tiled(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32, tile: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear::linear_tiled_reference(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32, tile: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear::rms_norm_linear(input: &str, w: &str, b: &str, out: &str, n: u32, in_dim: u32, out_dim: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::linear::try_rms_norm_linear(input: &str, w: &str, b: &str, out: &str, n: u32, in_dim: u32, out_dim: u32, eps: f32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub mod vyre_libs::nn::ln_scale_backward
pub fn vyre_libs::nn::ln_scale_backward::ln_scale_backward(input: &str, scale: &str, grad_out: &str, grad_x: &str, grad_scale: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::logit_softcap_backward
pub fn vyre_libs::nn::logit_softcap_backward::logit_softcap_backward(input: &str, grad_out: &str, grad_in: &str, n: u32, cap: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::mlp_backward
pub fn vyre_libs::nn::mlp_backward::mlp_backward(x: &str, w1: &str, b1: &str, w2: &str, grad_out: &str, grad_x: &str, model_dim: u32, hidden_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::norm
pub mod vyre_libs::nn::norm::layerwise_ln_scale
pub fn vyre_libs::nn::norm::layerwise_ln_scale::layerwise_ln_scale(input: &str, scale: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub struct vyre_libs::nn::norm::LayerNorm
impl vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::nn::LayerNorm::new(input: vyre_libs::tensor_ref::TensorRef, output: vyre_libs::tensor_ref::TensorRef, eps: f32) -> Self
impl vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::nn::LayerNorm::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::nn::LayerNorm::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::clone(&self) -> vyre_libs::nn::LayerNorm
impl core::fmt::Debug for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::nn::LayerNorm
impl core::marker::Send for vyre_libs::nn::LayerNorm
impl core::marker::Sync for vyre_libs::nn::LayerNorm
impl core::marker::Unpin for vyre_libs::nn::LayerNorm
impl core::marker::UnsafeUnpin for vyre_libs::nn::LayerNorm
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::nn::LayerNorm
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::nn::LayerNorm
impl<T, U> core::convert::Into<U> for vyre_libs::nn::LayerNorm where U: core::convert::From<T>
pub fn vyre_libs::nn::LayerNorm::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::nn::LayerNorm where U: core::convert::Into<T>
pub type vyre_libs::nn::LayerNorm::Error = core::convert::Infallible
pub fn vyre_libs::nn::LayerNorm::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::nn::LayerNorm where U: core::convert::TryFrom<T>
pub type vyre_libs::nn::LayerNorm::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::nn::LayerNorm::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::nn::LayerNorm where T: core::clone::Clone
pub type vyre_libs::nn::LayerNorm::Owned = T
pub fn vyre_libs::nn::LayerNorm::clone_into(&self, target: &mut T)
pub fn vyre_libs::nn::LayerNorm::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::nn::LayerNorm where T: 'static + ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::nn::LayerNorm where T: ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::nn::LayerNorm where T: ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::nn::LayerNorm where T: core::clone::Clone
pub unsafe fn vyre_libs::nn::LayerNorm::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::nn::LayerNorm
pub type vyre_libs::nn::LayerNorm::Init = T
pub const vyre_libs::nn::LayerNorm::ALIGN: usize
pub unsafe fn vyre_libs::nn::LayerNorm::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::nn::LayerNorm::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::nn::LayerNorm::drop(ptr: usize)
pub unsafe fn vyre_libs::nn::LayerNorm::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::nn::LayerNorm
impl<T> tracing::instrument::Instrument for vyre_libs::nn::LayerNorm
impl<T> tracing::instrument::WithSubscriber for vyre_libs::nn::LayerNorm
impl<T> typenum::type_operators::Same for vyre_libs::nn::LayerNorm
pub type vyre_libs::nn::LayerNorm::Output = T
pub fn vyre_libs::nn::norm::layer_norm(input: &str, output: &str, n: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::norm::layerwise_ln_scale(input: &str, scale: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::norm::rms_norm(input: &str, output: &str, n: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::norm::rms_norm_reference(input: &str, output: &str, n: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::optim
pub mod vyre_libs::nn::optim::adamw_step
pub fn vyre_libs::nn::optim::adamw_step::adamw_step(params: &str, grads: &str, m_buf: &str, v_buf: &str, n: u32, lr: f32, beta1: f32, beta2: f32, eps: f32, wd: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::optim::ema_apply
pub fn vyre_libs::nn::optim::ema_apply::ema_apply(ema: &str, theta: &str, n: u32, decay: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::optim::muon_update
pub fn vyre_libs::nn::optim::muon_update::muon_update(params: &str, grads: &str, momentum_buf: &str, output: &str, n: u32, lr: f32, momentum: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::optim::muoneq_r
pub fn vyre_libs::nn::optim::muoneq_r::muoneq_r(params: &str, grads: &str, momentum_buf: &str, output: &str, n: u32, rows: u32, cols: u32, lr: f32, momentum: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::optim::newton_schulz
pub fn vyre_libs::nn::optim::newton_schulz::newton_schulz_5step(mat: &str, output: &str, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::optim::adamw_step(params: &str, grads: &str, m_buf: &str, v_buf: &str, n: u32, lr: f32, beta1: f32, beta2: f32, eps: f32, wd: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::optim::ema_apply(ema: &str, theta: &str, n: u32, decay: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::optim::muon_update(params: &str, grads: &str, momentum_buf: &str, output: &str, n: u32, lr: f32, momentum: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::optim::muoneq_r(params: &str, grads: &str, momentum_buf: &str, output: &str, n: u32, rows: u32, cols: u32, lr: f32, momentum: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::optim::newton_schulz_5step(mat: &str, output: &str, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::partial_rope_backward
pub fn vyre_libs::nn::partial_rope_backward::partial_rope_backward(grad_out: &str, cos_table: &str, sin_table: &str, grad_in: &str, num_heads: u32, seq_len: u32, head_dim: u32, rope_dims: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::qk_gain_backward
pub fn vyre_libs::nn::qk_gain_backward::qk_gain_backward(gain: &str, grad_out: &str, grad_q: &str, num_heads: u32, seq_len: u32, head_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::quant
pub mod vyre_libs::nn::quant::byte_shuffle
pub fn vyre_libs::nn::quant::byte_shuffle::byte_shuffle(input: &str, output: &str, n: u32, elem_bytes: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::nn::quant::ggml
pub const vyre_libs::nn::quant::ggml::Q2_K_BLOCKS_PER_SUPER: u32
pub const vyre_libs::nn::quant::ggml::Q2_K_BLOCK_SIZE: u32
pub const vyre_libs::nn::quant::ggml::Q2_K_SUPER_BLOCK_SIZE: u32
pub const vyre_libs::nn::quant::ggml::Q4_K_BLOCKS_PER_SUPER: u32
pub const vyre_libs::nn::quant::ggml::Q4_K_BLOCK_SIZE: u32
pub const vyre_libs::nn::quant::ggml::Q4_K_SUPER_BLOCK_SIZE: u32
pub fn vyre_libs::nn::quant::ggml::q2_k_linear(x: &str, w_packed: &str, w_scales: &str, w_mins: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::quant::ggml::q2_k_unpack(packed: &str, scales: &str, mins: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::quant::ggml::q4_k_linear(x: &str, w_packed: &str, w_scales: &str, w_mins: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::quant::ggml::q4_k_unpack(packed: &str, scales: &str, mins: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::nn::quant::gptq
pub fn vyre_libs::nn::quant::gptq::gptq_round(input: &str, scale: &str, output: &str, n: u32, max_val: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::gptq::gptq_sdclip(input: &str, output: &str, n: u32, k: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::quant::int4
pub const vyre_libs::nn::quant::int4::INT4_BATCHED_MATMUL_SCALED_EXTENSION_NAME: &str
pub const vyre_libs::nn::quant::int4::INT4_BATCHED_MATMUL_TOP1_SCALED_EXTENSION_NAME: &str
pub const vyre_libs::nn::quant::int4::INT4_BATCHED_MATVEC_SCALED_EXTENSION_NAME: &str
pub const vyre_libs::nn::quant::int4::INT4_DOT_EXTENSION_NAME: &str
pub const vyre_libs::nn::quant::int4::INT4_DOT_SCALED_EXTENSION_NAME: &str
pub const vyre_libs::nn::quant::int4::INT4_MATVEC_SCALED_EXTENSION_NAME: &str
pub fn vyre_libs::nn::quant::int4::int4_batched_matmul_f32_scaled(weights_packed: &str, activation_batches_packed: &str, row_scales: &str, batch_scales: &str, out: &str, batch: u32, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4::int4_batched_matmul_scaled_extension_id() -> vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_libs::nn::quant::int4::int4_batched_matmul_top1_f32_scaled(weights_packed: &str, activation_batches_packed: &str, row_scales: &str, batch_scales: &str, out: &str, batch: u32, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4::int4_batched_matmul_top1_scaled_extension_id() -> vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_libs::nn::quant::int4::int4_batched_matvec_f32_scaled(weights_packed: &str, x_batches: &str, row_scales: &str, out: &str, batch: u32, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4::int4_batched_matvec_scaled_extension_id() -> vyre_spec::extension::ExtensionTernaryOpId
pub fn vyre_libs::nn::quant::int4::int4_dot_extension_id() -> vyre_spec::extension::ExtensionBinOpId
pub fn vyre_libs::nn::quant::int4::int4_dot_f32_scaled(lhs_packed: &str, rhs_packed: &str, lhs_scale: &str, rhs_scale: &str, out: &str, lane_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4::int4_dot_i32(lhs_packed: &str, rhs_packed: &str, out: &str, lane_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4::int4_dot_scaled_extension_id() -> vyre_spec::extension::ExtensionBinOpId
pub fn vyre_libs::nn::quant::int4::int4_matvec_f32_scaled(weights_packed: &str, x: &str, row_scales: &str, out: &str, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4::int4_matvec_scaled_extension_id() -> vyre_spec::extension::ExtensionTernaryOpId
pub mod vyre_libs::nn::quant::int6
pub fn vyre_libs::nn::quant::int6::int6_pack(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int6::int6_unpack(packed: &str, scale: &str, zero: &str, output: &str, n: u32, block_size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::quant::int8
pub fn vyre_libs::nn::quant::int8::int8_pack(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int8::int8_unpack(packed: &str, scales: &str, output: &str, n: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub const vyre_libs::nn::quant::Q2_K_BLOCKS_PER_SUPER: u32
pub const vyre_libs::nn::quant::Q2_K_BLOCK_SIZE: u32
pub const vyre_libs::nn::quant::Q2_K_SUPER_BLOCK_SIZE: u32
pub const vyre_libs::nn::quant::Q4_K_BLOCKS_PER_SUPER: u32
pub const vyre_libs::nn::quant::Q4_K_BLOCK_SIZE: u32
pub const vyre_libs::nn::quant::Q4_K_SUPER_BLOCK_SIZE: u32
pub fn vyre_libs::nn::quant::byte_shuffle(input: &str, output: &str, n: u32, elem_bytes: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::quant::gptq_round(input: &str, scale: &str, output: &str, n: u32, max_val: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::gptq_sdclip(input: &str, output: &str, n: u32, k: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4_batched_matmul_f32_scaled(weights_packed: &str, activation_batches_packed: &str, row_scales: &str, batch_scales: &str, out: &str, batch: u32, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4_batched_matmul_top1_f32_scaled(weights_packed: &str, activation_batches_packed: &str, row_scales: &str, batch_scales: &str, out: &str, batch: u32, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4_batched_matvec_f32_scaled(weights_packed: &str, x_batches: &str, row_scales: &str, out: &str, batch: u32, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4_dot_f32_scaled(lhs_packed: &str, rhs_packed: &str, lhs_scale: &str, rhs_scale: &str, out: &str, lane_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4_dot_i32(lhs_packed: &str, rhs_packed: &str, out: &str, lane_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int4_matvec_f32_scaled(weights_packed: &str, x: &str, row_scales: &str, out: &str, rows: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int6_pack(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int6_unpack(packed: &str, scale: &str, zero: &str, output: &str, n: u32, block_size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int8_pack(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::int8_unpack(packed: &str, scales: &str, output: &str, n: u32, cols: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::quant::q2_k_linear(x: &str, w_packed: &str, w_scales: &str, w_mins: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::quant::q2_k_unpack(packed: &str, scales: &str, mins: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::quant::q4_k_linear(x: &str, w_packed: &str, w_scales: &str, w_mins: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::quant::q4_k_unpack(packed: &str, scales: &str, mins: &str, output: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::nn::relu
pub fn vyre_libs::nn::relu::relu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::residual_block_backward
pub fn vyre_libs::nn::residual_block_backward::residual_block_backward(grad_out: &str, grad_x: &str, grad_attn: &str, grad_mlp: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::nn::skip_gate_backward
pub fn vyre_libs::nn::skip_gate_backward::skip_gate_backward(gate: &str, branch: &str, skip: &str, grad_out: &str, grad_gate: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub struct vyre_libs::nn::LayerNorm
impl vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::nn::LayerNorm::new(input: vyre_libs::tensor_ref::TensorRef, output: vyre_libs::tensor_ref::TensorRef, eps: f32) -> Self
impl vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::nn::LayerNorm::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::nn::LayerNorm::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::clone(&self) -> vyre_libs::nn::LayerNorm
impl core::fmt::Debug for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::nn::LayerNorm
impl core::marker::Send for vyre_libs::nn::LayerNorm
impl core::marker::Sync for vyre_libs::nn::LayerNorm
impl core::marker::Unpin for vyre_libs::nn::LayerNorm
impl core::marker::UnsafeUnpin for vyre_libs::nn::LayerNorm
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::nn::LayerNorm
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::nn::LayerNorm
impl<T, U> core::convert::Into<U> for vyre_libs::nn::LayerNorm where U: core::convert::From<T>
pub fn vyre_libs::nn::LayerNorm::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::nn::LayerNorm where U: core::convert::Into<T>
pub type vyre_libs::nn::LayerNorm::Error = core::convert::Infallible
pub fn vyre_libs::nn::LayerNorm::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::nn::LayerNorm where U: core::convert::TryFrom<T>
pub type vyre_libs::nn::LayerNorm::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::nn::LayerNorm::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::nn::LayerNorm where T: core::clone::Clone
pub type vyre_libs::nn::LayerNorm::Owned = T
pub fn vyre_libs::nn::LayerNorm::clone_into(&self, target: &mut T)
pub fn vyre_libs::nn::LayerNorm::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::nn::LayerNorm where T: 'static + ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::nn::LayerNorm where T: ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::nn::LayerNorm where T: ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::nn::LayerNorm where T: core::clone::Clone
pub unsafe fn vyre_libs::nn::LayerNorm::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::nn::LayerNorm
pub type vyre_libs::nn::LayerNorm::Init = T
pub const vyre_libs::nn::LayerNorm::ALIGN: usize
pub unsafe fn vyre_libs::nn::LayerNorm::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::nn::LayerNorm::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::nn::LayerNorm::drop(ptr: usize)
pub unsafe fn vyre_libs::nn::LayerNorm::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::nn::LayerNorm
impl<T> tracing::instrument::Instrument for vyre_libs::nn::LayerNorm
impl<T> tracing::instrument::WithSubscriber for vyre_libs::nn::LayerNorm
impl<T> typenum::type_operators::Same for vyre_libs::nn::LayerNorm
pub type vyre_libs::nn::LayerNorm::Output = T
pub struct vyre_libs::nn::Linear
impl vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::nn::Linear::new(x: vyre_libs::tensor_ref::TensorRef, w: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::nn::Linear::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::nn::Linear::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::clone(&self) -> vyre_libs::nn::Linear
impl core::fmt::Debug for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::nn::Linear
impl core::marker::Send for vyre_libs::nn::Linear
impl core::marker::Sync for vyre_libs::nn::Linear
impl core::marker::Unpin for vyre_libs::nn::Linear
impl core::marker::UnsafeUnpin for vyre_libs::nn::Linear
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::nn::Linear
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::nn::Linear
impl<T, U> core::convert::Into<U> for vyre_libs::nn::Linear where U: core::convert::From<T>
pub fn vyre_libs::nn::Linear::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::nn::Linear where U: core::convert::Into<T>
pub type vyre_libs::nn::Linear::Error = core::convert::Infallible
pub fn vyre_libs::nn::Linear::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::nn::Linear where U: core::convert::TryFrom<T>
pub type vyre_libs::nn::Linear::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::nn::Linear::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::nn::Linear where T: core::clone::Clone
pub type vyre_libs::nn::Linear::Owned = T
pub fn vyre_libs::nn::Linear::clone_into(&self, target: &mut T)
pub fn vyre_libs::nn::Linear::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::nn::Linear where T: 'static + ?core::marker::Sized
pub fn vyre_libs::nn::Linear::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::nn::Linear where T: ?core::marker::Sized
pub fn vyre_libs::nn::Linear::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::nn::Linear where T: ?core::marker::Sized
pub fn vyre_libs::nn::Linear::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::nn::Linear where T: core::clone::Clone
pub unsafe fn vyre_libs::nn::Linear::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::nn::Linear
pub type vyre_libs::nn::Linear::Init = T
pub const vyre_libs::nn::Linear::ALIGN: usize
pub unsafe fn vyre_libs::nn::Linear::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::nn::Linear::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::nn::Linear::drop(ptr: usize)
pub unsafe fn vyre_libs::nn::Linear::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::nn::Linear
impl<T> tracing::instrument::Instrument for vyre_libs::nn::Linear
impl<T> tracing::instrument::WithSubscriber for vyre_libs::nn::Linear
impl<T> typenum::type_operators::Same for vyre_libs::nn::Linear
pub type vyre_libs::nn::Linear::Output = T
pub fn vyre_libs::nn::layer_norm(input: &str, output: &str, n: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::leaky_relu_sq_backward(input: &str, grad_out: &str, grad_in: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::linear(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear_relu(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear_silu(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::linear_tiled(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32, tile: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::nn::ln_scale_backward(input: &str, scale: &str, grad_out: &str, grad_x: &str, grad_scale: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::logit_softcap_backward(input: &str, grad_out: &str, grad_in: &str, n: u32, cap: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::mlp_backward(x: &str, w1: &str, b1: &str, w2: &str, grad_out: &str, grad_x: &str, model_dim: u32, hidden_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::partial_rope_backward(grad_out: &str, cos_table: &str, sin_table: &str, grad_in: &str, num_heads: u32, seq_len: u32, head_dim: u32, rope_dims: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::qk_gain_backward(gain: &str, grad_out: &str, grad_q: &str, num_heads: u32, seq_len: u32, head_dim: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::relu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::residual_block_backward(grad_out: &str, grad_x: &str, grad_attn: &str, grad_mlp: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::rms_norm(input: &str, output: &str, n: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::rms_norm_reference(input: &str, output: &str, n: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::nn::skip_gate_backward(gate: &str, branch: &str, skip: &str, grad_out: &str, grad_gate: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::observability
pub use vyre_libs::observability::BackendObservabilityProvider
pub use vyre_libs::observability::DriverObservability
pub mod vyre_libs::parsing
pub mod vyre_libs::parsing::core
pub mod vyre_libs::parsing::core::ast
pub mod vyre_libs::parsing::core::ast::node
pub const vyre_libs::parsing::core::ast::node::AST_ADD: u32
pub const vyre_libs::parsing::core::ast::node::AST_ASSIGN: u32
pub const vyre_libs::parsing::core::ast::node::AST_CALL: u32
pub const vyre_libs::parsing::core::ast::node::AST_CAST: u32
pub const vyre_libs::parsing::core::ast::node::AST_CONST_INT: u32
pub const vyre_libs::parsing::core::ast::node::AST_DIV: u32
pub const vyre_libs::parsing::core::ast::node::AST_EQ: u32
pub const vyre_libs::parsing::core::ast::node::AST_GE: u32
pub const vyre_libs::parsing::core::ast::node::AST_GT: u32
pub const vyre_libs::parsing::core::ast::node::AST_IF: u32
pub const vyre_libs::parsing::core::ast::node::AST_LE: u32
pub const vyre_libs::parsing::core::ast::node::AST_LOGICAL_AND: u32
pub const vyre_libs::parsing::core::ast::node::AST_LOGICAL_OR: u32
pub const vyre_libs::parsing::core::ast::node::AST_LT: u32
pub const vyre_libs::parsing::core::ast::node::AST_MOD: u32
pub const vyre_libs::parsing::core::ast::node::AST_MUL: u32
pub const vyre_libs::parsing::core::ast::node::AST_NE: u32
pub const vyre_libs::parsing::core::ast::node::AST_PTR_DEREF: u32
pub const vyre_libs::parsing::core::ast::node::AST_RET: u32
pub const vyre_libs::parsing::core::ast::node::AST_SUB: u32
pub const vyre_libs::parsing::core::ast::node::AST_VAR: u32
pub mod vyre_libs::parsing::core::delimiter
pub use vyre_libs::parsing::core::delimiter::CLOSE_BRACE
pub use vyre_libs::parsing::core::delimiter::MATCH_NONE
pub use vyre_libs::parsing::core::delimiter::OPEN_BRACE
pub use vyre_libs::parsing::core::delimiter::OTHER
pub use vyre_libs::parsing::core::delimiter::cpu_ref
pub use vyre_libs::parsing::core::delimiter::pack_u32
pub const vyre_libs::parsing::core::delimiter::CORE_DELIMITER_OP_ID: &str
pub const vyre_libs::parsing::core::delimiter::OP_ID: &str
pub fn vyre_libs::parsing::core::delimiter::bracket_match(kinds: &str, stack: &str, match_pairs: &str, n: u32, max_depth: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::parsing::core::delimiter::core_delimiter_match(tok_types: &str, tok_depths: &str, tok_count: u32, open_tok_id: u32, close_tok_id: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::parsing::lr_tables
pub enum vyre_libs::parsing::lr_tables::Action
pub vyre_libs::parsing::lr_tables::Action::Accept
pub vyre_libs::parsing::lr_tables::Action::Error
pub vyre_libs::parsing::lr_tables::Action::Reduce(u32)
pub vyre_libs::parsing::lr_tables::Action::Shift(u32)
impl vyre_libs::parsing::lr_tables::Action
pub const fn vyre_libs::parsing::lr_tables::Action::pack(self) -> u32
pub const fn vyre_libs::parsing::lr_tables::Action::unpack(word: u32) -> Self
impl core::clone::Clone for vyre_libs::parsing::lr_tables::Action
pub fn vyre_libs::parsing::lr_tables::Action::clone(&self) -> vyre_libs::parsing::lr_tables::Action
impl core::cmp::Eq for vyre_libs::parsing::lr_tables::Action
impl core::cmp::PartialEq for vyre_libs::parsing::lr_tables::Action
pub fn vyre_libs::parsing::lr_tables::Action::eq(&self, other: &vyre_libs::parsing::lr_tables::Action) -> bool
impl core::fmt::Debug for vyre_libs::parsing::lr_tables::Action
pub fn vyre_libs::parsing::lr_tables::Action::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::parsing::lr_tables::Action
impl core::marker::StructuralPartialEq for vyre_libs::parsing::lr_tables::Action
impl core::marker::Freeze for vyre_libs::parsing::lr_tables::Action
impl core::marker::Send for vyre_libs::parsing::lr_tables::Action
impl core::marker::Sync for vyre_libs::parsing::lr_tables::Action
impl core::marker::Unpin for vyre_libs::parsing::lr_tables::Action
impl core::marker::UnsafeUnpin for vyre_libs::parsing::lr_tables::Action
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::parsing::lr_tables::Action
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::parsing::lr_tables::Action
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::parsing::lr_tables::Action where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Action::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::parsing::lr_tables::Action where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Action::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::parsing::lr_tables::Action where U: core::convert::From<T>
pub fn vyre_libs::parsing::lr_tables::Action::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::parsing::lr_tables::Action where U: core::convert::Into<T>
pub type vyre_libs::parsing::lr_tables::Action::Error = core::convert::Infallible
pub fn vyre_libs::parsing::lr_tables::Action::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::parsing::lr_tables::Action where U: core::convert::TryFrom<T>
pub type vyre_libs::parsing::lr_tables::Action::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::parsing::lr_tables::Action::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::parsing::lr_tables::Action where T: core::clone::Clone
pub type vyre_libs::parsing::lr_tables::Action::Owned = T
pub fn vyre_libs::parsing::lr_tables::Action::clone_into(&self, target: &mut T)
pub fn vyre_libs::parsing::lr_tables::Action::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::parsing::lr_tables::Action where T: 'static + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Action::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::parsing::lr_tables::Action where T: ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Action::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::parsing::lr_tables::Action where T: ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Action::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::parsing::lr_tables::Action where T: core::clone::Clone
pub unsafe fn vyre_libs::parsing::lr_tables::Action::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::parsing::lr_tables::Action
pub fn vyre_libs::parsing::lr_tables::Action::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::parsing::lr_tables::Action
pub type vyre_libs::parsing::lr_tables::Action::Init = T
pub const vyre_libs::parsing::lr_tables::Action::ALIGN: usize
pub unsafe fn vyre_libs::parsing::lr_tables::Action::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::parsing::lr_tables::Action::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::parsing::lr_tables::Action::drop(ptr: usize)
pub unsafe fn vyre_libs::parsing::lr_tables::Action::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::parsing::lr_tables::Action
impl<T> tracing::instrument::Instrument for vyre_libs::parsing::lr_tables::Action
impl<T> tracing::instrument::WithSubscriber for vyre_libs::parsing::lr_tables::Action
impl<T> typenum::type_operators::Same for vyre_libs::parsing::lr_tables::Action
pub type vyre_libs::parsing::lr_tables::Action::Output = T
pub enum vyre_libs::parsing::lr_tables::ParseError
pub vyre_libs::parsing::lr_tables::ParseError::InvalidProduction
pub vyre_libs::parsing::lr_tables::ParseError::InvalidProduction::prod_id: u32
pub vyre_libs::parsing::lr_tables::ParseError::NoGoto
pub vyre_libs::parsing::lr_tables::ParseError::NoGoto::nonterminal: u32
pub vyre_libs::parsing::lr_tables::ParseError::NoGoto::state: u32
pub vyre_libs::parsing::lr_tables::ParseError::StackUnderflow
pub vyre_libs::parsing::lr_tables::ParseError::UnexpectedToken
pub vyre_libs::parsing::lr_tables::ParseError::UnexpectedToken::pos: usize
pub vyre_libs::parsing::lr_tables::ParseError::UnexpectedToken::state: u32
pub vyre_libs::parsing::lr_tables::ParseError::UnexpectedToken::token: u32
impl core::clone::Clone for vyre_libs::parsing::lr_tables::ParseError
pub fn vyre_libs::parsing::lr_tables::ParseError::clone(&self) -> vyre_libs::parsing::lr_tables::ParseError
impl core::cmp::Eq for vyre_libs::parsing::lr_tables::ParseError
impl core::cmp::PartialEq for vyre_libs::parsing::lr_tables::ParseError
pub fn vyre_libs::parsing::lr_tables::ParseError::eq(&self, other: &vyre_libs::parsing::lr_tables::ParseError) -> bool
impl core::error::Error for vyre_libs::parsing::lr_tables::ParseError
impl core::fmt::Debug for vyre_libs::parsing::lr_tables::ParseError
pub fn vyre_libs::parsing::lr_tables::ParseError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::parsing::lr_tables::ParseError
pub fn vyre_libs::parsing::lr_tables::ParseError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_libs::parsing::lr_tables::ParseError
impl core::marker::Freeze for vyre_libs::parsing::lr_tables::ParseError
impl core::marker::Send for vyre_libs::parsing::lr_tables::ParseError
impl core::marker::Sync for vyre_libs::parsing::lr_tables::ParseError
impl core::marker::Unpin for vyre_libs::parsing::lr_tables::ParseError
impl core::marker::UnsafeUnpin for vyre_libs::parsing::lr_tables::ParseError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::parsing::lr_tables::ParseError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::parsing::lr_tables::ParseError
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::parsing::lr_tables::ParseError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::ParseError::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::parsing::lr_tables::ParseError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::ParseError::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::parsing::lr_tables::ParseError where U: core::convert::From<T>
pub fn vyre_libs::parsing::lr_tables::ParseError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::parsing::lr_tables::ParseError where U: core::convert::Into<T>
pub type vyre_libs::parsing::lr_tables::ParseError::Error = core::convert::Infallible
pub fn vyre_libs::parsing::lr_tables::ParseError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::parsing::lr_tables::ParseError where U: core::convert::TryFrom<T>
pub type vyre_libs::parsing::lr_tables::ParseError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::parsing::lr_tables::ParseError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::parsing::lr_tables::ParseError where T: core::clone::Clone
pub type vyre_libs::parsing::lr_tables::ParseError::Owned = T
pub fn vyre_libs::parsing::lr_tables::ParseError::clone_into(&self, target: &mut T)
pub fn vyre_libs::parsing::lr_tables::ParseError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_libs::parsing::lr_tables::ParseError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::ParseError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::parsing::lr_tables::ParseError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::ParseError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::parsing::lr_tables::ParseError where T: ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::ParseError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::parsing::lr_tables::ParseError where T: ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::ParseError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::parsing::lr_tables::ParseError where T: core::clone::Clone
pub unsafe fn vyre_libs::parsing::lr_tables::ParseError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::parsing::lr_tables::ParseError
pub fn vyre_libs::parsing::lr_tables::ParseError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::parsing::lr_tables::ParseError
pub type vyre_libs::parsing::lr_tables::ParseError::Init = T
pub const vyre_libs::parsing::lr_tables::ParseError::ALIGN: usize
pub unsafe fn vyre_libs::parsing::lr_tables::ParseError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::parsing::lr_tables::ParseError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::parsing::lr_tables::ParseError::drop(ptr: usize)
pub unsafe fn vyre_libs::parsing::lr_tables::ParseError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::parsing::lr_tables::ParseError
impl<T> tracing::instrument::Instrument for vyre_libs::parsing::lr_tables::ParseError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::parsing::lr_tables::ParseError
impl<T> typenum::type_operators::Same for vyre_libs::parsing::lr_tables::ParseError
pub type vyre_libs::parsing::lr_tables::ParseError::Output = T
pub struct vyre_libs::parsing::lr_tables::LrTables
pub vyre_libs::parsing::lr_tables::LrTables::action: &'static [u32]
pub vyre_libs::parsing::lr_tables::LrTables::goto: &'static [u32]
pub vyre_libs::parsing::lr_tables::LrTables::num_nonterminals: u32
pub vyre_libs::parsing::lr_tables::LrTables::num_states: u32
pub vyre_libs::parsing::lr_tables::LrTables::num_tokens: u32
pub vyre_libs::parsing::lr_tables::LrTables::productions: &'static [vyre_libs::parsing::lr_tables::Production]
impl vyre_libs::parsing::lr_tables::LrTables
pub fn vyre_libs::parsing::lr_tables::LrTables::action_at(&self, state: u32, token: u32) -> vyre_libs::parsing::lr_tables::Action
pub fn vyre_libs::parsing::lr_tables::LrTables::goto_at(&self, state: u32, nt: u32) -> u32
impl core::clone::Clone for vyre_libs::parsing::lr_tables::LrTables
pub fn vyre_libs::parsing::lr_tables::LrTables::clone(&self) -> vyre_libs::parsing::lr_tables::LrTables
impl core::cmp::Eq for vyre_libs::parsing::lr_tables::LrTables
impl core::cmp::PartialEq for vyre_libs::parsing::lr_tables::LrTables
pub fn vyre_libs::parsing::lr_tables::LrTables::eq(&self, other: &vyre_libs::parsing::lr_tables::LrTables) -> bool
impl core::fmt::Debug for vyre_libs::parsing::lr_tables::LrTables
pub fn vyre_libs::parsing::lr_tables::LrTables::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::parsing::lr_tables::LrTables
impl core::marker::StructuralPartialEq for vyre_libs::parsing::lr_tables::LrTables
impl core::marker::Freeze for vyre_libs::parsing::lr_tables::LrTables
impl core::marker::Send for vyre_libs::parsing::lr_tables::LrTables
impl core::marker::Sync for vyre_libs::parsing::lr_tables::LrTables
impl core::marker::Unpin for vyre_libs::parsing::lr_tables::LrTables
impl core::marker::UnsafeUnpin for vyre_libs::parsing::lr_tables::LrTables
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::parsing::lr_tables::LrTables
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::parsing::lr_tables::LrTables
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::parsing::lr_tables::LrTables where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::LrTables::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::parsing::lr_tables::LrTables where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::LrTables::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::parsing::lr_tables::LrTables where U: core::convert::From<T>
pub fn vyre_libs::parsing::lr_tables::LrTables::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::parsing::lr_tables::LrTables where U: core::convert::Into<T>
pub type vyre_libs::parsing::lr_tables::LrTables::Error = core::convert::Infallible
pub fn vyre_libs::parsing::lr_tables::LrTables::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::parsing::lr_tables::LrTables where U: core::convert::TryFrom<T>
pub type vyre_libs::parsing::lr_tables::LrTables::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::parsing::lr_tables::LrTables::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::parsing::lr_tables::LrTables where T: core::clone::Clone
pub type vyre_libs::parsing::lr_tables::LrTables::Owned = T
pub fn vyre_libs::parsing::lr_tables::LrTables::clone_into(&self, target: &mut T)
pub fn vyre_libs::parsing::lr_tables::LrTables::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::parsing::lr_tables::LrTables where T: 'static + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::LrTables::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::parsing::lr_tables::LrTables where T: ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::LrTables::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::parsing::lr_tables::LrTables where T: ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::LrTables::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::parsing::lr_tables::LrTables where T: core::clone::Clone
pub unsafe fn vyre_libs::parsing::lr_tables::LrTables::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::parsing::lr_tables::LrTables
pub fn vyre_libs::parsing::lr_tables::LrTables::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::parsing::lr_tables::LrTables
pub type vyre_libs::parsing::lr_tables::LrTables::Init = T
pub const vyre_libs::parsing::lr_tables::LrTables::ALIGN: usize
pub unsafe fn vyre_libs::parsing::lr_tables::LrTables::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::parsing::lr_tables::LrTables::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::parsing::lr_tables::LrTables::drop(ptr: usize)
pub unsafe fn vyre_libs::parsing::lr_tables::LrTables::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::parsing::lr_tables::LrTables
impl<T> tracing::instrument::Instrument for vyre_libs::parsing::lr_tables::LrTables
impl<T> tracing::instrument::WithSubscriber for vyre_libs::parsing::lr_tables::LrTables
impl<T> typenum::type_operators::Same for vyre_libs::parsing::lr_tables::LrTables
pub type vyre_libs::parsing::lr_tables::LrTables::Output = T
pub struct vyre_libs::parsing::lr_tables::Production
pub vyre_libs::parsing::lr_tables::Production::lhs: u32
pub vyre_libs::parsing::lr_tables::Production::rhs_len: u32
impl core::clone::Clone for vyre_libs::parsing::lr_tables::Production
pub fn vyre_libs::parsing::lr_tables::Production::clone(&self) -> vyre_libs::parsing::lr_tables::Production
impl core::cmp::Eq for vyre_libs::parsing::lr_tables::Production
impl core::cmp::PartialEq for vyre_libs::parsing::lr_tables::Production
pub fn vyre_libs::parsing::lr_tables::Production::eq(&self, other: &vyre_libs::parsing::lr_tables::Production) -> bool
impl core::fmt::Debug for vyre_libs::parsing::lr_tables::Production
pub fn vyre_libs::parsing::lr_tables::Production::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::parsing::lr_tables::Production
impl core::marker::StructuralPartialEq for vyre_libs::parsing::lr_tables::Production
impl core::marker::Freeze for vyre_libs::parsing::lr_tables::Production
impl core::marker::Send for vyre_libs::parsing::lr_tables::Production
impl core::marker::Sync for vyre_libs::parsing::lr_tables::Production
impl core::marker::Unpin for vyre_libs::parsing::lr_tables::Production
impl core::marker::UnsafeUnpin for vyre_libs::parsing::lr_tables::Production
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::parsing::lr_tables::Production
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::parsing::lr_tables::Production
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::parsing::lr_tables::Production where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Production::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::parsing::lr_tables::Production where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Production::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::parsing::lr_tables::Production where U: core::convert::From<T>
pub fn vyre_libs::parsing::lr_tables::Production::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::parsing::lr_tables::Production where U: core::convert::Into<T>
pub type vyre_libs::parsing::lr_tables::Production::Error = core::convert::Infallible
pub fn vyre_libs::parsing::lr_tables::Production::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::parsing::lr_tables::Production where U: core::convert::TryFrom<T>
pub type vyre_libs::parsing::lr_tables::Production::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::parsing::lr_tables::Production::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::parsing::lr_tables::Production where T: core::clone::Clone
pub type vyre_libs::parsing::lr_tables::Production::Owned = T
pub fn vyre_libs::parsing::lr_tables::Production::clone_into(&self, target: &mut T)
pub fn vyre_libs::parsing::lr_tables::Production::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::parsing::lr_tables::Production where T: 'static + ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Production::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::parsing::lr_tables::Production where T: ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Production::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::parsing::lr_tables::Production where T: ?core::marker::Sized
pub fn vyre_libs::parsing::lr_tables::Production::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::parsing::lr_tables::Production where T: core::clone::Clone
pub unsafe fn vyre_libs::parsing::lr_tables::Production::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::parsing::lr_tables::Production
pub fn vyre_libs::parsing::lr_tables::Production::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::parsing::lr_tables::Production
pub type vyre_libs::parsing::lr_tables::Production::Init = T
pub const vyre_libs::parsing::lr_tables::Production::ALIGN: usize
pub unsafe fn vyre_libs::parsing::lr_tables::Production::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::parsing::lr_tables::Production::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::parsing::lr_tables::Production::drop(ptr: usize)
pub unsafe fn vyre_libs::parsing::lr_tables::Production::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::parsing::lr_tables::Production
impl<T> tracing::instrument::Instrument for vyre_libs::parsing::lr_tables::Production
impl<T> tracing::instrument::WithSubscriber for vyre_libs::parsing::lr_tables::Production
impl<T> typenum::type_operators::Same for vyre_libs::parsing::lr_tables::Production
pub type vyre_libs::parsing::lr_tables::Production::Output = T
pub const vyre_libs::parsing::lr_tables::NT_E: u32
pub const vyre_libs::parsing::lr_tables::NT_F: u32
pub const vyre_libs::parsing::lr_tables::NT_T: u32
pub const vyre_libs::parsing::lr_tables::TOK_EOF: u32
pub const vyre_libs::parsing::lr_tables::TOK_ID: u32
pub const vyre_libs::parsing::lr_tables::TOK_LPAREN: u32
pub const vyre_libs::parsing::lr_tables::TOK_MINUS: u32
pub const vyre_libs::parsing::lr_tables::TOK_NUM: u32
pub const vyre_libs::parsing::lr_tables::TOK_PLUS: u32
pub const vyre_libs::parsing::lr_tables::TOK_RPAREN: u32
pub const vyre_libs::parsing::lr_tables::TOK_SLASH: u32
pub const vyre_libs::parsing::lr_tables::TOK_STAR: u32
pub static vyre_libs::parsing::lr_tables::ACTION_TABLE: &[u32]
pub static vyre_libs::parsing::lr_tables::C11_EXPR: vyre_libs::parsing::lr_tables::LrTables
pub static vyre_libs::parsing::lr_tables::GOTO_TABLE: &[u32]
pub static vyre_libs::parsing::lr_tables::PRODUCTIONS: &[vyre_libs::parsing::lr_tables::Production]
pub fn vyre_libs::parsing::lr_tables::parse_lr(tables: &vyre_libs::parsing::lr_tables::LrTables, tokens: &[u32]) -> core::result::Result<alloc::vec::Vec<u32>, vyre_libs::parsing::lr_tables::ParseError>
pub mod vyre_libs::parsing::parallel_parse
pub fn vyre_libs::parsing::parallel_parse::parse_corpus_parallel<T, F>(sources: &[(alloc::vec::Vec<u8>, alloc::vec::Vec<u8>)], cache: &vyre_libs::parsing::source_cache::ParsedSourceLru<T>, parse: F) -> alloc::vec::Vec<alloc::sync::Arc<T>> where T: core::marker::Send + core::marker::Sync, F: core::ops::function::Fn(&[u8]) -> T + core::marker::Sync
pub mod vyre_libs::parsing::source_cache
pub struct vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T> vyre_libs::parsing::source_cache::ParsedSourceLru<T>
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::get(&self, key: vyre_libs::parsing::source_cache::SourceHash) -> core::option::Option<alloc::sync::Arc<T>>
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::get_or_parse<F>(&self, source: &[u8], extra: &[u8], parse: F) -> alloc::sync::Arc<T> where F: core::ops::function::FnOnce(&[u8]) -> T
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::insert(&self, key: vyre_libs::parsing::source_cache::SourceHash, value: T) -> alloc::sync::Arc<T>
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::is_empty(&self) -> bool
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::len(&self) -> usize
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::with_capacity(capacity: usize) -> Self
impl<T> !core::marker::Freeze for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T> core::marker::Send for vyre_libs::parsing::source_cache::ParsedSourceLru<T> where T: core::marker::Sync + core::marker::Send
impl<T> core::marker::Sync for vyre_libs::parsing::source_cache::ParsedSourceLru<T> where T: core::marker::Sync + core::marker::Send
impl<T> core::marker::Unpin for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T> core::marker::UnsafeUnpin for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T> core::panic::unwind_safe::RefUnwindSafe for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T> core::panic::unwind_safe::UnwindSafe for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T, U> core::convert::Into<U> for vyre_libs::parsing::source_cache::ParsedSourceLru<T> where U: core::convert::From<T>
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::parsing::source_cache::ParsedSourceLru<T> where U: core::convert::Into<T>
pub type vyre_libs::parsing::source_cache::ParsedSourceLru<T>::Error = core::convert::Infallible
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::parsing::source_cache::ParsedSourceLru<T> where U: core::convert::TryFrom<T>
pub type vyre_libs::parsing::source_cache::ParsedSourceLru<T>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::parsing::source_cache::ParsedSourceLru<T> where T: 'static + ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::parsing::source_cache::ParsedSourceLru<T> where T: ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::parsing::source_cache::ParsedSourceLru<T> where T: ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
pub fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
pub type vyre_libs::parsing::source_cache::ParsedSourceLru<T>::Init = T
pub const vyre_libs::parsing::source_cache::ParsedSourceLru<T>::ALIGN: usize
pub unsafe fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::drop(ptr: usize)
pub unsafe fn vyre_libs::parsing::source_cache::ParsedSourceLru<T>::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T> tracing::instrument::Instrument for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T> tracing::instrument::WithSubscriber for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
impl<T> typenum::type_operators::Same for vyre_libs::parsing::source_cache::ParsedSourceLru<T>
pub type vyre_libs::parsing::source_cache::ParsedSourceLru<T>::Output = T
pub struct vyre_libs::parsing::source_cache::SourceHash(pub [u8; 32])
impl vyre_libs::parsing::source_cache::SourceHash
pub fn vyre_libs::parsing::source_cache::SourceHash::of(source: &[u8], extra: &[u8]) -> Self
impl core::clone::Clone for vyre_libs::parsing::source_cache::SourceHash
pub fn vyre_libs::parsing::source_cache::SourceHash::clone(&self) -> vyre_libs::parsing::source_cache::SourceHash
impl core::cmp::Eq for vyre_libs::parsing::source_cache::SourceHash
impl core::cmp::Ord for vyre_libs::parsing::source_cache::SourceHash
pub fn vyre_libs::parsing::source_cache::SourceHash::cmp(&self, other: &vyre_libs::parsing::source_cache::SourceHash) -> core::cmp::Ordering
impl core::cmp::PartialEq for vyre_libs::parsing::source_cache::SourceHash
pub fn vyre_libs::parsing::source_cache::SourceHash::eq(&self, other: &vyre_libs::parsing::source_cache::SourceHash) -> bool
impl core::cmp::PartialOrd for vyre_libs::parsing::source_cache::SourceHash
pub fn vyre_libs::parsing::source_cache::SourceHash::partial_cmp(&self, other: &vyre_libs::parsing::source_cache::SourceHash) -> core::option::Option<core::cmp::Ordering>
impl core::fmt::Debug for vyre_libs::parsing::source_cache::SourceHash
pub fn vyre_libs::parsing::source_cache::SourceHash::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_libs::parsing::source_cache::SourceHash
pub fn vyre_libs::parsing::source_cache::SourceHash::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_libs::parsing::source_cache::SourceHash
impl core::marker::StructuralPartialEq for vyre_libs::parsing::source_cache::SourceHash
impl core::marker::Freeze for vyre_libs::parsing::source_cache::SourceHash
impl core::marker::Send for vyre_libs::parsing::source_cache::SourceHash
impl core::marker::Sync for vyre_libs::parsing::source_cache::SourceHash
impl core::marker::Unpin for vyre_libs::parsing::source_cache::SourceHash
impl core::marker::UnsafeUnpin for vyre_libs::parsing::source_cache::SourceHash
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::parsing::source_cache::SourceHash
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::parsing::source_cache::SourceHash
impl<Q, K> equivalent::Comparable<K> for vyre_libs::parsing::source_cache::SourceHash where Q: core::cmp::Ord + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::SourceHash::compare(&self, key: &K) -> core::cmp::Ordering
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::parsing::source_cache::SourceHash where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::SourceHash::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::parsing::source_cache::SourceHash where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::SourceHash::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::parsing::source_cache::SourceHash where U: core::convert::From<T>
pub fn vyre_libs::parsing::source_cache::SourceHash::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::parsing::source_cache::SourceHash where U: core::convert::Into<T>
pub type vyre_libs::parsing::source_cache::SourceHash::Error = core::convert::Infallible
pub fn vyre_libs::parsing::source_cache::SourceHash::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::parsing::source_cache::SourceHash where U: core::convert::TryFrom<T>
pub type vyre_libs::parsing::source_cache::SourceHash::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::parsing::source_cache::SourceHash::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::parsing::source_cache::SourceHash where T: core::clone::Clone
pub type vyre_libs::parsing::source_cache::SourceHash::Owned = T
pub fn vyre_libs::parsing::source_cache::SourceHash::clone_into(&self, target: &mut T)
pub fn vyre_libs::parsing::source_cache::SourceHash::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::parsing::source_cache::SourceHash where T: 'static + ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::SourceHash::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::parsing::source_cache::SourceHash where T: ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::SourceHash::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::parsing::source_cache::SourceHash where T: ?core::marker::Sized
pub fn vyre_libs::parsing::source_cache::SourceHash::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::parsing::source_cache::SourceHash where T: core::clone::Clone
pub unsafe fn vyre_libs::parsing::source_cache::SourceHash::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::parsing::source_cache::SourceHash
pub fn vyre_libs::parsing::source_cache::SourceHash::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::parsing::source_cache::SourceHash
pub type vyre_libs::parsing::source_cache::SourceHash::Init = T
pub const vyre_libs::parsing::source_cache::SourceHash::ALIGN: usize
pub unsafe fn vyre_libs::parsing::source_cache::SourceHash::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::parsing::source_cache::SourceHash::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::parsing::source_cache::SourceHash::drop(ptr: usize)
pub unsafe fn vyre_libs::parsing::source_cache::SourceHash::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::parsing::source_cache::SourceHash
impl<T> tracing::instrument::Instrument for vyre_libs::parsing::source_cache::SourceHash
impl<T> tracing::instrument::WithSubscriber for vyre_libs::parsing::source_cache::SourceHash
impl<T> typenum::type_operators::Same for vyre_libs::parsing::source_cache::SourceHash
pub type vyre_libs::parsing::source_cache::SourceHash::Output = T
pub fn vyre_libs::parsing::source_cache::source_len_u32_nonzero(source: &[u8]) -> u32
pub mod vyre_libs::parsing::vast
pub use vyre_libs::parsing::vast::HEADER_LEN
pub use vyre_libs::parsing::vast::NODE_STRIDE_U32
pub use vyre_libs::parsing::vast::SENTINEL
pub use vyre_libs::parsing::vast::VAST_MAGIC
pub use vyre_libs::parsing::vast::VAST_VERSION
pub use vyre_libs::parsing::vast::VastError
pub use vyre_libs::parsing::vast::VastHeader
pub use vyre_libs::parsing::vast::VastNode
pub use vyre_libs::parsing::vast::pack_spine_vast
pub use vyre_libs::parsing::vast::validate_vast
pub use vyre_libs::parsing::vast::walk_postorder_indices
pub use vyre_libs::parsing::vast::walk_preorder_indices
pub mod vyre_libs::prelude
pub use vyre_libs::prelude::BackendError
pub use vyre_libs::prelude::BufferAccess
pub use vyre_libs::prelude::BufferDecl
pub use vyre_libs::prelude::CompiledDfa
pub use vyre_libs::prelude::DataType
pub use vyre_libs::prelude::DfaCompileError
pub use vyre_libs::prelude::DispatchConfig
pub use vyre_libs::prelude::Expr
pub use vyre_libs::prelude::GeneratorRef
pub use vyre_libs::prelude::Node
pub use vyre_libs::prelude::Program
pub use vyre_libs::prelude::dfa_compile
pub use vyre_libs::prelude::wrap
pub use vyre_libs::prelude::wrap_anonymous
pub use vyre_libs::prelude::wrap_child
pub mod vyre_libs::prelude::broadcast
pub fn vyre_libs::prelude::broadcast::broadcast(src: &str, dst: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::prelude::inflate
pub fn vyre_libs::prelude::inflate::inflate(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::inflate::inflate_stored_block(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::inflate::inflate_stored_block_buffered_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::inflate::inflate_stored_block_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::inflate::inflate_stored_block_tiled_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32, tile_width: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::inflate::inflate_then_aho_corasick(input: &str, decoded: &str, transitions: &str, accept: &str, matches: &str, input_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::prelude::linear
pub struct vyre_libs::prelude::linear::Linear
impl vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::nn::Linear::new(x: vyre_libs::tensor_ref::TensorRef, w: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::nn::Linear::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::nn::Linear::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::clone(&self) -> vyre_libs::nn::Linear
impl core::fmt::Debug for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::nn::Linear
impl core::marker::Send for vyre_libs::nn::Linear
impl core::marker::Sync for vyre_libs::nn::Linear
impl core::marker::Unpin for vyre_libs::nn::Linear
impl core::marker::UnsafeUnpin for vyre_libs::nn::Linear
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::nn::Linear
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::nn::Linear
impl<T, U> core::convert::Into<U> for vyre_libs::nn::Linear where U: core::convert::From<T>
pub fn vyre_libs::nn::Linear::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::nn::Linear where U: core::convert::Into<T>
pub type vyre_libs::nn::Linear::Error = core::convert::Infallible
pub fn vyre_libs::nn::Linear::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::nn::Linear where U: core::convert::TryFrom<T>
pub type vyre_libs::nn::Linear::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::nn::Linear::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::nn::Linear where T: core::clone::Clone
pub type vyre_libs::nn::Linear::Owned = T
pub fn vyre_libs::nn::Linear::clone_into(&self, target: &mut T)
pub fn vyre_libs::nn::Linear::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::nn::Linear where T: 'static + ?core::marker::Sized
pub fn vyre_libs::nn::Linear::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::nn::Linear where T: ?core::marker::Sized
pub fn vyre_libs::nn::Linear::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::nn::Linear where T: ?core::marker::Sized
pub fn vyre_libs::nn::Linear::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::nn::Linear where T: core::clone::Clone
pub unsafe fn vyre_libs::nn::Linear::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::nn::Linear
pub fn vyre_libs::nn::Linear::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::nn::Linear
pub type vyre_libs::nn::Linear::Init = T
pub const vyre_libs::nn::Linear::ALIGN: usize
pub unsafe fn vyre_libs::nn::Linear::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::nn::Linear::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::nn::Linear::drop(ptr: usize)
pub unsafe fn vyre_libs::nn::Linear::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::nn::Linear
impl<T> tracing::instrument::Instrument for vyre_libs::nn::Linear
impl<T> tracing::instrument::WithSubscriber for vyre_libs::nn::Linear
impl<T> typenum::type_operators::Same for vyre_libs::nn::Linear
pub type vyre_libs::nn::Linear::Output = T
pub fn vyre_libs::prelude::linear::batch_matmul(a: &str, b: &str, out: &str, batch: u32, m: u32, k: u32, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::prelude::linear::linear(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::prelude::linear::linear_relu(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::prelude::linear::linear_silu(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::prelude::linear::linear_tiled(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32, tile: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::prelude::linear::linear_tiled_reference(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32, tile: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::prelude::linear::rms_norm_linear(input: &str, w: &str, b: &str, out: &str, n: u32, in_dim: u32, out_dim: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::linear::try_rms_norm_linear(input: &str, w: &str, b: &str, out: &str, n: u32, in_dim: u32, out_dim: u32, eps: f32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub mod vyre_libs::prelude::matmul_tiled
pub struct vyre_libs::prelude::matmul_tiled::MatmulBiasTiled
impl vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulBiasTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBiasTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::clone(&self) -> vyre_libs::MatmulBiasTiled
impl core::fmt::Debug for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBiasTiled
impl core::marker::Send for vyre_libs::MatmulBiasTiled
impl core::marker::Sync for vyre_libs::MatmulBiasTiled
impl core::marker::Unpin for vyre_libs::MatmulBiasTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBiasTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBiasTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulBiasTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBiasTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulBiasTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBiasTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBiasTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBiasTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBiasTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub type vyre_libs::MatmulBiasTiled::Owned = T
pub fn vyre_libs::MatmulBiasTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBiasTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBiasTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBiasTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Init = T
pub const vyre_libs::MatmulBiasTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBiasTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBiasTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBiasTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBiasTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBiasTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Output = T
pub struct vyre_libs::prelude::matmul_tiled::MatmulTiled
impl vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::clone(&self) -> vyre_libs::MatmulTiled
impl core::fmt::Debug for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulTiled
impl core::marker::Send for vyre_libs::MatmulTiled
impl core::marker::Sync for vyre_libs::MatmulTiled
impl core::marker::Unpin for vyre_libs::MatmulTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulTiled where T: core::clone::Clone
pub type vyre_libs::MatmulTiled::Owned = T
pub fn vyre_libs::MatmulTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Init = T
pub const vyre_libs::MatmulTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Output = T
pub fn vyre_libs::prelude::matmul_tiled::matmul_bias_tiled(a: &str, b: &str, bias: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::matmul_tiled::matmul_tiled(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::prelude::relu
pub fn vyre_libs::prelude::relu::relu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
#[non_exhaustive] pub enum vyre_libs::prelude::TensorRefError
pub vyre_libs::prelude::TensorRefError::DtypeMismatch
pub vyre_libs::prelude::TensorRefError::DtypeMismatch::expected: vyre_spec::data_type::DataType
pub vyre_libs::prelude::TensorRefError::DtypeMismatch::found: vyre_spec::data_type::DataType
pub vyre_libs::prelude::TensorRefError::DtypeMismatch::name: alloc::string::String
pub vyre_libs::prelude::TensorRefError::DtypeMismatch::op: &'static str
pub vyre_libs::prelude::TensorRefError::ElementCountOverflow
pub vyre_libs::prelude::TensorRefError::ElementCountOverflow::name: alloc::string::String
pub vyre_libs::prelude::TensorRefError::ElementCountOverflow::shape: alloc::vec::Vec<u32>
pub vyre_libs::prelude::TensorRefError::NameCollision
pub vyre_libs::prelude::TensorRefError::NameCollision::name: alloc::string::String
pub vyre_libs::prelude::TensorRefError::NameCollision::op: &'static str
pub vyre_libs::prelude::TensorRefError::ShapeMismatch
pub vyre_libs::prelude::TensorRefError::ShapeMismatch::expected: alloc::vec::Vec<u32>
pub vyre_libs::prelude::TensorRefError::ShapeMismatch::found: alloc::vec::Vec<u32>
pub vyre_libs::prelude::TensorRefError::ShapeMismatch::name: alloc::string::String
pub vyre_libs::prelude::TensorRefError::ShapeMismatch::op: &'static str
impl core::clone::Clone for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::clone(&self) -> vyre_libs::tensor_ref::TensorRefError
impl core::error::Error for vyre_libs::tensor_ref::TensorRefError
impl core::fmt::Debug for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::fmt(&self, __formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Send for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Sync for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Unpin for vyre_libs::tensor_ref::TensorRefError
impl core::marker::UnsafeUnpin for vyre_libs::tensor_ref::TensorRefError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::tensor_ref::TensorRefError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::tensor_ref::TensorRefError
impl<T, U> core::convert::Into<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::From<T>
pub fn vyre_libs::tensor_ref::TensorRefError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::Into<T>
pub type vyre_libs::tensor_ref::TensorRefError::Error = core::convert::Infallible
pub fn vyre_libs::tensor_ref::TensorRefError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::TryFrom<T>
pub type vyre_libs::tensor_ref::TensorRefError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::tensor_ref::TensorRefError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::tensor_ref::TensorRefError where T: core::clone::Clone
pub type vyre_libs::tensor_ref::TensorRefError::Owned = T
pub fn vyre_libs::tensor_ref::TensorRefError::clone_into(&self, target: &mut T)
pub fn vyre_libs::tensor_ref::TensorRefError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_libs::tensor_ref::TensorRefError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::tensor_ref::TensorRefError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::tensor_ref::TensorRefError where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::tensor_ref::TensorRefError where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::tensor_ref::TensorRefError where T: core::clone::Clone
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::tensor_ref::TensorRefError
pub type vyre_libs::tensor_ref::TensorRefError::Init = T
pub const vyre_libs::tensor_ref::TensorRefError::ALIGN: usize
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::drop(ptr: usize)
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::tensor_ref::TensorRefError
impl<T> tracing::instrument::Instrument for vyre_libs::tensor_ref::TensorRefError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::tensor_ref::TensorRefError
impl<T> typenum::type_operators::Same for vyre_libs::tensor_ref::TensorRefError
pub type vyre_libs::tensor_ref::TensorRefError::Output = T
#[non_exhaustive] pub struct vyre_libs::prelude::BuildOptions
pub vyre_libs::prelude::BuildOptions::region_generator: core::option::Option<&'static str>
pub vyre_libs::prelude::BuildOptions::tenant_id: core::option::Option<u32>
pub vyre_libs::prelude::BuildOptions::workgroup_size: core::option::Option<[u32; 3]>
impl vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::new() -> Self
pub fn vyre_libs::builder::BuildOptions::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::builder::BuildOptions::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::builder::BuildOptions::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::clone(&self) -> vyre_libs::builder::BuildOptions
impl core::default::Default for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::default() -> vyre_libs::builder::BuildOptions
impl core::fmt::Debug for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::builder::BuildOptions
impl core::marker::Send for vyre_libs::builder::BuildOptions
impl core::marker::Sync for vyre_libs::builder::BuildOptions
impl core::marker::Unpin for vyre_libs::builder::BuildOptions
impl core::marker::UnsafeUnpin for vyre_libs::builder::BuildOptions
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::builder::BuildOptions
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::builder::BuildOptions
impl<T, U> core::convert::Into<U> for vyre_libs::builder::BuildOptions where U: core::convert::From<T>
pub fn vyre_libs::builder::BuildOptions::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::builder::BuildOptions where U: core::convert::Into<T>
pub type vyre_libs::builder::BuildOptions::Error = core::convert::Infallible
pub fn vyre_libs::builder::BuildOptions::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::builder::BuildOptions where U: core::convert::TryFrom<T>
pub type vyre_libs::builder::BuildOptions::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::builder::BuildOptions::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::builder::BuildOptions where T: core::clone::Clone
pub type vyre_libs::builder::BuildOptions::Owned = T
pub fn vyre_libs::builder::BuildOptions::clone_into(&self, target: &mut T)
pub fn vyre_libs::builder::BuildOptions::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::builder::BuildOptions where T: 'static + ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::builder::BuildOptions where T: ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::builder::BuildOptions where T: ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::builder::BuildOptions where T: core::clone::Clone
pub unsafe fn vyre_libs::builder::BuildOptions::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::builder::BuildOptions
pub type vyre_libs::builder::BuildOptions::Init = T
pub const vyre_libs::builder::BuildOptions::ALIGN: usize
pub unsafe fn vyre_libs::builder::BuildOptions::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::builder::BuildOptions::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::builder::BuildOptions::drop(ptr: usize)
pub unsafe fn vyre_libs::builder::BuildOptions::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::builder::BuildOptions
impl<T> tracing::instrument::Instrument for vyre_libs::builder::BuildOptions
impl<T> tracing::instrument::WithSubscriber for vyre_libs::builder::BuildOptions
impl<T> typenum::type_operators::Same for vyre_libs::builder::BuildOptions
pub type vyre_libs::builder::BuildOptions::Output = T
pub struct vyre_libs::prelude::LayerNorm
impl vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::nn::LayerNorm::new(input: vyre_libs::tensor_ref::TensorRef, output: vyre_libs::tensor_ref::TensorRef, eps: f32) -> Self
impl vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::nn::LayerNorm::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::nn::LayerNorm::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::clone(&self) -> vyre_libs::nn::LayerNorm
impl core::fmt::Debug for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::nn::LayerNorm
impl core::marker::Send for vyre_libs::nn::LayerNorm
impl core::marker::Sync for vyre_libs::nn::LayerNorm
impl core::marker::Unpin for vyre_libs::nn::LayerNorm
impl core::marker::UnsafeUnpin for vyre_libs::nn::LayerNorm
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::nn::LayerNorm
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::nn::LayerNorm
impl<T, U> core::convert::Into<U> for vyre_libs::nn::LayerNorm where U: core::convert::From<T>
pub fn vyre_libs::nn::LayerNorm::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::nn::LayerNorm where U: core::convert::Into<T>
pub type vyre_libs::nn::LayerNorm::Error = core::convert::Infallible
pub fn vyre_libs::nn::LayerNorm::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::nn::LayerNorm where U: core::convert::TryFrom<T>
pub type vyre_libs::nn::LayerNorm::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::nn::LayerNorm::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::nn::LayerNorm where T: core::clone::Clone
pub type vyre_libs::nn::LayerNorm::Owned = T
pub fn vyre_libs::nn::LayerNorm::clone_into(&self, target: &mut T)
pub fn vyre_libs::nn::LayerNorm::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::nn::LayerNorm where T: 'static + ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::nn::LayerNorm where T: ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::nn::LayerNorm where T: ?core::marker::Sized
pub fn vyre_libs::nn::LayerNorm::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::nn::LayerNorm where T: core::clone::Clone
pub unsafe fn vyre_libs::nn::LayerNorm::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::nn::LayerNorm
pub fn vyre_libs::nn::LayerNorm::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::nn::LayerNorm
pub type vyre_libs::nn::LayerNorm::Init = T
pub const vyre_libs::nn::LayerNorm::ALIGN: usize
pub unsafe fn vyre_libs::nn::LayerNorm::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::nn::LayerNorm::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::nn::LayerNorm::drop(ptr: usize)
pub unsafe fn vyre_libs::nn::LayerNorm::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::nn::LayerNorm
impl<T> tracing::instrument::Instrument for vyre_libs::nn::LayerNorm
impl<T> tracing::instrument::WithSubscriber for vyre_libs::nn::LayerNorm
impl<T> typenum::type_operators::Same for vyre_libs::nn::LayerNorm
pub type vyre_libs::nn::LayerNorm::Output = T
pub struct vyre_libs::prelude::Matmul
impl vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::math::Matmul::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::math::Matmul::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::math::Matmul::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::clone(&self) -> vyre_libs::math::Matmul
impl core::fmt::Debug for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::math::Matmul
impl core::marker::Send for vyre_libs::math::Matmul
impl core::marker::Sync for vyre_libs::math::Matmul
impl core::marker::Unpin for vyre_libs::math::Matmul
impl core::marker::UnsafeUnpin for vyre_libs::math::Matmul
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::math::Matmul
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::math::Matmul
impl<T, U> core::convert::Into<U> for vyre_libs::math::Matmul where U: core::convert::From<T>
pub fn vyre_libs::math::Matmul::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::math::Matmul where U: core::convert::Into<T>
pub type vyre_libs::math::Matmul::Error = core::convert::Infallible
pub fn vyre_libs::math::Matmul::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::math::Matmul where U: core::convert::TryFrom<T>
pub type vyre_libs::math::Matmul::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::math::Matmul::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::math::Matmul where T: core::clone::Clone
pub type vyre_libs::math::Matmul::Owned = T
pub fn vyre_libs::math::Matmul::clone_into(&self, target: &mut T)
pub fn vyre_libs::math::Matmul::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::math::Matmul where T: 'static + ?core::marker::Sized
pub fn vyre_libs::math::Matmul::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::math::Matmul where T: ?core::marker::Sized
pub fn vyre_libs::math::Matmul::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::math::Matmul where T: ?core::marker::Sized
pub fn vyre_libs::math::Matmul::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::math::Matmul where T: core::clone::Clone
pub unsafe fn vyre_libs::math::Matmul::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::math::Matmul
pub fn vyre_libs::math::Matmul::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::math::Matmul
pub type vyre_libs::math::Matmul::Init = T
pub const vyre_libs::math::Matmul::ALIGN: usize
pub unsafe fn vyre_libs::math::Matmul::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::math::Matmul::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::math::Matmul::drop(ptr: usize)
pub unsafe fn vyre_libs::math::Matmul::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::math::Matmul
impl<T> tracing::instrument::Instrument for vyre_libs::math::Matmul
impl<T> tracing::instrument::WithSubscriber for vyre_libs::math::Matmul
impl<T> typenum::type_operators::Same for vyre_libs::math::Matmul
pub type vyre_libs::math::Matmul::Output = T
pub struct vyre_libs::prelude::MatmulTiled
impl vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::clone(&self) -> vyre_libs::MatmulTiled
impl core::fmt::Debug for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulTiled
impl core::marker::Send for vyre_libs::MatmulTiled
impl core::marker::Sync for vyre_libs::MatmulTiled
impl core::marker::Unpin for vyre_libs::MatmulTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulTiled where T: core::clone::Clone
pub type vyre_libs::MatmulTiled::Owned = T
pub fn vyre_libs::MatmulTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Init = T
pub const vyre_libs::MatmulTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Output = T
#[non_exhaustive] pub struct vyre_libs::prelude::TensorRef
pub vyre_libs::prelude::TensorRef::dtype: vyre_spec::data_type::DataType
pub vyre_libs::prelude::TensorRef::name: vyre_foundation::ir_inner::model::expr::Ident
pub vyre_libs::prelude::TensorRef::shape: alloc::sync::Arc<[u32]>
impl vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::element_count(&self) -> core::option::Option<u32>
pub fn vyre_libs::tensor_ref::TensorRef::f16_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f16_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f32_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f32_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::name_str(&self) -> &str
pub fn vyre_libs::tensor_ref::TensorRef::new(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, dtype: vyre_spec::data_type::DataType, shape: alloc::vec::Vec<u32>) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::u32_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::u32_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
impl core::clone::Clone for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::clone(&self) -> vyre_libs::tensor_ref::TensorRef
impl core::cmp::Eq for vyre_libs::tensor_ref::TensorRef
impl core::cmp::PartialEq for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::eq(&self, other: &vyre_libs::tensor_ref::TensorRef) -> bool
impl core::fmt::Debug for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_libs::tensor_ref::TensorRef
impl core::marker::Freeze for vyre_libs::tensor_ref::TensorRef
impl core::marker::Send for vyre_libs::tensor_ref::TensorRef
impl core::marker::Sync for vyre_libs::tensor_ref::TensorRef
impl core::marker::Unpin for vyre_libs::tensor_ref::TensorRef
impl core::marker::UnsafeUnpin for vyre_libs::tensor_ref::TensorRef
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::tensor_ref::TensorRef
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::tensor_ref::TensorRef
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::tensor_ref::TensorRef where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::tensor_ref::TensorRef where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::From<T>
pub fn vyre_libs::tensor_ref::TensorRef::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::Into<T>
pub type vyre_libs::tensor_ref::TensorRef::Error = core::convert::Infallible
pub fn vyre_libs::tensor_ref::TensorRef::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::TryFrom<T>
pub type vyre_libs::tensor_ref::TensorRef::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::tensor_ref::TensorRef::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::tensor_ref::TensorRef where T: core::clone::Clone
pub type vyre_libs::tensor_ref::TensorRef::Owned = T
pub fn vyre_libs::tensor_ref::TensorRef::clone_into(&self, target: &mut T)
pub fn vyre_libs::tensor_ref::TensorRef::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::tensor_ref::TensorRef where T: 'static + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::tensor_ref::TensorRef where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::tensor_ref::TensorRef where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::tensor_ref::TensorRef where T: core::clone::Clone
pub unsafe fn vyre_libs::tensor_ref::TensorRef::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::tensor_ref::TensorRef
pub type vyre_libs::tensor_ref::TensorRef::Init = T
pub const vyre_libs::tensor_ref::TensorRef::ALIGN: usize
pub unsafe fn vyre_libs::tensor_ref::TensorRef::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::tensor_ref::TensorRef::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::tensor_ref::TensorRef::drop(ptr: usize)
pub unsafe fn vyre_libs::tensor_ref::TensorRef::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::tensor_ref::TensorRef
impl<T> tracing::instrument::Instrument for vyre_libs::tensor_ref::TensorRef
impl<T> tracing::instrument::WithSubscriber for vyre_libs::tensor_ref::TensorRef
impl<T> typenum::type_operators::Same for vyre_libs::tensor_ref::TensorRef
pub type vyre_libs::tensor_ref::TensorRef::Output = T
pub fn vyre_libs::prelude::aho_corasick(haystack: &str, transitions: &str, accept: &str, matches: &str, haystack_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::base64_decode(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::broadcast(src: &str, dst: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::check_dtype(r: &vyre_libs::tensor_ref::TensorRef, expected: vyre_spec::data_type::DataType, op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::prelude::check_shape(r: &vyre_libs::tensor_ref::TensorRef, expected: &[u32], op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::prelude::check_tensors(op: &'static str, tensors: &[(&vyre_libs::tensor_ref::TensorRef, vyre_spec::data_type::DataType)]) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::prelude::check_unique_names(refs: &[&vyre_libs::tensor_ref::TensorRef], op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::prelude::dot(lhs: &str, rhs: &str, out: &str, n: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::prelude::hex_decode(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::inflate(input: &str, output: &str, input_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::layer_norm(input: &str, output: &str, n: u32, eps: f32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::linear(x: &str, w: &str, b: &str, out: &str, in_dim: u32, out_dim: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::prelude::matmul(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::matmul_tiled(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::relu(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::scan_prefix_sum(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::substring_search(haystack: &str, needle: &str, matches: &str, haystack_len: u32, needle_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::prelude::ziftsieve_gpu(input: &str, output: &str, seq_literal_start: &str, seq_literal_len: &str, seq_literal_offset: &str, input_len: u32, seq_count: u32, max_output: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::range_ordering
pub const vyre_libs::range_ordering::MAX_CACHED_POSITIONS: u32
pub const vyre_libs::range_ordering::MAX_DEPTH: u32
pub fn vyre_libs::range_ordering::match_order(left_id: vyre_foundation::ir_inner::model::generated::Expr, right_id: vyre_foundation::ir_inner::model::generated::Expr, res_name: &str) -> (alloc::vec::Vec<vyre_foundation::ir_inner::model::generated::Node>, vyre_foundation::ir_inner::model::generated::Expr)
pub mod vyre_libs::region
pub use vyre_libs::region::reparent_program_children
pub use vyre_libs::region::tag_program
pub use vyre_libs::region::wrap
pub use vyre_libs::region::wrap_anonymous
pub use vyre_libs::region::wrap_child
pub mod vyre_libs::representation
pub mod vyre_libs::representation::unpack
pub fn vyre_libs::representation::unpack::unpack_4bit_f32(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::representation::unpack_4bit_f32(input: &str, output: &str, n: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::scan
pub use vyre_libs::scan::CompiledDfa
pub use vyre_libs::scan::DEFAULT_DFA_BUDGET_BYTES
pub use vyre_libs::scan::DfaCompileError
pub use vyre_libs::scan::FusionError
pub use vyre_libs::scan::LiteralMatch
pub use vyre_libs::scan::RegionTriple
pub use vyre_libs::scan::dedup_regions_flag_program
pub use vyre_libs::scan::dedup_regions_inplace
pub use vyre_libs::scan::dfa_compile
pub use vyre_libs::scan::dfa_compile_with_budget
pub use vyre_libs::scan::fuse_programs
pub use vyre_libs::scan::fuse_programs_vec
pub mod vyre_libs::scan::builders
pub fn vyre_libs::scan::builders::append_match(hits_buffer: &str, count_buffer: &str, tag: impl core::convert::Into<vyre_foundation::ir_inner::model::generated::Expr>, start: impl core::convert::Into<vyre_foundation::ir_inner::model::generated::Expr>, end: impl core::convert::Into<vyre_foundation::ir_inner::model::generated::Expr>) -> vyre_foundation::ir_inner::model::generated::Node
pub fn vyre_libs::scan::builders::append_match_subgroup(hits_buffer: &str, count_buffer: &str, tag: impl core::convert::Into<vyre_foundation::ir_inner::model::generated::Expr>, start: impl core::convert::Into<vyre_foundation::ir_inner::model::generated::Expr>, end: impl core::convert::Into<vyre_foundation::ir_inner::model::generated::Expr>, cond: vyre_foundation::ir_inner::model::generated::Expr) -> alloc::vec::Vec<vyre_foundation::ir_inner::model::generated::Node>
pub fn vyre_libs::scan::builders::load_packed_byte(haystack: &str, idx: vyre_foundation::ir_inner::model::generated::Expr) -> (vyre_foundation::ir_inner::model::generated::Node, vyre_foundation::ir_inner::model::generated::Expr)
pub fn vyre_libs::scan::builders::load_packed_byte_expr(haystack: &str, idx: vyre_foundation::ir_inner::model::generated::Expr) -> vyre_foundation::ir_inner::model::generated::Expr
pub mod vyre_libs::scan::classic_ac
pub struct vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub vyre_libs::scan::classic_ac::ClassicAcAutomaton::dfa: vyre_primitives::matching::dfa_compile::CompiledDfa
impl core::clone::Clone for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::clone(&self) -> vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::fmt::Debug for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::marker::Send for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::marker::Sync for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::marker::Unpin for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::marker::UnsafeUnpin for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl<T, U> core::convert::Into<U> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where U: core::convert::From<T>
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where U: core::convert::Into<T>
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Error = core::convert::Infallible
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: core::clone::Clone
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Owned = T
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: ?core::marker::Sized
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: ?core::marker::Sized
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::classic_ac::ClassicAcAutomaton where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Init = T
pub const vyre_libs::scan::classic_ac::ClassicAcAutomaton::ALIGN: usize
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::classic_ac::ClassicAcAutomaton::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl<T> tracing::instrument::Instrument for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::classic_ac::ClassicAcAutomaton
impl<T> typenum::type_operators::Same for vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub type vyre_libs::scan::classic_ac::ClassicAcAutomaton::Output = T
pub const vyre_libs::scan::classic_ac::CLASSIC_AC_SUFFIX2_MASK_WORDS: usize
pub const vyre_libs::scan::classic_ac::CLASSIC_AC_SUFFIX3_BLOOM_WORDS: usize
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_count_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_count_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_count_suffix2_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_count_suffix3_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_ranges_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_ranges_prefilter_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_ranges_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_ranges_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_ranges_suffix3_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::build_ac_bounded_ranges_suffix3_prefilter_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_count_prefilter_program(haystack: &str, transitions: &str, output_offsets: &str, candidate_end_mask: &str, haystack_len: &str, match_count: &str, state_count: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_count_program(haystack: &str, transitions: &str, output_offsets: &str, haystack_len: &str, match_count: &str, state_count: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_count_suffix2_prefilter_program(haystack: &str, transitions: &str, output_offsets: &str, candidate_end_mask: &str, candidate_suffix2_mask: &str, haystack_len: &str, match_count: &str, state_count: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_count_suffix3_prefilter_program(haystack: &str, transitions: &str, output_offsets: &str, candidate_end_mask: &str, candidate_suffix2_mask: &str, candidate_suffix3_bloom: &str, haystack_len: &str, match_count: &str, state_count: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_ranges_prefilter_program(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, candidate_end_mask: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_ranges_prefilter_program_ext(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, candidate_end_mask: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_ranges_program(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_ranges_program_ext(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_ranges_scan(ac: &vyre_libs::scan::classic_ac::ClassicAcAutomaton, pattern_lengths: &[u32], haystack: &[u8]) -> alloc::vec::Vec<(u32, u32, u32)>
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_ranges_suffix3_prefilter_program(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, candidate_end_mask: &str, candidate_suffix2_mask: &str, candidate_suffix3_bloom: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_ranges_suffix3_prefilter_program_ext(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, match_count: &str, candidate_end_mask: &str, candidate_suffix2_mask: &str, candidate_suffix3_bloom: &str, matches: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_matches: u32, max_pattern_len: u32, use_subgroup_coalesce: bool) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_bounded_ranges_suffix3_presence_program_ext(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, pattern_lengths: &str, haystack_len: &str, presence: &str, candidate_end_mask: &str, candidate_suffix2_mask: &str, candidate_suffix3_bloom: &str, state_count: u32, output_records_len: u32, pattern_count: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_candidate_end_byte_mask_words(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> [u32; 8]
pub fn vyre_libs::scan::classic_ac::classic_ac_candidate_suffix2_mask_words(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa) -> [u32; 2048]
pub fn vyre_libs::scan::classic_ac::classic_ac_candidate_suffix3_bloom_words(patterns: &[&[u8]]) -> alloc::vec::Vec<u32>
pub fn vyre_libs::scan::classic_ac::classic_ac_compile(patterns: &[&[u8]]) -> vyre_libs::scan::classic_ac::ClassicAcAutomaton
pub fn vyre_libs::scan::classic_ac::classic_ac_program(haystack: &str, transitions: &str, output_offsets: &str, output_records: &str, match_count: &str, matches: &str, haystack_len: u32, state_count: u32, output_records_len: u32, max_matches: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::classic_ac::classic_ac_scan(ac: &vyre_libs::scan::classic_ac::ClassicAcAutomaton, haystack: &[u8]) -> alloc::vec::Vec<(u32, u32)>
pub fn vyre_libs::scan::classic_ac::classic_ac_scan_counts(ac: &vyre_libs::scan::classic_ac::ClassicAcAutomaton, haystack: &[u8]) -> alloc::vec::Vec<u32>
pub fn vyre_libs::scan::classic_ac::classic_ac_suffix3_bloom_contains(mask: &[u32], previous2: u8, previous: u8, current: u8) -> bool
pub fn vyre_libs::scan::classic_ac::presence_bitmap_words(pattern_count: u32) -> u32
pub fn vyre_libs::scan::classic_ac::try_build_ac_bounded_ranges_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::scan::classic_ac::try_build_ac_bounded_ranges_prefilter_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::scan::classic_ac::try_build_ac_bounded_ranges_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::scan::classic_ac::try_build_ac_bounded_ranges_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::scan::classic_ac::try_build_ac_bounded_ranges_suffix3_prefilter_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::scan::classic_ac::try_build_ac_bounded_ranges_suffix3_prefilter_program_ext(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32, max_matches: u32, use_subgroup_coalesce: bool) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub fn vyre_libs::scan::classic_ac::try_build_ac_bounded_ranges_suffix3_presence_program(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, pattern_count: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, alloc::string::String>
pub mod vyre_libs::scan::dfa
pub use vyre_libs::scan::dfa::CompiledDfa
pub use vyre_libs::scan::dfa::DEFAULT_DFA_BUDGET_BYTES
pub use vyre_libs::scan::dfa::DfaCompileError
pub use vyre_libs::scan::dfa::dfa_compile
pub use vyre_libs::scan::dfa::dfa_compile_with_budget
pub fn vyre_libs::scan::dfa::aho_corasick(haystack: &str, transitions: &str, accept: &str, matches: &str, haystack_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::dfa::aho_corasick_bounded(haystack: &str, transitions: &str, accept: &str, matches: &str, haystack_len: u32, state_count: u32, max_pattern_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::dfa::cooperative_dfa_scan(input: &str, transitions: &str, accept_mask: &str, matches: &str, input_len: u32, state_count: u32, subgroup_size: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::dfa::cooperative_dfa_scan_body_with_store(input: &str, transitions: &str, accept_mask: &str, matches: &str, subgroup_size: u32, store_value: vyre_foundation::ir_inner::model::generated::Expr) -> alloc::vec::Vec<vyre_foundation::ir_inner::model::generated::Node>
pub mod vyre_libs::scan::direct_gpu
pub use vyre_libs::scan::direct_gpu::Match
pub struct vyre_libs::scan::direct_gpu::DirectGpuScanner
impl vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::compile(patterns: &[&[u8]]) -> Self
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::literal_set_cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::program(&self) -> &vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl !core::marker::Freeze for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Send for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Sync for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Unpin for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::UnsafeUnpin for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T, U> core::convert::Into<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::From<T>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::Into<T>
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Error = core::convert::Infallible
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Init = T
pub const vyre_libs::scan::direct_gpu::DirectGpuScanner::ALIGN: usize
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> tracing::instrument::Instrument for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> typenum::type_operators::Same for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Output = T
pub mod vyre_libs::scan::dispatch_io
pub struct vyre_libs::scan::dispatch_io::ScanDispatchScratch
pub vyre_libs::scan::dispatch_io::ScanDispatchScratch::haystack_bytes: alloc::vec::Vec<u8>
pub vyre_libs::scan::dispatch_io::ScanDispatchScratch::hit_bytes: alloc::vec::Vec<u8>
impl core::default::Default for vyre_libs::scan::dispatch_io::ScanDispatchScratch
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::default() -> vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::fmt::Debug for vyre_libs::scan::dispatch_io::ScanDispatchScratch
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::marker::Send for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::marker::Sync for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::marker::Unpin for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::marker::UnsafeUnpin for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl<T, U> core::convert::Into<U> for vyre_libs::scan::dispatch_io::ScanDispatchScratch where U: core::convert::From<T>
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::dispatch_io::ScanDispatchScratch where U: core::convert::Into<T>
pub type vyre_libs::scan::dispatch_io::ScanDispatchScratch::Error = core::convert::Infallible
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::dispatch_io::ScanDispatchScratch where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::dispatch_io::ScanDispatchScratch::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::dispatch_io::ScanDispatchScratch where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::dispatch_io::ScanDispatchScratch where T: ?core::marker::Sized
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::dispatch_io::ScanDispatchScratch where T: ?core::marker::Sized
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::dispatch_io::ScanDispatchScratch
pub fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::dispatch_io::ScanDispatchScratch
pub type vyre_libs::scan::dispatch_io::ScanDispatchScratch::Init = T
pub const vyre_libs::scan::dispatch_io::ScanDispatchScratch::ALIGN: usize
pub unsafe fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::dispatch_io::ScanDispatchScratch::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl<T> tracing::instrument::Instrument for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl<T> typenum::type_operators::Same for vyre_libs::scan::dispatch_io::ScanDispatchScratch
pub type vyre_libs::scan::dispatch_io::ScanDispatchScratch::Output = T
pub const vyre_libs::scan::dispatch_io::DEFAULT_MAX_SCAN_BYTES: u32
pub fn vyre_libs::scan::dispatch_io::byte_scan_dispatch_config(haystack_len: u32, workgroup_x: u32) -> vyre_driver::backend::dispatch_config::DispatchConfig
pub fn vyre_libs::scan::dispatch_io::candidate_start_dispatch_config(haystack_len: u32) -> vyre_driver::backend::dispatch_config::DispatchConfig
pub fn vyre_libs::scan::dispatch_io::haystack_len_u32(haystack: &[u8], context: &str) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::haystack_padded_u32_byte_len(byte_len: usize) -> core::result::Result<usize, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::pack_haystack_u32(haystack: &[u8]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::scan::dispatch_io::pack_haystack_u32_into(haystack: &[u8], packed: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::pack_u32_slice(words: &[u32]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::scan::dispatch_io::scan_guard(haystack: &[u8], context: &str, max_bytes: u32) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::try_output_bytes<'a>(outputs: &'a [alloc::vec::Vec<u8>], index: usize, field: &'static str) -> core::result::Result<&'a [u8], vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::try_pack_haystack_u32(haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<u8>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::try_read_u32_prefix(bytes: &[u8], field: &'static str) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::try_unpack_match_triples(triples_bytes: &[u8], count: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::try_unpack_match_triples_exact_prefix_into(triples_bytes: &[u8], count: u32, results: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::try_unpack_match_triples_into(triples_bytes: &[u8], count: u32, results: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::dispatch_io::u32_words_as_le_bytes(words: &[u32]) -> alloc::borrow::Cow<'_, [u8]>
pub fn vyre_libs::scan::dispatch_io::unpack_match_triples(triples_bytes: &[u8], count: u32) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::dispatch_io::unpack_match_triples_into(triples_bytes: &[u8], count: u32, results: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>)
pub mod vyre_libs::scan::engine
pub struct vyre_libs::scan::engine::ScanResult
pub vyre_libs::scan::engine::ScanResult::cache_hit: bool
pub vyre_libs::scan::engine::ScanResult::elapsed: core::time::Duration
pub vyre_libs::scan::engine::ScanResult::matches: alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub vyre_libs::scan::engine::ScanResult::truncated: bool
impl vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::from_matches(matches: alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> Self
pub fn vyre_libs::scan::engine::ScanResult::is_empty(&self) -> bool
pub fn vyre_libs::scan::engine::ScanResult::len(&self) -> usize
impl core::clone::Clone for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::clone(&self) -> vyre_libs::scan::engine::ScanResult
impl core::default::Default for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::default() -> vyre_libs::scan::engine::ScanResult
impl core::fmt::Debug for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::engine::ScanResult
impl core::marker::Send for vyre_libs::scan::engine::ScanResult
impl core::marker::Sync for vyre_libs::scan::engine::ScanResult
impl core::marker::Unpin for vyre_libs::scan::engine::ScanResult
impl core::marker::UnsafeUnpin for vyre_libs::scan::engine::ScanResult
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::engine::ScanResult
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::engine::ScanResult
impl<T, U> core::convert::Into<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::From<T>
pub fn vyre_libs::scan::engine::ScanResult::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::Into<T>
pub type vyre_libs::scan::engine::ScanResult::Error = core::convert::Infallible
pub fn vyre_libs::scan::engine::ScanResult::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::engine::ScanResult::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::engine::ScanResult::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::engine::ScanResult where T: core::clone::Clone
pub type vyre_libs::scan::engine::ScanResult::Owned = T
pub fn vyre_libs::scan::engine::ScanResult::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::engine::ScanResult::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::engine::ScanResult where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::engine::ScanResult where T: ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::engine::ScanResult where T: ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::engine::ScanResult where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::engine::ScanResult::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::engine::ScanResult
pub type vyre_libs::scan::engine::ScanResult::Init = T
pub const vyre_libs::scan::engine::ScanResult::ALIGN: usize
pub unsafe fn vyre_libs::scan::engine::ScanResult::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::engine::ScanResult::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::engine::ScanResult::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::engine::ScanResult::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::engine::ScanResult
impl<T> tracing::instrument::Instrument for vyre_libs::scan::engine::ScanResult
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::engine::ScanResult
impl<T> typenum::type_operators::Same for vyre_libs::scan::engine::ScanResult
pub type vyre_libs::scan::engine::ScanResult::Output = T
pub trait vyre_libs::scan::engine::MatchEngineCache: core::marker::Sized
pub type vyre_libs::scan::engine::MatchEngineCache::WireError: core::fmt::Display + core::fmt::Debug
pub const vyre_libs::scan::engine::MatchEngineCache::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::scan::engine::MatchEngineCache::WIRE_VERSION: u32
pub fn vyre_libs::scan::engine::MatchEngineCache::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::scan::engine::MatchEngineCache::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
impl vyre_libs::scan::engine::MatchEngineCache for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::WireError = vyre_libs::scan::literal_set::LiteralSetWireError
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_VERSION: u32
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
pub trait vyre_libs::scan::engine::MatchScan
pub fn vyre_libs::scan::engine::MatchScan::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::engine::MatchScan::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::engine::MatchScan::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::engine::cache_path(cache_dir: &std::path::Path, cache_key: &str) -> core::option::Option<std::path::PathBuf>
pub fn vyre_libs::scan::engine::cached_load_or_compile<E, F>(cache_dir: &std::path::Path, cache_key: &str, compile: F) -> E where E: vyre_libs::scan::engine::MatchEngineCache, F: core::ops::function::FnOnce() -> E
pub mod vyre_libs::scan::hit_buffer
pub const vyre_libs::scan::hit_buffer::HIT_BUFFER_LIVE_LENGTH: &str
pub const vyre_libs::scan::hit_buffer::HIT_BUFFER_OVERFLOW_COUNT: &str
pub fn vyre_libs::scan::hit_buffer::compact_hits(out_hits: &str, out_cursor: &str, max_capacity: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::hit_buffer::compact_hits_with_layout(out_hits: &str, out_cursor: &str, hit_capacity: u32, max_capacity: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::hit_buffer::emit_hit(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::hit_buffer::emit_hit_then_compact(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::execution_plan::fusion::FusionError>
pub fn vyre_libs::scan::hit_buffer::emit_hit_then_compact_with_layout(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str, lane_count: u32, max_hits: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::execution_plan::fusion::FusionError>
pub fn vyre_libs::scan::hit_buffer::emit_hit_with_layout(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str, lane_count: u32, max_hits: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub mod vyre_libs::scan::literal_set
pub use vyre_libs::scan::literal_set::Match
pub enum vyre_libs::scan::literal_set::LiteralSetCompileError
pub vyre_libs::scan::literal_set::LiteralSetCompileError::DispatchProgramBuildFailed
pub vyre_libs::scan::literal_set::LiteralSetCompileError::DispatchProgramBuildFailed::message: alloc::string::String
pub vyre_libs::scan::literal_set::LiteralSetCompileError::PatternByteCountExceedsGpuAbi
pub vyre_libs::scan::literal_set::LiteralSetCompileError::PatternByteCountExceedsGpuAbi::count: usize
pub vyre_libs::scan::literal_set::LiteralSetCompileError::PatternByteCountOverflow
pub vyre_libs::scan::literal_set::LiteralSetCompileError::PatternCountOverflow
pub vyre_libs::scan::literal_set::LiteralSetCompileError::PatternCountOverflow::count: usize
pub vyre_libs::scan::literal_set::LiteralSetCompileError::PatternLengthOverflow
pub vyre_libs::scan::literal_set::LiteralSetCompileError::PatternLengthOverflow::len: usize
pub vyre_libs::scan::literal_set::LiteralSetCompileError::PatternLengthOverflow::pattern_index: usize
pub vyre_libs::scan::literal_set::LiteralSetCompileError::StorageReserveFailed
pub vyre_libs::scan::literal_set::LiteralSetCompileError::StorageReserveFailed::field: &'static str
pub vyre_libs::scan::literal_set::LiteralSetCompileError::StorageReserveFailed::message: alloc::string::String
pub vyre_libs::scan::literal_set::LiteralSetCompileError::StorageReserveFailed::requested: usize
impl core::error::Error for vyre_libs::scan::literal_set::LiteralSetCompileError
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetCompileError
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::scan::literal_set::LiteralSetCompileError
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetCompileError
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetCompileError
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetCompileError
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetCompileError
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetCompileError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetCompileError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetCompileError
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetCompileError where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetCompileError where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetCompileError::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetCompileError where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetCompileError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::string::ToString for vyre_libs::scan::literal_set::LiteralSetCompileError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetCompileError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetCompileError where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetCompileError where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetCompileError
pub fn vyre_libs::scan::literal_set::LiteralSetCompileError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetCompileError
pub type vyre_libs::scan::literal_set::LiteralSetCompileError::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetCompileError::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetCompileError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetCompileError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetCompileError::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetCompileError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetCompileError
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetCompileError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetCompileError
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetCompileError
pub type vyre_libs::scan::literal_set::LiteralSetCompileError::Output = T
#[non_exhaustive] pub enum vyre_libs::scan::literal_set::LiteralSetWireError
pub vyre_libs::scan::literal_set::LiteralSetWireError::InvalidDfa(vyre_primitives::matching::dfa_compile::DfaWireError)
pub vyre_libs::scan::literal_set::LiteralSetWireError::InvalidProgram(alloc::string::String)
pub vyre_libs::scan::literal_set::LiteralSetWireError::WireFraming(vyre_foundation::serial::envelope::EnvelopeError)
impl core::error::Error for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::string::ToString for vyre_libs::scan::literal_set::LiteralSetWireError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetWireError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetWireError where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetWireError where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetWireError
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetWireError::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetWireError
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Output = T
pub struct vyre_libs::scan::literal_set::GpuLiteralSet
pub vyre_libs::scan::literal_set::GpuLiteralSet::dfa: vyre_primitives::matching::dfa_compile::CompiledDfa
pub vyre_libs::scan::literal_set::GpuLiteralSet::pattern_bytes: alloc::vec::Vec<u32>
pub vyre_libs::scan::literal_set::GpuLiteralSet::pattern_lengths: alloc::vec::Vec<u32>
pub vyre_libs::scan::literal_set::GpuLiteralSet::pattern_offsets: alloc::vec::Vec<u32>
pub vyre_libs::scan::literal_set::GpuLiteralSet::program: vyre_foundation::ir_inner::model::program::core::Program
impl vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::compile(patterns: &[&[u8]]) -> Self
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::count<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8]) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::count_with_literal_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, vyre_libs::scan::literal_set::LiteralSetWireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_count_dispatch(&self, haystack: &[u8]) -> core::result::Result<vyre_libs::scan::literal_set::LiteralSetPreparedCount, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_count_scratch(&self, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_literal_scratch(&self, max_matches: u32, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_scan_dispatch(&self, haystack: &[u8], max_matches: u32) -> core::result::Result<vyre_libs::scan::literal_set::LiteralSetPreparedScan, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into_with_literal_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into_with_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, scratch: &mut vyre_libs::scan::dispatch_io::ScanDispatchScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_presence<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<u32>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_presence_with_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], scratch: &mut vyre_libs::scan::dispatch_io::ScanDispatchScratch) -> core::result::Result<alloc::vec::Vec<u32>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, vyre_libs::scan::literal_set::LiteralSetWireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_compile(patterns: &[&[u8]]) -> core::result::Result<Self, vyre_libs::scan::literal_set::LiteralSetCompileError>
impl vyre_libs::scan::engine::MatchEngineCache for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::WireError = vyre_libs::scan::literal_set::LiteralSetWireError
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_VERSION: u32
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl !core::marker::Freeze for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Send for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Sync for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Unpin for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::GpuLiteralSet
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::GpuLiteralSet
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::literal_set::GpuLiteralSet where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::GpuLiteralSet where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::GpuLiteralSet where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Init = T
pub const vyre_libs::scan::literal_set::GpuLiteralSet::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Output = T
pub struct vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub vyre_libs::scan::literal_set::LiteralSetPreparedCount::dispatch_config: vyre_driver::backend::dispatch_config::DispatchConfig
pub vyre_libs::scan::literal_set::LiteralSetPreparedCount::encoded_input_bytes: u64
pub vyre_libs::scan::literal_set::LiteralSetPreparedCount::haystack_len: u32
pub vyre_libs::scan::literal_set::LiteralSetPreparedCount::inputs: alloc::vec::Vec<alloc::vec::Vec<u8>>
pub vyre_libs::scan::literal_set::LiteralSetPreparedCount::program: vyre_foundation::ir_inner::model::program::core::Program
impl vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub const fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::count_readback_bytes(&self) -> usize
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::decode_outputs(&self, outputs: &[alloc::vec::Vec<u8>]) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
impl core::clone::Clone for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::clone(&self) -> vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl !core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: core::clone::Clone
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Owned = T
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetPreparedCount::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Output = T
pub struct vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub vyre_libs::scan::literal_set::LiteralSetPreparedScan::dispatch_config: vyre_driver::backend::dispatch_config::DispatchConfig
pub vyre_libs::scan::literal_set::LiteralSetPreparedScan::encoded_input_bytes: u64
pub vyre_libs::scan::literal_set::LiteralSetPreparedScan::haystack_len: u32
pub vyre_libs::scan::literal_set::LiteralSetPreparedScan::inputs: alloc::vec::Vec<alloc::vec::Vec<u8>>
pub vyre_libs::scan::literal_set::LiteralSetPreparedScan::matches_output_bytes: usize
pub vyre_libs::scan::literal_set::LiteralSetPreparedScan::max_matches: u32
pub vyre_libs::scan::literal_set::LiteralSetPreparedScan::program: vyre_foundation::ir_inner::model::program::core::Program
impl vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::decode_outputs_into(&self, outputs: &[alloc::vec::Vec<u8>], matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub const fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::match_count_readback_bytes(&self) -> usize
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::match_triples_readback_bytes(&self, match_count: u32) -> core::result::Result<usize, vyre_driver::backend::error::BackendError>
impl core::clone::Clone for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::clone(&self) -> vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl !core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: core::clone::Clone
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Owned = T
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetPreparedScan::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Output = T
pub struct vyre_libs::scan::literal_set::LiteralSetScanScratch
pub vyre_libs::scan::literal_set::LiteralSetScanScratch::dispatch: vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::default::Default for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::default() -> vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl !core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetScanScratch where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetScanScratch where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetScanScratch::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetScanScratch where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetScanScratch::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetScanScratch where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetScanScratch where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetScanScratch where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub type vyre_libs::scan::literal_set::LiteralSetScanScratch::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetScanScratch::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetScanScratch::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetScanScratch::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetScanScratch::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetScanScratch::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub type vyre_libs::scan::literal_set::LiteralSetScanScratch::Output = T
pub const vyre_libs::scan::literal_set::LITERAL_SET_COUNT_RESET_RESOURCE_INDICES: [usize; 1]
pub const vyre_libs::scan::literal_set::LITERAL_SET_COUNT_RESOURCE_INDEX: usize
pub const vyre_libs::scan::literal_set::LITERAL_SET_COUNT_SCAN_RESOURCE_INDICES: [usize; 8]
pub const vyre_libs::scan::literal_set::LITERAL_SET_MATCHES_RESOURCE_INDEX: usize
pub const vyre_libs::scan::literal_set::LITERAL_SET_MATCH_COUNT_RESOURCE_INDEX: usize
pub const vyre_libs::scan::literal_set::LITERAL_SET_RESET_RESOURCE_INDICES: [usize; 1]
pub const vyre_libs::scan::literal_set::LITERAL_SET_SCAN_RESOURCE_INDICES: [usize; 11]
pub fn vyre_libs::scan::literal_set::dfa_to_jit_ir(dfa: &vyre_primitives::matching::dfa_compile::CompiledDfa, state_var: &str, byte_expr: vyre_foundation::ir_inner::model::generated::Expr) -> vyre_foundation::ir_inner::model::generated::Node
pub type vyre_libs::scan::literal_set::LiteralMatch = vyre_foundation::runtime::match_result::Match
pub mod vyre_libs::scan::pipeline
pub struct vyre_libs::scan::pipeline::Pipeline<E>
pub vyre_libs::scan::pipeline::Pipeline::engine: E
pub vyre_libs::scan::pipeline::Pipeline::post_process: vyre_libs::scan::pipeline::PostProcessFn
impl<E: vyre_libs::scan::engine::MatchScan> vyre_libs::scan::pipeline::Pipeline<E>
pub const fn vyre_libs::scan::pipeline::Pipeline<E>::new(engine: E) -> Self
pub fn vyre_libs::scan::pipeline::Pipeline<E>::reference_scan_processed(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::scan_processed(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_reference_scan_processed(&self, haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub const fn vyre_libs::scan::pipeline::Pipeline<E>::with_post_process(engine: E, post_process: vyre_libs::scan::pipeline::PostProcessFn) -> Self
impl<E: core::clone::Clone> core::clone::Clone for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::clone(&self) -> Self
impl<E: core::fmt::Debug> core::fmt::Debug for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl<E> core::marker::Freeze for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Freeze
impl<E> core::marker::Send for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Send
impl<E> core::marker::Sync for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Sync
impl<E> core::marker::Unpin for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Unpin
impl<E> core::marker::UnsafeUnpin for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::UnsafeUnpin
impl<E> core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::pipeline::Pipeline<E> where E: core::panic::unwind_safe::RefUnwindSafe
impl<E> core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::pipeline::Pipeline<E> where E: core::panic::unwind_safe::UnwindSafe
impl<T, U> core::convert::Into<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::From<T>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::Into<T>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Error = core::convert::Infallible
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::pipeline::Pipeline<E> where T: core::clone::Clone
pub type vyre_libs::scan::pipeline::Pipeline<E>::Owned = T
pub fn vyre_libs::scan::pipeline::Pipeline<E>::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::pipeline::Pipeline<E>::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::pipeline::Pipeline<E> where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::pipeline::Pipeline<E> where T: ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::pipeline::Pipeline<E> where T: ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::pipeline::Pipeline<E> where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::pipeline::Pipeline<E>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Init = T
pub const vyre_libs::scan::pipeline::Pipeline<E>::ALIGN: usize
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> tracing::instrument::Instrument for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> typenum::type_operators::Same for vyre_libs::scan::pipeline::Pipeline<E>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Output = T
pub type vyre_libs::scan::pipeline::PostProcessFn = fn(&[vyre_foundation::runtime::match_result::Match], &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub mod vyre_libs::scan::post_process
pub enum vyre_libs::scan::post_process::PostProcessError
pub vyre_libs::scan::post_process::PostProcessError::InvalidRange
pub vyre_libs::scan::post_process::PostProcessError::InvalidRange::end: u32
pub vyre_libs::scan::post_process::PostProcessError::InvalidRange::haystack_len: usize
pub vyre_libs::scan::post_process::PostProcessError::InvalidRange::pattern_id: u32
pub vyre_libs::scan::post_process::PostProcessError::InvalidRange::start: u32
impl core::clone::Clone for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::clone(&self) -> vyre_libs::scan::post_process::PostProcessError
impl core::cmp::Eq for vyre_libs::scan::post_process::PostProcessError
impl core::cmp::PartialEq for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::eq(&self, other: &vyre_libs::scan::post_process::PostProcessError) -> bool
impl core::error::Error for vyre_libs::scan::post_process::PostProcessError
impl core::fmt::Debug for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::scan::post_process::PostProcessError
impl core::marker::StructuralPartialEq for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Freeze for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Send for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Sync for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Unpin for vyre_libs::scan::post_process::PostProcessError
impl core::marker::UnsafeUnpin for vyre_libs::scan::post_process::PostProcessError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::post_process::PostProcessError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::post_process::PostProcessError
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::scan::post_process::PostProcessError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::scan::post_process::PostProcessError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::From<T>
pub fn vyre_libs::scan::post_process::PostProcessError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::Into<T>
pub type vyre_libs::scan::post_process::PostProcessError::Error = core::convert::Infallible
pub fn vyre_libs::scan::post_process::PostProcessError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::post_process::PostProcessError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::post_process::PostProcessError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::post_process::PostProcessError where T: core::clone::Clone
pub type vyre_libs::scan::post_process::PostProcessError::Owned = T
pub fn vyre_libs::scan::post_process::PostProcessError::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::post_process::PostProcessError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_libs::scan::post_process::PostProcessError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::scan::post_process::PostProcessError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::post_process::PostProcessError where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::post_process::PostProcessError where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::post_process::PostProcessError where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::post_process::PostProcessError
pub type vyre_libs::scan::post_process::PostProcessError::Init = T
pub const vyre_libs::scan::post_process::PostProcessError::ALIGN: usize
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::post_process::PostProcessError
impl<T> tracing::instrument::Instrument for vyre_libs::scan::post_process::PostProcessError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::post_process::PostProcessError
impl<T> typenum::type_operators::Same for vyre_libs::scan::post_process::PostProcessError
pub type vyre_libs::scan::post_process::PostProcessError::Output = T
pub struct vyre_libs::scan::post_process::PostProcessedMatch
pub vyre_libs::scan::post_process::PostProcessedMatch::confidence: f32
pub vyre_libs::scan::post_process::PostProcessedMatch::end: u32
pub vyre_libs::scan::post_process::PostProcessedMatch::entropy_bits_per_byte: f32
pub vyre_libs::scan::post_process::PostProcessedMatch::pattern_id: u32
pub vyre_libs::scan::post_process::PostProcessedMatch::start: u32
impl core::clone::Clone for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::clone(&self) -> vyre_libs::scan::post_process::PostProcessedMatch
impl core::cmp::PartialEq for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::eq(&self, other: &vyre_libs::scan::post_process::PostProcessedMatch) -> bool
impl core::fmt::Debug for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::StructuralPartialEq for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Freeze for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Send for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Sync for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Unpin for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::UnsafeUnpin for vyre_libs::scan::post_process::PostProcessedMatch
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::post_process::PostProcessedMatch
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::post_process::PostProcessedMatch
impl<T, U> core::convert::Into<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::From<T>
pub fn vyre_libs::scan::post_process::PostProcessedMatch::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::Into<T>
pub type vyre_libs::scan::post_process::PostProcessedMatch::Error = core::convert::Infallible
pub fn vyre_libs::scan::post_process::PostProcessedMatch::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::post_process::PostProcessedMatch::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::post_process::PostProcessedMatch::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::post_process::PostProcessedMatch where T: core::clone::Clone
pub type vyre_libs::scan::post_process::PostProcessedMatch::Owned = T
pub fn vyre_libs::scan::post_process::PostProcessedMatch::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::post_process::PostProcessedMatch::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::post_process::PostProcessedMatch where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::post_process::PostProcessedMatch where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::post_process::PostProcessedMatch where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::post_process::PostProcessedMatch where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::post_process::PostProcessedMatch
pub type vyre_libs::scan::post_process::PostProcessedMatch::Init = T
pub const vyre_libs::scan::post_process::PostProcessedMatch::ALIGN: usize
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> tracing::instrument::Instrument for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> typenum::type_operators::Same for vyre_libs::scan::post_process::PostProcessedMatch
pub type vyre_libs::scan::post_process::PostProcessedMatch::Output = T
pub fn vyre_libs::scan::post_process::reference_post_process(matches: &[vyre_foundation::runtime::match_result::Match], haystack: &[u8]) -> alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>
pub fn vyre_libs::scan::post_process::shannon_entropy_bits_per_byte(bytes: &[u8]) -> f32
pub fn vyre_libs::scan::post_process::try_reference_post_process(matches: &[vyre_foundation::runtime::match_result::Match], haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub fn vyre_libs::scan::post_process::try_reference_post_process_into(matches: &[vyre_foundation::runtime::match_result::Match], haystack: &[u8], triples: &mut alloc::vec::Vec<vyre_primitives::matching::region::RegionTriple>, out: &mut alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>) -> core::result::Result<(), vyre_libs::scan::post_process::PostProcessError>
pub mod vyre_libs::scan::substring
pub const vyre_libs::scan::substring::SCAN_SUBSTRING_OP_ID: &str
pub fn vyre_libs::scan::substring::substring_search(haystack: &str, needle: &str, matches: &str, haystack_len: u32, needle_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub enum vyre_libs::scan::ApiKind
pub vyre_libs::scan::ApiKind::Const
pub vyre_libs::scan::ApiKind::Enum
pub vyre_libs::scan::ApiKind::Function
pub vyre_libs::scan::ApiKind::Struct
pub vyre_libs::scan::ApiKind::Trait
pub vyre_libs::scan::ApiKind::TypeAlias
impl core::clone::Clone for vyre_libs::scan::ApiKind
pub fn vyre_libs::scan::ApiKind::clone(&self) -> vyre_libs::scan::ApiKind
impl core::cmp::Eq for vyre_libs::scan::ApiKind
impl core::cmp::PartialEq for vyre_libs::scan::ApiKind
pub fn vyre_libs::scan::ApiKind::eq(&self, other: &vyre_libs::scan::ApiKind) -> bool
impl core::fmt::Debug for vyre_libs::scan::ApiKind
pub fn vyre_libs::scan::ApiKind::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::scan::ApiKind
impl core::marker::StructuralPartialEq for vyre_libs::scan::ApiKind
impl core::marker::Freeze for vyre_libs::scan::ApiKind
impl core::marker::Send for vyre_libs::scan::ApiKind
impl core::marker::Sync for vyre_libs::scan::ApiKind
impl core::marker::Unpin for vyre_libs::scan::ApiKind
impl core::marker::UnsafeUnpin for vyre_libs::scan::ApiKind
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::ApiKind
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::ApiKind
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::scan::ApiKind where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::scan::ApiKind where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::scan::ApiKind where U: core::convert::From<T>
pub fn vyre_libs::scan::ApiKind::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::ApiKind where U: core::convert::Into<T>
pub type vyre_libs::scan::ApiKind::Error = core::convert::Infallible
pub fn vyre_libs::scan::ApiKind::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::ApiKind where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::ApiKind::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::ApiKind::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::ApiKind where T: core::clone::Clone
pub type vyre_libs::scan::ApiKind::Owned = T
pub fn vyre_libs::scan::ApiKind::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::ApiKind::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::ApiKind where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::ApiKind where T: ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::ApiKind where T: ?core::marker::Sized
pub fn vyre_libs::scan::ApiKind::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::ApiKind where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::ApiKind::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::ApiKind
pub fn vyre_libs::scan::ApiKind::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::ApiKind
pub type vyre_libs::scan::ApiKind::Init = T
pub const vyre_libs::scan::ApiKind::ALIGN: usize
pub unsafe fn vyre_libs::scan::ApiKind::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::ApiKind::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::ApiKind::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::ApiKind::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::ApiKind
impl<T> tracing::instrument::Instrument for vyre_libs::scan::ApiKind
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::ApiKind
impl<T> typenum::type_operators::Same for vyre_libs::scan::ApiKind
pub type vyre_libs::scan::ApiKind::Output = T
#[non_exhaustive] pub enum vyre_libs::scan::LiteralSetWireError
pub vyre_libs::scan::LiteralSetWireError::InvalidDfa(vyre_primitives::matching::dfa_compile::DfaWireError)
pub vyre_libs::scan::LiteralSetWireError::InvalidProgram(alloc::string::String)
pub vyre_libs::scan::LiteralSetWireError::WireFraming(vyre_foundation::serial::envelope::EnvelopeError)
impl core::error::Error for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetWireError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetWireError where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::string::ToString for vyre_libs::scan::literal_set::LiteralSetWireError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetWireError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetWireError where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetWireError where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetWireError
pub fn vyre_libs::scan::literal_set::LiteralSetWireError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetWireError
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetWireError::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetWireError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetWireError
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetWireError
pub type vyre_libs::scan::literal_set::LiteralSetWireError::Output = T
pub enum vyre_libs::scan::PostProcessError
pub vyre_libs::scan::PostProcessError::InvalidRange
pub vyre_libs::scan::PostProcessError::InvalidRange::end: u32
pub vyre_libs::scan::PostProcessError::InvalidRange::haystack_len: usize
pub vyre_libs::scan::PostProcessError::InvalidRange::pattern_id: u32
pub vyre_libs::scan::PostProcessError::InvalidRange::start: u32
impl core::clone::Clone for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::clone(&self) -> vyre_libs::scan::post_process::PostProcessError
impl core::cmp::Eq for vyre_libs::scan::post_process::PostProcessError
impl core::cmp::PartialEq for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::eq(&self, other: &vyre_libs::scan::post_process::PostProcessError) -> bool
impl core::error::Error for vyre_libs::scan::post_process::PostProcessError
impl core::fmt::Debug for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::scan::post_process::PostProcessError
impl core::marker::StructuralPartialEq for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Freeze for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Send for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Sync for vyre_libs::scan::post_process::PostProcessError
impl core::marker::Unpin for vyre_libs::scan::post_process::PostProcessError
impl core::marker::UnsafeUnpin for vyre_libs::scan::post_process::PostProcessError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::post_process::PostProcessError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::post_process::PostProcessError
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::scan::post_process::PostProcessError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::scan::post_process::PostProcessError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::From<T>
pub fn vyre_libs::scan::post_process::PostProcessError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::Into<T>
pub type vyre_libs::scan::post_process::PostProcessError::Error = core::convert::Infallible
pub fn vyre_libs::scan::post_process::PostProcessError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::post_process::PostProcessError where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::post_process::PostProcessError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::post_process::PostProcessError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::post_process::PostProcessError where T: core::clone::Clone
pub type vyre_libs::scan::post_process::PostProcessError::Owned = T
pub fn vyre_libs::scan::post_process::PostProcessError::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::post_process::PostProcessError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_libs::scan::post_process::PostProcessError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::scan::post_process::PostProcessError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::post_process::PostProcessError where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::post_process::PostProcessError where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::post_process::PostProcessError where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::post_process::PostProcessError
pub fn vyre_libs::scan::post_process::PostProcessError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::post_process::PostProcessError
pub type vyre_libs::scan::post_process::PostProcessError::Init = T
pub const vyre_libs::scan::post_process::PostProcessError::ALIGN: usize
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::post_process::PostProcessError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::post_process::PostProcessError
impl<T> tracing::instrument::Instrument for vyre_libs::scan::post_process::PostProcessError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::post_process::PostProcessError
impl<T> typenum::type_operators::Same for vyre_libs::scan::post_process::PostProcessError
pub type vyre_libs::scan::post_process::PostProcessError::Output = T
pub struct vyre_libs::scan::DirectGpuScanner
impl vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::compile(patterns: &[&[u8]]) -> Self
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::literal_set_cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::program(&self) -> &vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl !core::marker::Freeze for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Send for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Sync for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::Unpin for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl core::marker::UnsafeUnpin for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T, U> core::convert::Into<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::From<T>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::Into<T>
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Error = core::convert::Infallible
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::direct_gpu::DirectGpuScanner where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner where T: ?core::marker::Sized
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Init = T
pub const vyre_libs::scan::direct_gpu::DirectGpuScanner::ALIGN: usize
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::direct_gpu::DirectGpuScanner::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> tracing::instrument::Instrument for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::direct_gpu::DirectGpuScanner
impl<T> typenum::type_operators::Same for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub type vyre_libs::scan::direct_gpu::DirectGpuScanner::Output = T
pub struct vyre_libs::scan::GpuLiteralSet
pub vyre_libs::scan::GpuLiteralSet::dfa: vyre_primitives::matching::dfa_compile::CompiledDfa
pub vyre_libs::scan::GpuLiteralSet::pattern_bytes: alloc::vec::Vec<u32>
pub vyre_libs::scan::GpuLiteralSet::pattern_lengths: alloc::vec::Vec<u32>
pub vyre_libs::scan::GpuLiteralSet::pattern_offsets: alloc::vec::Vec<u32>
pub vyre_libs::scan::GpuLiteralSet::program: vyre_foundation::ir_inner::model::program::core::Program
impl vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::compile(patterns: &[&[u8]]) -> Self
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::count<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8]) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::count_with_literal_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, vyre_libs::scan::literal_set::LiteralSetWireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_count_dispatch(&self, haystack: &[u8]) -> core::result::Result<vyre_libs::scan::literal_set::LiteralSetPreparedCount, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_count_scratch(&self, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_literal_scratch(&self, max_matches: u32, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::prepare_scan_dispatch(&self, haystack: &[u8], max_matches: u32) -> core::result::Result<vyre_libs::scan::literal_set::LiteralSetPreparedScan, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into_with_literal_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, scratch: &mut vyre_libs::scan::literal_set::LiteralSetScanScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_into_with_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], max_matches: u32, matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, scratch: &mut vyre_libs::scan::dispatch_io::ScanDispatchScratch) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_presence<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<u32>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan_presence_with_scratch<B: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized>(&self, backend: &B, haystack: &[u8], scratch: &mut vyre_libs::scan::dispatch_io::ScanDispatchScratch) -> core::result::Result<alloc::vec::Vec<u32>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, vyre_libs::scan::literal_set::LiteralSetWireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_compile(patterns: &[&[u8]]) -> core::result::Result<Self, vyre_libs::scan::literal_set::LiteralSetCompileError>
impl vyre_libs::scan::engine::MatchEngineCache for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::WireError = vyre_libs::scan::literal_set::LiteralSetWireError
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_VERSION: u32
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl !core::marker::Freeze for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Send for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Sync for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::Unpin for vyre_libs::scan::literal_set::GpuLiteralSet
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::GpuLiteralSet
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::GpuLiteralSet
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::GpuLiteralSet where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::literal_set::GpuLiteralSet where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::GpuLiteralSet where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::GpuLiteralSet where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Init = T
pub const vyre_libs::scan::literal_set::GpuLiteralSet::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::GpuLiteralSet::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::GpuLiteralSet
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::Output = T
pub struct vyre_libs::scan::LiteralSetPreparedCount
pub vyre_libs::scan::LiteralSetPreparedCount::dispatch_config: vyre_driver::backend::dispatch_config::DispatchConfig
pub vyre_libs::scan::LiteralSetPreparedCount::encoded_input_bytes: u64
pub vyre_libs::scan::LiteralSetPreparedCount::haystack_len: u32
pub vyre_libs::scan::LiteralSetPreparedCount::inputs: alloc::vec::Vec<alloc::vec::Vec<u8>>
pub vyre_libs::scan::LiteralSetPreparedCount::program: vyre_foundation::ir_inner::model::program::core::Program
impl vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub const fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::count_readback_bytes(&self) -> usize
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::decode_outputs(&self, outputs: &[alloc::vec::Vec<u8>]) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
impl core::clone::Clone for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::clone(&self) -> vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl !core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: core::clone::Clone
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Owned = T
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::literal_set::LiteralSetPreparedCount where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetPreparedCount::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedCount::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetPreparedCount
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetPreparedCount
pub type vyre_libs::scan::literal_set::LiteralSetPreparedCount::Output = T
pub struct vyre_libs::scan::LiteralSetPreparedScan
pub vyre_libs::scan::LiteralSetPreparedScan::dispatch_config: vyre_driver::backend::dispatch_config::DispatchConfig
pub vyre_libs::scan::LiteralSetPreparedScan::encoded_input_bytes: u64
pub vyre_libs::scan::LiteralSetPreparedScan::haystack_len: u32
pub vyre_libs::scan::LiteralSetPreparedScan::inputs: alloc::vec::Vec<alloc::vec::Vec<u8>>
pub vyre_libs::scan::LiteralSetPreparedScan::matches_output_bytes: usize
pub vyre_libs::scan::LiteralSetPreparedScan::max_matches: u32
pub vyre_libs::scan::LiteralSetPreparedScan::program: vyre_foundation::ir_inner::model::program::core::Program
impl vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::decode_outputs_into(&self, outputs: &[alloc::vec::Vec<u8>], matches: &mut alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub const fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::match_count_readback_bytes(&self) -> usize
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::match_triples_readback_bytes(&self, match_count: u32) -> core::result::Result<usize, vyre_driver::backend::error::BackendError>
impl core::clone::Clone for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::clone(&self) -> vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl !core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: core::clone::Clone
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Owned = T
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::literal_set::LiteralSetPreparedScan where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetPreparedScan::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetPreparedScan::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetPreparedScan
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetPreparedScan
pub type vyre_libs::scan::literal_set::LiteralSetPreparedScan::Output = T
pub struct vyre_libs::scan::LiteralSetScanScratch
pub vyre_libs::scan::LiteralSetScanScratch::dispatch: vyre_libs::scan::dispatch_io::ScanDispatchScratch
impl core::default::Default for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::default() -> vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::fmt::Debug for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl !core::marker::Freeze for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::marker::Send for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::marker::Sync for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::marker::Unpin for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl core::marker::UnsafeUnpin for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl !core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl<T, U> core::convert::Into<U> for vyre_libs::scan::literal_set::LiteralSetScanScratch where U: core::convert::From<T>
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::literal_set::LiteralSetScanScratch where U: core::convert::Into<T>
pub type vyre_libs::scan::literal_set::LiteralSetScanScratch::Error = core::convert::Infallible
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::literal_set::LiteralSetScanScratch where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::literal_set::LiteralSetScanScratch::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_libs::scan::literal_set::LiteralSetScanScratch where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::literal_set::LiteralSetScanScratch where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::literal_set::LiteralSetScanScratch where T: ?core::marker::Sized
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub fn vyre_libs::scan::literal_set::LiteralSetScanScratch::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub type vyre_libs::scan::literal_set::LiteralSetScanScratch::Init = T
pub const vyre_libs::scan::literal_set::LiteralSetScanScratch::ALIGN: usize
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetScanScratch::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetScanScratch::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetScanScratch::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::literal_set::LiteralSetScanScratch::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl<T> tracing::instrument::Instrument for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::literal_set::LiteralSetScanScratch
impl<T> typenum::type_operators::Same for vyre_libs::scan::literal_set::LiteralSetScanScratch
pub type vyre_libs::scan::literal_set::LiteralSetScanScratch::Output = T
pub struct vyre_libs::scan::Pipeline<E>
pub vyre_libs::scan::Pipeline::engine: E
pub vyre_libs::scan::Pipeline::post_process: vyre_libs::scan::pipeline::PostProcessFn
impl<E: vyre_libs::scan::engine::MatchScan> vyre_libs::scan::pipeline::Pipeline<E>
pub const fn vyre_libs::scan::pipeline::Pipeline<E>::new(engine: E) -> Self
pub fn vyre_libs::scan::pipeline::Pipeline<E>::reference_scan_processed(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::scan_processed(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_reference_scan_processed(&self, haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub const fn vyre_libs::scan::pipeline::Pipeline<E>::with_post_process(engine: E, post_process: vyre_libs::scan::pipeline::PostProcessFn) -> Self
impl<E: core::clone::Clone> core::clone::Clone for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::clone(&self) -> Self
impl<E: core::fmt::Debug> core::fmt::Debug for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl<E> core::marker::Freeze for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Freeze
impl<E> core::marker::Send for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Send
impl<E> core::marker::Sync for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Sync
impl<E> core::marker::Unpin for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::Unpin
impl<E> core::marker::UnsafeUnpin for vyre_libs::scan::pipeline::Pipeline<E> where E: core::marker::UnsafeUnpin
impl<E> core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::pipeline::Pipeline<E> where E: core::panic::unwind_safe::RefUnwindSafe
impl<E> core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::pipeline::Pipeline<E> where E: core::panic::unwind_safe::UnwindSafe
impl<T, U> core::convert::Into<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::From<T>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::Into<T>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Error = core::convert::Infallible
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::pipeline::Pipeline<E> where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::pipeline::Pipeline<E>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::pipeline::Pipeline<E> where T: core::clone::Clone
pub type vyre_libs::scan::pipeline::Pipeline<E>::Owned = T
pub fn vyre_libs::scan::pipeline::Pipeline<E>::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::pipeline::Pipeline<E>::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::pipeline::Pipeline<E> where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::pipeline::Pipeline<E> where T: ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::pipeline::Pipeline<E> where T: ?core::marker::Sized
pub fn vyre_libs::scan::pipeline::Pipeline<E>::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::pipeline::Pipeline<E> where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::pipeline::Pipeline<E>
pub fn vyre_libs::scan::pipeline::Pipeline<E>::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::pipeline::Pipeline<E>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Init = T
pub const vyre_libs::scan::pipeline::Pipeline<E>::ALIGN: usize
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::pipeline::Pipeline<E>::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> tracing::instrument::Instrument for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::pipeline::Pipeline<E>
impl<T> typenum::type_operators::Same for vyre_libs::scan::pipeline::Pipeline<E>
pub type vyre_libs::scan::pipeline::Pipeline<E>::Output = T
pub struct vyre_libs::scan::PostProcessedMatch
pub vyre_libs::scan::PostProcessedMatch::confidence: f32
pub vyre_libs::scan::PostProcessedMatch::end: u32
pub vyre_libs::scan::PostProcessedMatch::entropy_bits_per_byte: f32
pub vyre_libs::scan::PostProcessedMatch::pattern_id: u32
pub vyre_libs::scan::PostProcessedMatch::start: u32
impl core::clone::Clone for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::clone(&self) -> vyre_libs::scan::post_process::PostProcessedMatch
impl core::cmp::PartialEq for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::eq(&self, other: &vyre_libs::scan::post_process::PostProcessedMatch) -> bool
impl core::fmt::Debug for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::StructuralPartialEq for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Freeze for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Send for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Sync for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::Unpin for vyre_libs::scan::post_process::PostProcessedMatch
impl core::marker::UnsafeUnpin for vyre_libs::scan::post_process::PostProcessedMatch
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::post_process::PostProcessedMatch
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::post_process::PostProcessedMatch
impl<T, U> core::convert::Into<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::From<T>
pub fn vyre_libs::scan::post_process::PostProcessedMatch::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::Into<T>
pub type vyre_libs::scan::post_process::PostProcessedMatch::Error = core::convert::Infallible
pub fn vyre_libs::scan::post_process::PostProcessedMatch::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::post_process::PostProcessedMatch where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::post_process::PostProcessedMatch::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::post_process::PostProcessedMatch::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::post_process::PostProcessedMatch where T: core::clone::Clone
pub type vyre_libs::scan::post_process::PostProcessedMatch::Owned = T
pub fn vyre_libs::scan::post_process::PostProcessedMatch::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::post_process::PostProcessedMatch::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::post_process::PostProcessedMatch where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::post_process::PostProcessedMatch where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::post_process::PostProcessedMatch where T: ?core::marker::Sized
pub fn vyre_libs::scan::post_process::PostProcessedMatch::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::post_process::PostProcessedMatch where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::post_process::PostProcessedMatch
pub fn vyre_libs::scan::post_process::PostProcessedMatch::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::post_process::PostProcessedMatch
pub type vyre_libs::scan::post_process::PostProcessedMatch::Init = T
pub const vyre_libs::scan::post_process::PostProcessedMatch::ALIGN: usize
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::post_process::PostProcessedMatch::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> tracing::instrument::Instrument for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::post_process::PostProcessedMatch
impl<T> typenum::type_operators::Same for vyre_libs::scan::post_process::PostProcessedMatch
pub type vyre_libs::scan::post_process::PostProcessedMatch::Output = T
pub struct vyre_libs::scan::ScanResult
pub vyre_libs::scan::ScanResult::cache_hit: bool
pub vyre_libs::scan::ScanResult::elapsed: core::time::Duration
pub vyre_libs::scan::ScanResult::matches: alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub vyre_libs::scan::ScanResult::truncated: bool
impl vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::from_matches(matches: alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>) -> Self
pub fn vyre_libs::scan::engine::ScanResult::is_empty(&self) -> bool
pub fn vyre_libs::scan::engine::ScanResult::len(&self) -> usize
impl core::clone::Clone for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::clone(&self) -> vyre_libs::scan::engine::ScanResult
impl core::default::Default for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::default() -> vyre_libs::scan::engine::ScanResult
impl core::fmt::Debug for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::scan::engine::ScanResult
impl core::marker::Send for vyre_libs::scan::engine::ScanResult
impl core::marker::Sync for vyre_libs::scan::engine::ScanResult
impl core::marker::Unpin for vyre_libs::scan::engine::ScanResult
impl core::marker::UnsafeUnpin for vyre_libs::scan::engine::ScanResult
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::scan::engine::ScanResult
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::scan::engine::ScanResult
impl<T, U> core::convert::Into<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::From<T>
pub fn vyre_libs::scan::engine::ScanResult::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::Into<T>
pub type vyre_libs::scan::engine::ScanResult::Error = core::convert::Infallible
pub fn vyre_libs::scan::engine::ScanResult::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::scan::engine::ScanResult where U: core::convert::TryFrom<T>
pub type vyre_libs::scan::engine::ScanResult::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::scan::engine::ScanResult::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::scan::engine::ScanResult where T: core::clone::Clone
pub type vyre_libs::scan::engine::ScanResult::Owned = T
pub fn vyre_libs::scan::engine::ScanResult::clone_into(&self, target: &mut T)
pub fn vyre_libs::scan::engine::ScanResult::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::scan::engine::ScanResult where T: 'static + ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::scan::engine::ScanResult where T: ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::scan::engine::ScanResult where T: ?core::marker::Sized
pub fn vyre_libs::scan::engine::ScanResult::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::scan::engine::ScanResult where T: core::clone::Clone
pub unsafe fn vyre_libs::scan::engine::ScanResult::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::scan::engine::ScanResult
pub fn vyre_libs::scan::engine::ScanResult::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::scan::engine::ScanResult
pub type vyre_libs::scan::engine::ScanResult::Init = T
pub const vyre_libs::scan::engine::ScanResult::ALIGN: usize
pub unsafe fn vyre_libs::scan::engine::ScanResult::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::scan::engine::ScanResult::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::scan::engine::ScanResult::drop(ptr: usize)
pub unsafe fn vyre_libs::scan::engine::ScanResult::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::scan::engine::ScanResult
impl<T> tracing::instrument::Instrument for vyre_libs::scan::engine::ScanResult
impl<T> tracing::instrument::WithSubscriber for vyre_libs::scan::engine::ScanResult
impl<T> typenum::type_operators::Same for vyre_libs::scan::engine::ScanResult
pub type vyre_libs::scan::engine::ScanResult::Output = T
pub const vyre_libs::scan::API_INDEX: &[(&str, vyre_libs::scan::ApiKind, core::option::Option<&str>)]
pub const vyre_libs::scan::DEFAULT_MAX_SCAN_BYTES: u32
pub const vyre_libs::scan::HIT_BUFFER_LIVE_LENGTH: &str
pub const vyre_libs::scan::HIT_BUFFER_OVERFLOW_COUNT: &str
pub const vyre_libs::scan::LITERAL_SET_COUNT_RESET_RESOURCE_INDICES: [usize; 1]
pub const vyre_libs::scan::LITERAL_SET_COUNT_RESOURCE_INDEX: usize
pub const vyre_libs::scan::LITERAL_SET_COUNT_SCAN_RESOURCE_INDICES: [usize; 8]
pub const vyre_libs::scan::LITERAL_SET_MATCHES_RESOURCE_INDEX: usize
pub const vyre_libs::scan::LITERAL_SET_MATCH_COUNT_RESOURCE_INDEX: usize
pub const vyre_libs::scan::LITERAL_SET_RESET_RESOURCE_INDICES: [usize; 1]
pub const vyre_libs::scan::LITERAL_SET_SCAN_RESOURCE_INDICES: [usize; 11]
pub const vyre_libs::scan::SCAN_SUBSTRING_OP_ID: &str
pub trait vyre_libs::scan::MatchEngineCache: core::marker::Sized
pub type vyre_libs::scan::MatchEngineCache::WireError: core::fmt::Display + core::fmt::Debug
pub const vyre_libs::scan::MatchEngineCache::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::scan::MatchEngineCache::WIRE_VERSION: u32
pub fn vyre_libs::scan::MatchEngineCache::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::scan::MatchEngineCache::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
impl vyre_libs::scan::engine::MatchEngineCache for vyre_libs::scan::literal_set::GpuLiteralSet
pub type vyre_libs::scan::literal_set::GpuLiteralSet::WireError = vyre_libs::scan::literal_set::LiteralSetWireError
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_MAGIC: [u8; 4]
pub const vyre_libs::scan::literal_set::GpuLiteralSet::WIRE_VERSION: u32
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::from_bytes(bytes: &[u8]) -> core::result::Result<Self, Self::WireError>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::to_bytes(&self) -> core::result::Result<alloc::vec::Vec<u8>, Self::WireError>
pub trait vyre_libs::scan::MatchScan
pub fn vyre_libs::scan::MatchScan::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::MatchScan::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::MatchScan::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::direct_gpu::DirectGpuScanner
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::direct_gpu::DirectGpuScanner::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
impl vyre_libs::scan::engine::MatchScan for vyre_libs::scan::literal_set::GpuLiteralSet
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::cache_key(&self) -> alloc::string::String
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::reference_scan(&self, haystack: &[u8]) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub fn vyre_libs::scan::literal_set::GpuLiteralSet::scan(&self, backend: &dyn vyre_driver::backend::vyre_backend::VyreBackend, haystack: &[u8], max_matches: u32) -> core::result::Result<alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::aho_corasick(haystack: &str, transitions: &str, accept: &str, matches: &str, haystack_len: u32, state_count: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::byte_scan_dispatch_config(haystack_len: u32, workgroup_x: u32) -> vyre_driver::backend::dispatch_config::DispatchConfig
pub fn vyre_libs::scan::cached_load_or_compile<E, F>(cache_dir: &std::path::Path, cache_key: &str, compile: F) -> E where E: vyre_libs::scan::engine::MatchEngineCache, F: core::ops::function::FnOnce() -> E
pub fn vyre_libs::scan::candidate_start_dispatch_config(haystack_len: u32) -> vyre_driver::backend::dispatch_config::DispatchConfig
pub fn vyre_libs::scan::compact_hits(out_hits: &str, out_cursor: &str, max_capacity: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::compact_hits_with_layout(out_hits: &str, out_cursor: &str, hit_capacity: u32, max_capacity: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::dedup_regions_reference(input: alloc::vec::Vec<vyre_primitives::matching::region::RegionTriple>) -> alloc::vec::Vec<vyre_primitives::matching::region::RegionTriple>
pub fn vyre_libs::scan::emit_hit(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::emit_hit_then_compact(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::execution_plan::fusion::FusionError>
pub fn vyre_libs::scan::emit_hit_then_compact_with_layout(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str, lane_count: u32, max_hits: u32) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_foundation::execution_plan::fusion::FusionError>
pub fn vyre_libs::scan::emit_hit_with_layout(rule_id: &str, file_id: &str, span_start: &str, span_len: &str, out_hits: &str, out_cursor: &str, lane_count: u32, max_hits: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::engine_cache_path(cache_dir: &std::path::Path, cache_key: &str) -> core::option::Option<std::path::PathBuf>
pub fn vyre_libs::scan::haystack_len_u32(haystack: &[u8], context: &str) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::pack_haystack_u32(haystack: &[u8]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::scan::pack_u32_slice(words: &[u32]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::scan::reference_post_process(matches: &[vyre_foundation::runtime::match_result::Match], haystack: &[u8]) -> alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>
pub fn vyre_libs::scan::scan_guard(haystack: &[u8], context: &str, max_bytes: u32) -> core::result::Result<u32, vyre_driver::backend::error::BackendError>
pub fn vyre_libs::scan::shannon_entropy_bits_per_byte(bytes: &[u8]) -> f32
pub fn vyre_libs::scan::substring_search(haystack: &str, needle: &str, matches: &str, haystack_len: u32, needle_len: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::scan::try_reference_post_process(matches: &[vyre_foundation::runtime::match_result::Match], haystack: &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub fn vyre_libs::scan::try_reference_post_process_into(matches: &[vyre_foundation::runtime::match_result::Match], haystack: &[u8], triples: &mut alloc::vec::Vec<vyre_primitives::matching::region::RegionTriple>, out: &mut alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>) -> core::result::Result<(), vyre_libs::scan::post_process::PostProcessError>
pub fn vyre_libs::scan::u32_words_as_le_bytes(words: &[u32]) -> alloc::borrow::Cow<'_, [u8]>
pub fn vyre_libs::scan::unpack_match_triples(triples_bytes: &[u8], count: u32) -> alloc::vec::Vec<vyre_foundation::runtime::match_result::Match>
pub type vyre_libs::scan::PostProcessFn = fn(&[vyre_foundation::runtime::match_result::Match], &[u8]) -> core::result::Result<alloc::vec::Vec<vyre_libs::scan::post_process::PostProcessedMatch>, vyre_libs::scan::post_process::PostProcessError>
pub mod vyre_libs::signatures
pub const vyre_libs::signatures::BOOL_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::BYTES_TO_BYTES_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::BYTES_TO_BYTES_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::BYTES_TO_U32_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::F32_F32_F32_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::F32_F32_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::F32_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::F32_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::I32_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::U32_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::U32_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::signatures::U32_U32_INPUTS: &[vyre_spec::data_type::DataType]
pub mod vyre_libs::tensor_ref
#[non_exhaustive] pub enum vyre_libs::tensor_ref::TensorRefError
pub vyre_libs::tensor_ref::TensorRefError::DtypeMismatch
pub vyre_libs::tensor_ref::TensorRefError::DtypeMismatch::expected: vyre_spec::data_type::DataType
pub vyre_libs::tensor_ref::TensorRefError::DtypeMismatch::found: vyre_spec::data_type::DataType
pub vyre_libs::tensor_ref::TensorRefError::DtypeMismatch::name: alloc::string::String
pub vyre_libs::tensor_ref::TensorRefError::DtypeMismatch::op: &'static str
pub vyre_libs::tensor_ref::TensorRefError::ElementCountOverflow
pub vyre_libs::tensor_ref::TensorRefError::ElementCountOverflow::name: alloc::string::String
pub vyre_libs::tensor_ref::TensorRefError::ElementCountOverflow::shape: alloc::vec::Vec<u32>
pub vyre_libs::tensor_ref::TensorRefError::NameCollision
pub vyre_libs::tensor_ref::TensorRefError::NameCollision::name: alloc::string::String
pub vyre_libs::tensor_ref::TensorRefError::NameCollision::op: &'static str
pub vyre_libs::tensor_ref::TensorRefError::ShapeMismatch
pub vyre_libs::tensor_ref::TensorRefError::ShapeMismatch::expected: alloc::vec::Vec<u32>
pub vyre_libs::tensor_ref::TensorRefError::ShapeMismatch::found: alloc::vec::Vec<u32>
pub vyre_libs::tensor_ref::TensorRefError::ShapeMismatch::name: alloc::string::String
pub vyre_libs::tensor_ref::TensorRefError::ShapeMismatch::op: &'static str
impl core::clone::Clone for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::clone(&self) -> vyre_libs::tensor_ref::TensorRefError
impl core::error::Error for vyre_libs::tensor_ref::TensorRefError
impl core::fmt::Debug for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::fmt(&self, __formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Send for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Sync for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Unpin for vyre_libs::tensor_ref::TensorRefError
impl core::marker::UnsafeUnpin for vyre_libs::tensor_ref::TensorRefError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::tensor_ref::TensorRefError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::tensor_ref::TensorRefError
impl<T, U> core::convert::Into<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::From<T>
pub fn vyre_libs::tensor_ref::TensorRefError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::Into<T>
pub type vyre_libs::tensor_ref::TensorRefError::Error = core::convert::Infallible
pub fn vyre_libs::tensor_ref::TensorRefError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::TryFrom<T>
pub type vyre_libs::tensor_ref::TensorRefError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::tensor_ref::TensorRefError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::tensor_ref::TensorRefError where T: core::clone::Clone
pub type vyre_libs::tensor_ref::TensorRefError::Owned = T
pub fn vyre_libs::tensor_ref::TensorRefError::clone_into(&self, target: &mut T)
pub fn vyre_libs::tensor_ref::TensorRefError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_libs::tensor_ref::TensorRefError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::tensor_ref::TensorRefError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::tensor_ref::TensorRefError where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::tensor_ref::TensorRefError where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::tensor_ref::TensorRefError where T: core::clone::Clone
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::tensor_ref::TensorRefError
pub type vyre_libs::tensor_ref::TensorRefError::Init = T
pub const vyre_libs::tensor_ref::TensorRefError::ALIGN: usize
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::drop(ptr: usize)
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::tensor_ref::TensorRefError
impl<T> tracing::instrument::Instrument for vyre_libs::tensor_ref::TensorRefError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::tensor_ref::TensorRefError
impl<T> typenum::type_operators::Same for vyre_libs::tensor_ref::TensorRefError
pub type vyre_libs::tensor_ref::TensorRefError::Output = T
#[non_exhaustive] pub struct vyre_libs::tensor_ref::TensorRef
pub vyre_libs::tensor_ref::TensorRef::dtype: vyre_spec::data_type::DataType
pub vyre_libs::tensor_ref::TensorRef::name: vyre_foundation::ir_inner::model::expr::Ident
pub vyre_libs::tensor_ref::TensorRef::shape: alloc::sync::Arc<[u32]>
impl vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::element_count(&self) -> core::option::Option<u32>
pub fn vyre_libs::tensor_ref::TensorRef::f16_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f16_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f32_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f32_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::name_str(&self) -> &str
pub fn vyre_libs::tensor_ref::TensorRef::new(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, dtype: vyre_spec::data_type::DataType, shape: alloc::vec::Vec<u32>) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::u32_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::u32_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
impl core::clone::Clone for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::clone(&self) -> vyre_libs::tensor_ref::TensorRef
impl core::cmp::Eq for vyre_libs::tensor_ref::TensorRef
impl core::cmp::PartialEq for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::eq(&self, other: &vyre_libs::tensor_ref::TensorRef) -> bool
impl core::fmt::Debug for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_libs::tensor_ref::TensorRef
impl core::marker::Freeze for vyre_libs::tensor_ref::TensorRef
impl core::marker::Send for vyre_libs::tensor_ref::TensorRef
impl core::marker::Sync for vyre_libs::tensor_ref::TensorRef
impl core::marker::Unpin for vyre_libs::tensor_ref::TensorRef
impl core::marker::UnsafeUnpin for vyre_libs::tensor_ref::TensorRef
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::tensor_ref::TensorRef
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::tensor_ref::TensorRef
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::tensor_ref::TensorRef where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::tensor_ref::TensorRef where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::From<T>
pub fn vyre_libs::tensor_ref::TensorRef::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::Into<T>
pub type vyre_libs::tensor_ref::TensorRef::Error = core::convert::Infallible
pub fn vyre_libs::tensor_ref::TensorRef::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::TryFrom<T>
pub type vyre_libs::tensor_ref::TensorRef::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::tensor_ref::TensorRef::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::tensor_ref::TensorRef where T: core::clone::Clone
pub type vyre_libs::tensor_ref::TensorRef::Owned = T
pub fn vyre_libs::tensor_ref::TensorRef::clone_into(&self, target: &mut T)
pub fn vyre_libs::tensor_ref::TensorRef::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::tensor_ref::TensorRef where T: 'static + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::tensor_ref::TensorRef where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::tensor_ref::TensorRef where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::tensor_ref::TensorRef where T: core::clone::Clone
pub unsafe fn vyre_libs::tensor_ref::TensorRef::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::tensor_ref::TensorRef
pub type vyre_libs::tensor_ref::TensorRef::Init = T
pub const vyre_libs::tensor_ref::TensorRef::ALIGN: usize
pub unsafe fn vyre_libs::tensor_ref::TensorRef::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::tensor_ref::TensorRef::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::tensor_ref::TensorRef::drop(ptr: usize)
pub unsafe fn vyre_libs::tensor_ref::TensorRef::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::tensor_ref::TensorRef
impl<T> tracing::instrument::Instrument for vyre_libs::tensor_ref::TensorRef
impl<T> tracing::instrument::WithSubscriber for vyre_libs::tensor_ref::TensorRef
impl<T> typenum::type_operators::Same for vyre_libs::tensor_ref::TensorRef
pub type vyre_libs::tensor_ref::TensorRef::Output = T
pub fn vyre_libs::tensor_ref::check_dtype(r: &vyre_libs::tensor_ref::TensorRef, expected: vyre_spec::data_type::DataType, op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::tensor_ref::check_shape(r: &vyre_libs::tensor_ref::TensorRef, expected: &[u32], op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::tensor_ref::check_unique_names(refs: &[&vyre_libs::tensor_ref::TensorRef], op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub mod vyre_libs::test_support
pub mod vyre_libs::test_support::byte_pack
pub fn vyre_libs::test_support::byte_pack::bytes_to_u32(slice: &[u8]) -> alloc::vec::Vec<u32>
pub fn vyre_libs::test_support::byte_pack::decode_f32(bytes: &[u8]) -> alloc::vec::Vec<f32>
pub fn vyre_libs::test_support::byte_pack::decode_f32_one(bytes: &[u8]) -> f32
pub fn vyre_libs::test_support::byte_pack::decode_u32_one(bytes: &[u8]) -> u32
pub fn vyre_libs::test_support::byte_pack::f32_bytes(values: &[f32]) -> alloc::vec::Vec<u8>
pub fn vyre_libs::test_support::byte_pack::u32_bytes(words: &[u32]) -> alloc::vec::Vec<u8>
pub mod vyre_libs::text
pub use vyre_libs::text::C_ALPHA
pub use vyre_libs::text::C_AMP
pub use vyre_libs::text::C_BACKSLASH
pub use vyre_libs::text::C_BANG
pub use vyre_libs::text::C_CARET
pub use vyre_libs::text::C_CLOSE_BRACE
pub use vyre_libs::text::C_CLOSE_BRACKET
pub use vyre_libs::text::C_CLOSE_PAREN
pub use vyre_libs::text::C_COMMA
pub use vyre_libs::text::C_DIGIT
pub use vyre_libs::text::C_DOT
pub use vyre_libs::text::C_DQUOTE
pub use vyre_libs::text::C_EOF
pub use vyre_libs::text::C_EQUALS
pub use vyre_libs::text::C_GT
pub use vyre_libs::text::C_HASH
pub use vyre_libs::text::C_LT
pub use vyre_libs::text::C_MINUS
pub use vyre_libs::text::C_NEWLINE
pub use vyre_libs::text::C_OPEN_BRACE
pub use vyre_libs::text::C_OPEN_BRACKET
pub use vyre_libs::text::C_OPEN_PAREN
pub use vyre_libs::text::C_OTHER
pub use vyre_libs::text::C_PERCENT
pub use vyre_libs::text::C_PIPE
pub use vyre_libs::text::C_PLUS
pub use vyre_libs::text::C_QUOTE
pub use vyre_libs::text::C_SEMICOLON
pub use vyre_libs::text::C_SLASH
pub use vyre_libs::text::C_STAR
pub use vyre_libs::text::C_TILDE
pub use vyre_libs::text::C_WS
pub use vyre_libs::text::build_char_class_table
pub use vyre_libs::text::char_class
pub mod vyre_libs::text::char_class
pub use vyre_libs::text::char_class::C_ALPHA
pub use vyre_libs::text::char_class::C_AMP
pub use vyre_libs::text::char_class::C_BACKSLASH
pub use vyre_libs::text::char_class::C_BANG
pub use vyre_libs::text::char_class::C_CARET
pub use vyre_libs::text::char_class::C_CLOSE_BRACE
pub use vyre_libs::text::char_class::C_CLOSE_BRACKET
pub use vyre_libs::text::char_class::C_CLOSE_PAREN
pub use vyre_libs::text::char_class::C_COMMA
pub use vyre_libs::text::char_class::C_DIGIT
pub use vyre_libs::text::char_class::C_DOT
pub use vyre_libs::text::char_class::C_DQUOTE
pub use vyre_libs::text::char_class::C_EOF
pub use vyre_libs::text::char_class::C_EQUALS
pub use vyre_libs::text::char_class::C_GT
pub use vyre_libs::text::char_class::C_HASH
pub use vyre_libs::text::char_class::C_LT
pub use vyre_libs::text::char_class::C_MINUS
pub use vyre_libs::text::char_class::C_NEWLINE
pub use vyre_libs::text::char_class::C_OPEN_BRACE
pub use vyre_libs::text::char_class::C_OPEN_BRACKET
pub use vyre_libs::text::char_class::C_OPEN_PAREN
pub use vyre_libs::text::char_class::C_OTHER
pub use vyre_libs::text::char_class::C_PERCENT
pub use vyre_libs::text::char_class::C_PIPE
pub use vyre_libs::text::char_class::C_PLUS
pub use vyre_libs::text::char_class::C_QUOTE
pub use vyre_libs::text::char_class::C_SEMICOLON
pub use vyre_libs::text::char_class::C_SLASH
pub use vyre_libs::text::char_class::C_STAR
pub use vyre_libs::text::char_class::C_TILDE
pub use vyre_libs::text::char_class::C_WS
pub use vyre_libs::text::char_class::build_char_class_table
pub use vyre_libs::text::char_class::char_class
pub use vyre_libs::text::char_class::pack_bytes_as_u32
pub use vyre_libs::text::char_class::pack_u32
pub use vyre_libs::text::char_class::reference_char_class
#[non_exhaustive] pub enum vyre_libs::TensorRefError
pub vyre_libs::TensorRefError::DtypeMismatch
pub vyre_libs::TensorRefError::DtypeMismatch::expected: vyre_spec::data_type::DataType
pub vyre_libs::TensorRefError::DtypeMismatch::found: vyre_spec::data_type::DataType
pub vyre_libs::TensorRefError::DtypeMismatch::name: alloc::string::String
pub vyre_libs::TensorRefError::DtypeMismatch::op: &'static str
pub vyre_libs::TensorRefError::ElementCountOverflow
pub vyre_libs::TensorRefError::ElementCountOverflow::name: alloc::string::String
pub vyre_libs::TensorRefError::ElementCountOverflow::shape: alloc::vec::Vec<u32>
pub vyre_libs::TensorRefError::NameCollision
pub vyre_libs::TensorRefError::NameCollision::name: alloc::string::String
pub vyre_libs::TensorRefError::NameCollision::op: &'static str
pub vyre_libs::TensorRefError::ShapeMismatch
pub vyre_libs::TensorRefError::ShapeMismatch::expected: alloc::vec::Vec<u32>
pub vyre_libs::TensorRefError::ShapeMismatch::found: alloc::vec::Vec<u32>
pub vyre_libs::TensorRefError::ShapeMismatch::name: alloc::string::String
pub vyre_libs::TensorRefError::ShapeMismatch::op: &'static str
impl core::clone::Clone for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::clone(&self) -> vyre_libs::tensor_ref::TensorRefError
impl core::error::Error for vyre_libs::tensor_ref::TensorRefError
impl core::fmt::Debug for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::fmt(&self, __formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Send for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Sync for vyre_libs::tensor_ref::TensorRefError
impl core::marker::Unpin for vyre_libs::tensor_ref::TensorRefError
impl core::marker::UnsafeUnpin for vyre_libs::tensor_ref::TensorRefError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::tensor_ref::TensorRefError
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::tensor_ref::TensorRefError
impl<T, U> core::convert::Into<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::From<T>
pub fn vyre_libs::tensor_ref::TensorRefError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::Into<T>
pub type vyre_libs::tensor_ref::TensorRefError::Error = core::convert::Infallible
pub fn vyre_libs::tensor_ref::TensorRefError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::tensor_ref::TensorRefError where U: core::convert::TryFrom<T>
pub type vyre_libs::tensor_ref::TensorRefError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::tensor_ref::TensorRefError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::tensor_ref::TensorRefError where T: core::clone::Clone
pub type vyre_libs::tensor_ref::TensorRefError::Owned = T
pub fn vyre_libs::tensor_ref::TensorRefError::clone_into(&self, target: &mut T)
pub fn vyre_libs::tensor_ref::TensorRefError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_libs::tensor_ref::TensorRefError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_libs::tensor_ref::TensorRefError where T: 'static + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::tensor_ref::TensorRefError where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::tensor_ref::TensorRefError where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRefError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::tensor_ref::TensorRefError where T: core::clone::Clone
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::tensor_ref::TensorRefError
pub fn vyre_libs::tensor_ref::TensorRefError::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::tensor_ref::TensorRefError
pub type vyre_libs::tensor_ref::TensorRefError::Init = T
pub const vyre_libs::tensor_ref::TensorRefError::ALIGN: usize
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::drop(ptr: usize)
pub unsafe fn vyre_libs::tensor_ref::TensorRefError::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::tensor_ref::TensorRefError
impl<T> tracing::instrument::Instrument for vyre_libs::tensor_ref::TensorRefError
impl<T> tracing::instrument::WithSubscriber for vyre_libs::tensor_ref::TensorRefError
impl<T> typenum::type_operators::Same for vyre_libs::tensor_ref::TensorRefError
pub type vyre_libs::tensor_ref::TensorRefError::Output = T
#[non_exhaustive] pub struct vyre_libs::BufferDescriptor
pub vyre_libs::BufferDescriptor::access: vyre_spec::buffer_access::BufferAccess
pub vyre_libs::BufferDescriptor::count: u32
pub vyre_libs::BufferDescriptor::dtype: vyre_spec::data_type::DataType
pub vyre_libs::BufferDescriptor::name: alloc::string::String
impl vyre_libs::descriptor::BufferDescriptor
pub fn vyre_libs::descriptor::BufferDescriptor::new(name: alloc::string::String, access: vyre_spec::buffer_access::BufferAccess, dtype: vyre_spec::data_type::DataType, count: u32) -> Self
impl core::clone::Clone for vyre_libs::descriptor::BufferDescriptor
pub fn vyre_libs::descriptor::BufferDescriptor::clone(&self) -> vyre_libs::descriptor::BufferDescriptor
impl core::fmt::Debug for vyre_libs::descriptor::BufferDescriptor
pub fn vyre_libs::descriptor::BufferDescriptor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::descriptor::BufferDescriptor
impl core::marker::Send for vyre_libs::descriptor::BufferDescriptor
impl core::marker::Sync for vyre_libs::descriptor::BufferDescriptor
impl core::marker::Unpin for vyre_libs::descriptor::BufferDescriptor
impl core::marker::UnsafeUnpin for vyre_libs::descriptor::BufferDescriptor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::descriptor::BufferDescriptor
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::descriptor::BufferDescriptor
impl<T, U> core::convert::Into<U> for vyre_libs::descriptor::BufferDescriptor where U: core::convert::From<T>
pub fn vyre_libs::descriptor::BufferDescriptor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::descriptor::BufferDescriptor where U: core::convert::Into<T>
pub type vyre_libs::descriptor::BufferDescriptor::Error = core::convert::Infallible
pub fn vyre_libs::descriptor::BufferDescriptor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::descriptor::BufferDescriptor where U: core::convert::TryFrom<T>
pub type vyre_libs::descriptor::BufferDescriptor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::descriptor::BufferDescriptor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::descriptor::BufferDescriptor where T: core::clone::Clone
pub type vyre_libs::descriptor::BufferDescriptor::Owned = T
pub fn vyre_libs::descriptor::BufferDescriptor::clone_into(&self, target: &mut T)
pub fn vyre_libs::descriptor::BufferDescriptor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::descriptor::BufferDescriptor where T: 'static + ?core::marker::Sized
pub fn vyre_libs::descriptor::BufferDescriptor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::descriptor::BufferDescriptor where T: ?core::marker::Sized
pub fn vyre_libs::descriptor::BufferDescriptor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::descriptor::BufferDescriptor where T: ?core::marker::Sized
pub fn vyre_libs::descriptor::BufferDescriptor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::descriptor::BufferDescriptor where T: core::clone::Clone
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::descriptor::BufferDescriptor
pub fn vyre_libs::descriptor::BufferDescriptor::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::descriptor::BufferDescriptor
pub type vyre_libs::descriptor::BufferDescriptor::Init = T
pub const vyre_libs::descriptor::BufferDescriptor::ALIGN: usize
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::drop(ptr: usize)
pub unsafe fn vyre_libs::descriptor::BufferDescriptor::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::descriptor::BufferDescriptor
impl<T> tracing::instrument::Instrument for vyre_libs::descriptor::BufferDescriptor
impl<T> tracing::instrument::WithSubscriber for vyre_libs::descriptor::BufferDescriptor
impl<T> typenum::type_operators::Same for vyre_libs::descriptor::BufferDescriptor
pub type vyre_libs::descriptor::BufferDescriptor::Output = T
#[non_exhaustive] pub struct vyre_libs::BuildOptions
pub vyre_libs::BuildOptions::region_generator: core::option::Option<&'static str>
pub vyre_libs::BuildOptions::tenant_id: core::option::Option<u32>
pub vyre_libs::BuildOptions::workgroup_size: core::option::Option<[u32; 3]>
impl vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::new() -> Self
pub fn vyre_libs::builder::BuildOptions::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::builder::BuildOptions::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::builder::BuildOptions::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::clone(&self) -> vyre_libs::builder::BuildOptions
impl core::default::Default for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::default() -> vyre_libs::builder::BuildOptions
impl core::fmt::Debug for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::builder::BuildOptions
impl core::marker::Send for vyre_libs::builder::BuildOptions
impl core::marker::Sync for vyre_libs::builder::BuildOptions
impl core::marker::Unpin for vyre_libs::builder::BuildOptions
impl core::marker::UnsafeUnpin for vyre_libs::builder::BuildOptions
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::builder::BuildOptions
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::builder::BuildOptions
impl<T, U> core::convert::Into<U> for vyre_libs::builder::BuildOptions where U: core::convert::From<T>
pub fn vyre_libs::builder::BuildOptions::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::builder::BuildOptions where U: core::convert::Into<T>
pub type vyre_libs::builder::BuildOptions::Error = core::convert::Infallible
pub fn vyre_libs::builder::BuildOptions::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::builder::BuildOptions where U: core::convert::TryFrom<T>
pub type vyre_libs::builder::BuildOptions::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::builder::BuildOptions::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::builder::BuildOptions where T: core::clone::Clone
pub type vyre_libs::builder::BuildOptions::Owned = T
pub fn vyre_libs::builder::BuildOptions::clone_into(&self, target: &mut T)
pub fn vyre_libs::builder::BuildOptions::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::builder::BuildOptions where T: 'static + ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::builder::BuildOptions where T: ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::builder::BuildOptions where T: ?core::marker::Sized
pub fn vyre_libs::builder::BuildOptions::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::builder::BuildOptions where T: core::clone::Clone
pub unsafe fn vyre_libs::builder::BuildOptions::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::builder::BuildOptions
pub fn vyre_libs::builder::BuildOptions::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::builder::BuildOptions
pub type vyre_libs::builder::BuildOptions::Init = T
pub const vyre_libs::builder::BuildOptions::ALIGN: usize
pub unsafe fn vyre_libs::builder::BuildOptions::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::builder::BuildOptions::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::builder::BuildOptions::drop(ptr: usize)
pub unsafe fn vyre_libs::builder::BuildOptions::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::builder::BuildOptions
impl<T> tracing::instrument::Instrument for vyre_libs::builder::BuildOptions
impl<T> tracing::instrument::WithSubscriber for vyre_libs::builder::BuildOptions
impl<T> typenum::type_operators::Same for vyre_libs::builder::BuildOptions
pub type vyre_libs::builder::BuildOptions::Output = T
pub struct vyre_libs::MatmulBias
impl vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBias::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
impl vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBias::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBias::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::clone(&self) -> vyre_libs::MatmulBias
impl core::fmt::Debug for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBias
impl core::marker::Send for vyre_libs::MatmulBias
impl core::marker::Sync for vyre_libs::MatmulBias
impl core::marker::Unpin for vyre_libs::MatmulBias
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBias
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBias
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBias
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBias where U: core::convert::From<T>
pub fn vyre_libs::MatmulBias::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBias where U: core::convert::Into<T>
pub type vyre_libs::MatmulBias::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBias::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBias where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBias::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBias::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBias where T: core::clone::Clone
pub type vyre_libs::MatmulBias::Owned = T
pub fn vyre_libs::MatmulBias::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBias::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBias where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBias::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBias where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBias::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBias where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBias::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBias where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBias::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBias
pub fn vyre_libs::MatmulBias::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBias
pub type vyre_libs::MatmulBias::Init = T
pub const vyre_libs::MatmulBias::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBias::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBias::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBias::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBias::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBias
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBias
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBias
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBias
pub type vyre_libs::MatmulBias::Output = T
pub struct vyre_libs::MatmulBiasTiled
impl vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulBiasTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulBiasTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, bias: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulBiasTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::clone(&self) -> vyre_libs::MatmulBiasTiled
impl core::fmt::Debug for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulBiasTiled
impl core::marker::Send for vyre_libs::MatmulBiasTiled
impl core::marker::Sync for vyre_libs::MatmulBiasTiled
impl core::marker::Unpin for vyre_libs::MatmulBiasTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulBiasTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulBiasTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulBiasTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulBiasTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulBiasTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulBiasTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulBiasTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulBiasTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulBiasTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulBiasTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub type vyre_libs::MatmulBiasTiled::Owned = T
pub fn vyre_libs::MatmulBiasTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulBiasTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulBiasTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulBiasTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulBiasTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulBiasTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulBiasTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulBiasTiled
pub fn vyre_libs::MatmulBiasTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Init = T
pub const vyre_libs::MatmulBiasTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulBiasTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulBiasTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulBiasTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulBiasTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulBiasTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulBiasTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulBiasTiled
pub type vyre_libs::MatmulBiasTiled::Output = T
pub struct vyre_libs::MatmulTiled
impl vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::auto(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef) -> Self
pub fn vyre_libs::MatmulTiled::build(self) -> core::result::Result<vyre_foundation::ir_inner::model::program::core::Program, vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::MatmulTiled::new(a: vyre_libs::tensor_ref::TensorRef, b: vyre_libs::tensor_ref::TensorRef, out: vyre_libs::tensor_ref::TensorRef, tile: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_region_generator(self, name: &'static str) -> Self
pub fn vyre_libs::MatmulTiled::with_tenant_id(self, tenant_id: u32) -> Self
pub fn vyre_libs::MatmulTiled::with_workgroup_size(self, size: [u32; 3]) -> Self
impl core::clone::Clone for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::clone(&self) -> vyre_libs::MatmulTiled
impl core::fmt::Debug for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::MatmulTiled
impl core::marker::Send for vyre_libs::MatmulTiled
impl core::marker::Sync for vyre_libs::MatmulTiled
impl core::marker::Unpin for vyre_libs::MatmulTiled
impl core::marker::UnsafeUnpin for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::MatmulTiled
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::MatmulTiled
impl<T, U> core::convert::Into<U> for vyre_libs::MatmulTiled where U: core::convert::From<T>
pub fn vyre_libs::MatmulTiled::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::MatmulTiled where U: core::convert::Into<T>
pub type vyre_libs::MatmulTiled::Error = core::convert::Infallible
pub fn vyre_libs::MatmulTiled::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::MatmulTiled where U: core::convert::TryFrom<T>
pub type vyre_libs::MatmulTiled::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::MatmulTiled::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::MatmulTiled where T: core::clone::Clone
pub type vyre_libs::MatmulTiled::Owned = T
pub fn vyre_libs::MatmulTiled::clone_into(&self, target: &mut T)
pub fn vyre_libs::MatmulTiled::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::MatmulTiled where T: 'static + ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::MatmulTiled where T: ?core::marker::Sized
pub fn vyre_libs::MatmulTiled::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::MatmulTiled where T: core::clone::Clone
pub unsafe fn vyre_libs::MatmulTiled::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::MatmulTiled
pub fn vyre_libs::MatmulTiled::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Init = T
pub const vyre_libs::MatmulTiled::ALIGN: usize
pub unsafe fn vyre_libs::MatmulTiled::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::MatmulTiled::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::MatmulTiled::drop(ptr: usize)
pub unsafe fn vyre_libs::MatmulTiled::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::MatmulTiled
impl<T> tracing::instrument::Instrument for vyre_libs::MatmulTiled
impl<T> tracing::instrument::WithSubscriber for vyre_libs::MatmulTiled
impl<T> typenum::type_operators::Same for vyre_libs::MatmulTiled
pub type vyre_libs::MatmulTiled::Output = T
#[non_exhaustive] pub struct vyre_libs::ProgramDescriptor
pub vyre_libs::ProgramDescriptor::buffer_count: usize
pub vyre_libs::ProgramDescriptor::buffers: alloc::vec::Vec<vyre_libs::descriptor::BufferDescriptor>
pub vyre_libs::ProgramDescriptor::entry_node_count: usize
pub vyre_libs::ProgramDescriptor::rw_bytes_lower_bound: usize
pub vyre_libs::ProgramDescriptor::workgroup_size: [u32; 3]
impl vyre_libs::descriptor::ProgramDescriptor
pub fn vyre_libs::descriptor::ProgramDescriptor::from_program(program: &vyre_foundation::ir_inner::model::program::core::Program) -> Self
pub fn vyre_libs::descriptor::ProgramDescriptor::new(buffer_count: usize, workgroup_size: [u32; 3], buffers: alloc::vec::Vec<vyre_libs::descriptor::BufferDescriptor>, rw_bytes_lower_bound: usize, entry_node_count: usize) -> Self
impl core::clone::Clone for vyre_libs::descriptor::ProgramDescriptor
pub fn vyre_libs::descriptor::ProgramDescriptor::clone(&self) -> vyre_libs::descriptor::ProgramDescriptor
impl core::fmt::Debug for vyre_libs::descriptor::ProgramDescriptor
pub fn vyre_libs::descriptor::ProgramDescriptor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_libs::descriptor::ProgramDescriptor
impl core::marker::Send for vyre_libs::descriptor::ProgramDescriptor
impl core::marker::Sync for vyre_libs::descriptor::ProgramDescriptor
impl core::marker::Unpin for vyre_libs::descriptor::ProgramDescriptor
impl core::marker::UnsafeUnpin for vyre_libs::descriptor::ProgramDescriptor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::descriptor::ProgramDescriptor
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::descriptor::ProgramDescriptor
impl<T, U> core::convert::Into<U> for vyre_libs::descriptor::ProgramDescriptor where U: core::convert::From<T>
pub fn vyre_libs::descriptor::ProgramDescriptor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::descriptor::ProgramDescriptor where U: core::convert::Into<T>
pub type vyre_libs::descriptor::ProgramDescriptor::Error = core::convert::Infallible
pub fn vyre_libs::descriptor::ProgramDescriptor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::descriptor::ProgramDescriptor where U: core::convert::TryFrom<T>
pub type vyre_libs::descriptor::ProgramDescriptor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::descriptor::ProgramDescriptor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::descriptor::ProgramDescriptor where T: core::clone::Clone
pub type vyre_libs::descriptor::ProgramDescriptor::Owned = T
pub fn vyre_libs::descriptor::ProgramDescriptor::clone_into(&self, target: &mut T)
pub fn vyre_libs::descriptor::ProgramDescriptor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::descriptor::ProgramDescriptor where T: 'static + ?core::marker::Sized
pub fn vyre_libs::descriptor::ProgramDescriptor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::descriptor::ProgramDescriptor where T: ?core::marker::Sized
pub fn vyre_libs::descriptor::ProgramDescriptor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::descriptor::ProgramDescriptor where T: ?core::marker::Sized
pub fn vyre_libs::descriptor::ProgramDescriptor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::descriptor::ProgramDescriptor where T: core::clone::Clone
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::descriptor::ProgramDescriptor
pub fn vyre_libs::descriptor::ProgramDescriptor::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::descriptor::ProgramDescriptor
pub type vyre_libs::descriptor::ProgramDescriptor::Init = T
pub const vyre_libs::descriptor::ProgramDescriptor::ALIGN: usize
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::drop(ptr: usize)
pub unsafe fn vyre_libs::descriptor::ProgramDescriptor::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::descriptor::ProgramDescriptor
impl<T> tracing::instrument::Instrument for vyre_libs::descriptor::ProgramDescriptor
impl<T> tracing::instrument::WithSubscriber for vyre_libs::descriptor::ProgramDescriptor
impl<T> typenum::type_operators::Same for vyre_libs::descriptor::ProgramDescriptor
pub type vyre_libs::descriptor::ProgramDescriptor::Output = T
#[non_exhaustive] pub struct vyre_libs::TensorRef
pub vyre_libs::TensorRef::dtype: vyre_spec::data_type::DataType
pub vyre_libs::TensorRef::name: vyre_foundation::ir_inner::model::expr::Ident
pub vyre_libs::TensorRef::shape: alloc::sync::Arc<[u32]>
impl vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::element_count(&self) -> core::option::Option<u32>
pub fn vyre_libs::tensor_ref::TensorRef::f16_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f16_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f32_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::f32_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::name_str(&self) -> &str
pub fn vyre_libs::tensor_ref::TensorRef::new(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, dtype: vyre_spec::data_type::DataType, shape: alloc::vec::Vec<u32>) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::u32_1d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, len: u32) -> Self
pub fn vyre_libs::tensor_ref::TensorRef::u32_2d(name: impl core::convert::Into<vyre_foundation::ir_inner::model::expr::Ident>, rows: u32, cols: u32) -> Self
impl core::clone::Clone for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::clone(&self) -> vyre_libs::tensor_ref::TensorRef
impl core::cmp::Eq for vyre_libs::tensor_ref::TensorRef
impl core::cmp::PartialEq for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::eq(&self, other: &vyre_libs::tensor_ref::TensorRef) -> bool
impl core::fmt::Debug for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_libs::tensor_ref::TensorRef
impl core::marker::Freeze for vyre_libs::tensor_ref::TensorRef
impl core::marker::Send for vyre_libs::tensor_ref::TensorRef
impl core::marker::Sync for vyre_libs::tensor_ref::TensorRef
impl core::marker::Unpin for vyre_libs::tensor_ref::TensorRef
impl core::marker::UnsafeUnpin for vyre_libs::tensor_ref::TensorRef
impl core::panic::unwind_safe::RefUnwindSafe for vyre_libs::tensor_ref::TensorRef
impl core::panic::unwind_safe::UnwindSafe for vyre_libs::tensor_ref::TensorRef
impl<Q, K> equivalent::Equivalent<K> for vyre_libs::tensor_ref::TensorRef where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_libs::tensor_ref::TensorRef where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::From<T>
pub fn vyre_libs::tensor_ref::TensorRef::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::Into<T>
pub type vyre_libs::tensor_ref::TensorRef::Error = core::convert::Infallible
pub fn vyre_libs::tensor_ref::TensorRef::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_libs::tensor_ref::TensorRef where U: core::convert::TryFrom<T>
pub type vyre_libs::tensor_ref::TensorRef::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_libs::tensor_ref::TensorRef::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_libs::tensor_ref::TensorRef where T: core::clone::Clone
pub type vyre_libs::tensor_ref::TensorRef::Owned = T
pub fn vyre_libs::tensor_ref::TensorRef::clone_into(&self, target: &mut T)
pub fn vyre_libs::tensor_ref::TensorRef::to_owned(&self) -> T
impl<T> core::any::Any for vyre_libs::tensor_ref::TensorRef where T: 'static + ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_libs::tensor_ref::TensorRef where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_libs::tensor_ref::TensorRef where T: ?core::marker::Sized
pub fn vyre_libs::tensor_ref::TensorRef::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_libs::tensor_ref::TensorRef where T: core::clone::Clone
pub unsafe fn vyre_libs::tensor_ref::TensorRef::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_libs::tensor_ref::TensorRef
pub fn vyre_libs::tensor_ref::TensorRef::from(t: T) -> T
impl<T> crossbeam_epoch::atomic::Pointable for vyre_libs::tensor_ref::TensorRef
pub type vyre_libs::tensor_ref::TensorRef::Init = T
pub const vyre_libs::tensor_ref::TensorRef::ALIGN: usize
pub unsafe fn vyre_libs::tensor_ref::TensorRef::deref<'a>(ptr: usize) -> &'a T
pub unsafe fn vyre_libs::tensor_ref::TensorRef::deref_mut<'a>(ptr: usize) -> &'a mut T
pub unsafe fn vyre_libs::tensor_ref::TensorRef::drop(ptr: usize)
pub unsafe fn vyre_libs::tensor_ref::TensorRef::init(init: <T as crossbeam_epoch::atomic::Pointable>::Init) -> usize
impl<T> either::into_either::IntoEither for vyre_libs::tensor_ref::TensorRef
impl<T> tracing::instrument::Instrument for vyre_libs::tensor_ref::TensorRef
impl<T> tracing::instrument::WithSubscriber for vyre_libs::tensor_ref::TensorRef
impl<T> typenum::type_operators::Same for vyre_libs::tensor_ref::TensorRef
pub type vyre_libs::tensor_ref::TensorRef::Output = T
pub const vyre_libs::BOOL_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::BYTES_TO_BYTES_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::BYTES_TO_BYTES_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::BYTES_TO_U32_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::F32_F32_F32_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::F32_F32_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::F32_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::F32_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::I32_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::U32_INPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::U32_OUTPUTS: &[vyre_spec::data_type::DataType]
pub const vyre_libs::U32_U32_INPUTS: &[vyre_spec::data_type::DataType]
pub fn vyre_libs::check_dtype(r: &vyre_libs::tensor_ref::TensorRef, expected: vyre_spec::data_type::DataType, op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::check_shape(r: &vyre_libs::tensor_ref::TensorRef, expected: &[u32], op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::check_tensors(op: &'static str, tensors: &[(&vyre_libs::tensor_ref::TensorRef, vyre_spec::data_type::DataType)]) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::check_unique_names(refs: &[&vyre_libs::tensor_ref::TensorRef], op: &'static str) -> core::result::Result<(), vyre_libs::tensor_ref::TensorRefError>
pub fn vyre_libs::matmul_bias_tiled(a: &str, b: &str, bias: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program
pub fn vyre_libs::matmul_tiled(a: &str, b: &str, out: &str, m: u32, k: u32, n: u32, tile: u32) -> vyre_foundation::ir_inner::model::program::core::Program

pub mod vyre_primitives
pub mod vyre_primitives::prelude
pub fn vyre_primitives::prelude::append_f32_slice_le_bytes(values: &[f32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::prelude::append_packed_byte_lane(bytes: &[u8], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::prelude::append_u32_slice_le_bytes(words: &[u32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::prelude::decode_f32_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<f32>
pub fn vyre_primitives::prelude::decode_i32_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<i32>
pub fn vyre_primitives::prelude::decode_u16_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<u16>
pub fn vyre_primitives::prelude::decode_u32_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<u32>
pub fn vyre_primitives::prelude::decode_u64_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<u64>
pub fn vyre_primitives::prelude::pack_bytes_as_u32_slice(bytes: &[u8]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::prelude::pack_bytes_as_u32_slice_min_words(bytes: &[u8], min_words: usize) -> core::result::Result<(alloc::vec::Vec<u8>, usize), alloc::string::String>
pub fn vyre_primitives::prelude::pack_f32_slice(values: &[f32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::prelude::pack_f32_slice_into(values: &[f32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::prelude::pack_f32_slice_into_uninit(values: &[f32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::prelude::pack_i32_slice(values: &[i32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::prelude::pack_i32_slice_into(values: &[i32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::prelude::pack_u16_slice(values: &[u16]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::prelude::pack_u16_slice_into(values: &[u16], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::prelude::pack_u32_slice(words: &[u32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::prelude::pack_u32_slice_into(words: &[u32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::prelude::pack_u32_slice_into_uninit(words: &[u32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::prelude::pack_u32_slice_min_words_into(words: &[u32], min_words: u32, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), alloc::string::String>
pub fn vyre_primitives::prelude::pack_u64_slice(values: &[u64]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::prelude::pack_u64_slice_into(values: &[u64], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::prelude::read_f32_le_word(bytes: &[u8], word_index: usize, label: &str) -> core::result::Result<f32, alloc::string::String>
pub fn vyre_primitives::prelude::read_u32_le_word(bytes: &[u8], word_index: usize, label: &str) -> core::result::Result<u32, alloc::string::String>
pub fn vyre_primitives::prelude::unpack_f32_slice(bytes: &[u8], count: usize, label: &str) -> core::result::Result<alloc::vec::Vec<f32>, alloc::string::String>
pub fn vyre_primitives::prelude::unpack_f32_slice_into(bytes: &[u8], count: usize, label: &str, out: &mut alloc::vec::Vec<f32>) -> core::result::Result<(), alloc::string::String>
pub fn vyre_primitives::prelude::unpack_u32_slice_into(bytes: &[u8], count: usize, label: &str, out: &mut alloc::vec::Vec<u32>) -> core::result::Result<(), alloc::string::String>
pub mod vyre_primitives::range
#[non_exhaustive] #[repr(C)] pub struct vyre_primitives::range::ByteRange
pub vyre_primitives::range::ByteRange::end: u32
pub vyre_primitives::range::ByteRange::start: u32
pub vyre_primitives::range::ByteRange::tag: u32
impl vyre_primitives::range::ByteRange
pub const fn vyre_primitives::range::ByteRange::contains(&self, other: &vyre_primitives::range::ByteRange) -> bool
pub const fn vyre_primitives::range::ByteRange::ends_before(&self, other: &vyre_primitives::range::ByteRange) -> bool
pub const fn vyre_primitives::range::ByteRange::is_empty(&self) -> bool
pub const fn vyre_primitives::range::ByteRange::len(&self) -> u32
pub const fn vyre_primitives::range::ByteRange::new(tag: u32, start: u32, end: u32) -> Self
impl core::clone::Clone for vyre_primitives::range::ByteRange
pub fn vyre_primitives::range::ByteRange::clone(&self) -> vyre_primitives::range::ByteRange
impl core::cmp::Eq for vyre_primitives::range::ByteRange
impl core::cmp::Ord for vyre_primitives::range::ByteRange
pub fn vyre_primitives::range::ByteRange::cmp(&self, other: &vyre_primitives::range::ByteRange) -> core::cmp::Ordering
impl core::cmp::PartialEq for vyre_primitives::range::ByteRange
pub fn vyre_primitives::range::ByteRange::eq(&self, other: &vyre_primitives::range::ByteRange) -> bool
impl core::cmp::PartialOrd for vyre_primitives::range::ByteRange
pub fn vyre_primitives::range::ByteRange::partial_cmp(&self, other: &vyre_primitives::range::ByteRange) -> core::option::Option<core::cmp::Ordering>
impl core::fmt::Debug for vyre_primitives::range::ByteRange
pub fn vyre_primitives::range::ByteRange::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::range::ByteRange
pub fn vyre_primitives::range::ByteRange::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::range::ByteRange
impl core::marker::StructuralPartialEq for vyre_primitives::range::ByteRange
impl core::marker::Freeze for vyre_primitives::range::ByteRange
impl core::marker::Send for vyre_primitives::range::ByteRange
impl core::marker::Sync for vyre_primitives::range::ByteRange
impl core::marker::Unpin for vyre_primitives::range::ByteRange
impl core::marker::UnsafeUnpin for vyre_primitives::range::ByteRange
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::range::ByteRange
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::range::ByteRange
impl<T, U> core::convert::Into<U> for vyre_primitives::range::ByteRange where U: core::convert::From<T>
pub fn vyre_primitives::range::ByteRange::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::range::ByteRange where U: core::convert::Into<T>
pub type vyre_primitives::range::ByteRange::Error = core::convert::Infallible
pub fn vyre_primitives::range::ByteRange::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::range::ByteRange where U: core::convert::TryFrom<T>
pub type vyre_primitives::range::ByteRange::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::range::ByteRange::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::range::ByteRange where T: core::clone::Clone
pub type vyre_primitives::range::ByteRange::Owned = T
pub fn vyre_primitives::range::ByteRange::clone_into(&self, target: &mut T)
pub fn vyre_primitives::range::ByteRange::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::range::ByteRange where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::range::ByteRange::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::range::ByteRange where T: ?core::marker::Sized
pub fn vyre_primitives::range::ByteRange::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::range::ByteRange where T: ?core::marker::Sized
pub fn vyre_primitives::range::ByteRange::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::range::ByteRange where T: core::clone::Clone
pub unsafe fn vyre_primitives::range::ByteRange::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::range::ByteRange
pub fn vyre_primitives::range::ByteRange::from(t: T) -> T
pub mod vyre_primitives::wire
pub fn vyre_primitives::wire::append_f32_slice_le_bytes(values: &[f32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::append_packed_byte_lane(bytes: &[u8], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::append_u32_slice_le_bytes(words: &[u32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::decode_f32_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<f32>
pub fn vyre_primitives::wire::decode_i32_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<i32>
pub fn vyre_primitives::wire::decode_u16_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<u16>
pub fn vyre_primitives::wire::decode_u32_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<u32>
pub fn vyre_primitives::wire::decode_u32x8_le_bytes(bytes: &[u8; 32]) -> [u32; 8]
pub fn vyre_primitives::wire::decode_u64_le_bytes_all(bytes: &[u8]) -> alloc::vec::Vec<u64>
pub fn vyre_primitives::wire::pack_bytes_as_u32_slice(bytes: &[u8]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::wire::pack_bytes_as_u32_slice_into(bytes: &[u8], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::pack_bytes_as_u32_slice_min_words(bytes: &[u8], min_words: usize) -> core::result::Result<(alloc::vec::Vec<u8>, usize), alloc::string::String>
pub fn vyre_primitives::wire::pack_bytes_as_u32_slice_min_words_into(bytes: &[u8], min_words: usize, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<usize, alloc::string::String>
pub fn vyre_primitives::wire::pack_f32_slice(values: &[f32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::wire::pack_f32_slice_into(values: &[f32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::pack_f32_slice_into_uninit(values: &[f32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::wire::pack_i32_slice(values: &[i32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::wire::pack_i32_slice_into(values: &[i32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::pack_u16_slice(values: &[u16]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::wire::pack_u16_slice_into(values: &[u16], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::pack_u32_iter<I>(words: I) -> alloc::vec::Vec<u8> where I: core::iter::traits::collect::IntoIterator<Item = u32>
pub fn vyre_primitives::wire::pack_u32_slice(words: &[u32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::wire::pack_u32_slice_into(words: &[u32], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::pack_u32_slice_into_uninit(words: &[u32]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::wire::pack_u32_slice_min_words_into(words: &[u32], min_words: u32, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), alloc::string::String>
pub fn vyre_primitives::wire::pack_u64_slice(values: &[u64]) -> alloc::vec::Vec<u8>
pub fn vyre_primitives::wire::pack_u64_slice_into(values: &[u64], out: &mut alloc::vec::Vec<u8>)
pub fn vyre_primitives::wire::read_f32_le_word(bytes: &[u8], word_index: usize, label: &str) -> core::result::Result<f32, alloc::string::String>
pub fn vyre_primitives::wire::read_u32_le_at(bytes: &[u8], byte_offset: usize, label: &str) -> core::result::Result<u32, alloc::string::String>
pub fn vyre_primitives::wire::read_u32_le_word(bytes: &[u8], word_index: usize, label: &str) -> core::result::Result<u32, alloc::string::String>
pub fn vyre_primitives::wire::try_pack_bytes_as_u32_slice_into(bytes: &[u8], out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), alloc::string::String>
pub fn vyre_primitives::wire::try_pack_f32_slice_into(values: &[f32], out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), alloc::string::String>
pub fn vyre_primitives::wire::try_pack_u32_slice_into(words: &[u32], out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), alloc::string::String>
pub fn vyre_primitives::wire::unpack_f32_slice(bytes: &[u8], count: usize, label: &str) -> core::result::Result<alloc::vec::Vec<f32>, alloc::string::String>
pub fn vyre_primitives::wire::unpack_f32_slice_into(bytes: &[u8], count: usize, label: &str, out: &mut alloc::vec::Vec<f32>) -> core::result::Result<(), alloc::string::String>
pub fn vyre_primitives::wire::unpack_u32_slice_into(bytes: &[u8], count: usize, label: &str, out: &mut alloc::vec::Vec<u32>) -> core::result::Result<(), alloc::string::String>
#[non_exhaustive] pub enum vyre_primitives::CombineOp
pub vyre_primitives::CombineOp::Add
pub vyre_primitives::CombineOp::BitAnd
pub vyre_primitives::CombineOp::BitOr
pub vyre_primitives::CombineOp::BitXor
pub vyre_primitives::CombineOp::Max
pub vyre_primitives::CombineOp::Min
pub vyre_primitives::CombineOp::Mul
impl core::clone::Clone for vyre_primitives::CombineOp
pub fn vyre_primitives::CombineOp::clone(&self) -> vyre_primitives::CombineOp
impl core::cmp::Eq for vyre_primitives::CombineOp
impl core::cmp::PartialEq for vyre_primitives::CombineOp
pub fn vyre_primitives::CombineOp::eq(&self, other: &vyre_primitives::CombineOp) -> bool
impl core::default::Default for vyre_primitives::CombineOp
pub fn vyre_primitives::CombineOp::default() -> vyre_primitives::CombineOp
impl core::fmt::Debug for vyre_primitives::CombineOp
pub fn vyre_primitives::CombineOp::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::CombineOp
pub fn vyre_primitives::CombineOp::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::CombineOp
impl core::marker::StructuralPartialEq for vyre_primitives::CombineOp
impl core::marker::Freeze for vyre_primitives::CombineOp
impl core::marker::Send for vyre_primitives::CombineOp
impl core::marker::Sync for vyre_primitives::CombineOp
impl core::marker::Unpin for vyre_primitives::CombineOp
impl core::marker::UnsafeUnpin for vyre_primitives::CombineOp
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::CombineOp
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::CombineOp
impl<T, U> core::convert::Into<U> for vyre_primitives::CombineOp where U: core::convert::From<T>
pub fn vyre_primitives::CombineOp::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::CombineOp where U: core::convert::Into<T>
pub type vyre_primitives::CombineOp::Error = core::convert::Infallible
pub fn vyre_primitives::CombineOp::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::CombineOp where U: core::convert::TryFrom<T>
pub type vyre_primitives::CombineOp::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::CombineOp::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::CombineOp where T: core::clone::Clone
pub type vyre_primitives::CombineOp::Owned = T
pub fn vyre_primitives::CombineOp::clone_into(&self, target: &mut T)
pub fn vyre_primitives::CombineOp::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::CombineOp where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::CombineOp::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::CombineOp where T: ?core::marker::Sized
pub fn vyre_primitives::CombineOp::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::CombineOp where T: ?core::marker::Sized
pub fn vyre_primitives::CombineOp::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::CombineOp where T: core::clone::Clone
pub unsafe fn vyre_primitives::CombineOp::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::CombineOp
pub fn vyre_primitives::CombineOp::from(t: T) -> T
pub struct vyre_primitives::ArithAdd
impl core::clone::Clone for vyre_primitives::ArithAdd
pub fn vyre_primitives::ArithAdd::clone(&self) -> vyre_primitives::ArithAdd
impl core::cmp::Eq for vyre_primitives::ArithAdd
impl core::cmp::PartialEq for vyre_primitives::ArithAdd
pub fn vyre_primitives::ArithAdd::eq(&self, other: &vyre_primitives::ArithAdd) -> bool
impl core::default::Default for vyre_primitives::ArithAdd
pub fn vyre_primitives::ArithAdd::default() -> vyre_primitives::ArithAdd
impl core::fmt::Debug for vyre_primitives::ArithAdd
pub fn vyre_primitives::ArithAdd::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::ArithAdd
pub fn vyre_primitives::ArithAdd::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::ArithAdd
impl core::marker::StructuralPartialEq for vyre_primitives::ArithAdd
impl core::marker::Freeze for vyre_primitives::ArithAdd
impl core::marker::Send for vyre_primitives::ArithAdd
impl core::marker::Sync for vyre_primitives::ArithAdd
impl core::marker::Unpin for vyre_primitives::ArithAdd
impl core::marker::UnsafeUnpin for vyre_primitives::ArithAdd
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::ArithAdd
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::ArithAdd
impl<T, U> core::convert::Into<U> for vyre_primitives::ArithAdd where U: core::convert::From<T>
pub fn vyre_primitives::ArithAdd::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::ArithAdd where U: core::convert::Into<T>
pub type vyre_primitives::ArithAdd::Error = core::convert::Infallible
pub fn vyre_primitives::ArithAdd::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::ArithAdd where U: core::convert::TryFrom<T>
pub type vyre_primitives::ArithAdd::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::ArithAdd::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::ArithAdd where T: core::clone::Clone
pub type vyre_primitives::ArithAdd::Owned = T
pub fn vyre_primitives::ArithAdd::clone_into(&self, target: &mut T)
pub fn vyre_primitives::ArithAdd::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::ArithAdd where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::ArithAdd::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::ArithAdd where T: ?core::marker::Sized
pub fn vyre_primitives::ArithAdd::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::ArithAdd where T: ?core::marker::Sized
pub fn vyre_primitives::ArithAdd::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::ArithAdd where T: core::clone::Clone
pub unsafe fn vyre_primitives::ArithAdd::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::ArithAdd
pub fn vyre_primitives::ArithAdd::from(t: T) -> T
pub struct vyre_primitives::ArithMul
impl core::clone::Clone for vyre_primitives::ArithMul
pub fn vyre_primitives::ArithMul::clone(&self) -> vyre_primitives::ArithMul
impl core::cmp::Eq for vyre_primitives::ArithMul
impl core::cmp::PartialEq for vyre_primitives::ArithMul
pub fn vyre_primitives::ArithMul::eq(&self, other: &vyre_primitives::ArithMul) -> bool
impl core::default::Default for vyre_primitives::ArithMul
pub fn vyre_primitives::ArithMul::default() -> vyre_primitives::ArithMul
impl core::fmt::Debug for vyre_primitives::ArithMul
pub fn vyre_primitives::ArithMul::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::ArithMul
pub fn vyre_primitives::ArithMul::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::ArithMul
impl core::marker::StructuralPartialEq for vyre_primitives::ArithMul
impl core::marker::Freeze for vyre_primitives::ArithMul
impl core::marker::Send for vyre_primitives::ArithMul
impl core::marker::Sync for vyre_primitives::ArithMul
impl core::marker::Unpin for vyre_primitives::ArithMul
impl core::marker::UnsafeUnpin for vyre_primitives::ArithMul
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::ArithMul
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::ArithMul
impl<T, U> core::convert::Into<U> for vyre_primitives::ArithMul where U: core::convert::From<T>
pub fn vyre_primitives::ArithMul::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::ArithMul where U: core::convert::Into<T>
pub type vyre_primitives::ArithMul::Error = core::convert::Infallible
pub fn vyre_primitives::ArithMul::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::ArithMul where U: core::convert::TryFrom<T>
pub type vyre_primitives::ArithMul::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::ArithMul::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::ArithMul where T: core::clone::Clone
pub type vyre_primitives::ArithMul::Owned = T
pub fn vyre_primitives::ArithMul::clone_into(&self, target: &mut T)
pub fn vyre_primitives::ArithMul::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::ArithMul where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::ArithMul::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::ArithMul where T: ?core::marker::Sized
pub fn vyre_primitives::ArithMul::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::ArithMul where T: ?core::marker::Sized
pub fn vyre_primitives::ArithMul::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::ArithMul where T: core::clone::Clone
pub unsafe fn vyre_primitives::ArithMul::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::ArithMul
pub fn vyre_primitives::ArithMul::from(t: T) -> T
pub struct vyre_primitives::BitwiseAnd
impl core::clone::Clone for vyre_primitives::BitwiseAnd
pub fn vyre_primitives::BitwiseAnd::clone(&self) -> vyre_primitives::BitwiseAnd
impl core::cmp::Eq for vyre_primitives::BitwiseAnd
impl core::cmp::PartialEq for vyre_primitives::BitwiseAnd
pub fn vyre_primitives::BitwiseAnd::eq(&self, other: &vyre_primitives::BitwiseAnd) -> bool
impl core::default::Default for vyre_primitives::BitwiseAnd
pub fn vyre_primitives::BitwiseAnd::default() -> vyre_primitives::BitwiseAnd
impl core::fmt::Debug for vyre_primitives::BitwiseAnd
pub fn vyre_primitives::BitwiseAnd::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::BitwiseAnd
pub fn vyre_primitives::BitwiseAnd::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::BitwiseAnd
impl core::marker::StructuralPartialEq for vyre_primitives::BitwiseAnd
impl core::marker::Freeze for vyre_primitives::BitwiseAnd
impl core::marker::Send for vyre_primitives::BitwiseAnd
impl core::marker::Sync for vyre_primitives::BitwiseAnd
impl core::marker::Unpin for vyre_primitives::BitwiseAnd
impl core::marker::UnsafeUnpin for vyre_primitives::BitwiseAnd
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::BitwiseAnd
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::BitwiseAnd
impl<T, U> core::convert::Into<U> for vyre_primitives::BitwiseAnd where U: core::convert::From<T>
pub fn vyre_primitives::BitwiseAnd::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::BitwiseAnd where U: core::convert::Into<T>
pub type vyre_primitives::BitwiseAnd::Error = core::convert::Infallible
pub fn vyre_primitives::BitwiseAnd::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::BitwiseAnd where U: core::convert::TryFrom<T>
pub type vyre_primitives::BitwiseAnd::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::BitwiseAnd::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::BitwiseAnd where T: core::clone::Clone
pub type vyre_primitives::BitwiseAnd::Owned = T
pub fn vyre_primitives::BitwiseAnd::clone_into(&self, target: &mut T)
pub fn vyre_primitives::BitwiseAnd::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::BitwiseAnd where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::BitwiseAnd::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::BitwiseAnd where T: ?core::marker::Sized
pub fn vyre_primitives::BitwiseAnd::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::BitwiseAnd where T: ?core::marker::Sized
pub fn vyre_primitives::BitwiseAnd::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::BitwiseAnd where T: core::clone::Clone
pub unsafe fn vyre_primitives::BitwiseAnd::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::BitwiseAnd
pub fn vyre_primitives::BitwiseAnd::from(t: T) -> T
pub struct vyre_primitives::BitwiseOr
impl core::clone::Clone for vyre_primitives::BitwiseOr
pub fn vyre_primitives::BitwiseOr::clone(&self) -> vyre_primitives::BitwiseOr
impl core::cmp::Eq for vyre_primitives::BitwiseOr
impl core::cmp::PartialEq for vyre_primitives::BitwiseOr
pub fn vyre_primitives::BitwiseOr::eq(&self, other: &vyre_primitives::BitwiseOr) -> bool
impl core::default::Default for vyre_primitives::BitwiseOr
pub fn vyre_primitives::BitwiseOr::default() -> vyre_primitives::BitwiseOr
impl core::fmt::Debug for vyre_primitives::BitwiseOr
pub fn vyre_primitives::BitwiseOr::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::BitwiseOr
pub fn vyre_primitives::BitwiseOr::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::BitwiseOr
impl core::marker::StructuralPartialEq for vyre_primitives::BitwiseOr
impl core::marker::Freeze for vyre_primitives::BitwiseOr
impl core::marker::Send for vyre_primitives::BitwiseOr
impl core::marker::Sync for vyre_primitives::BitwiseOr
impl core::marker::Unpin for vyre_primitives::BitwiseOr
impl core::marker::UnsafeUnpin for vyre_primitives::BitwiseOr
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::BitwiseOr
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::BitwiseOr
impl<T, U> core::convert::Into<U> for vyre_primitives::BitwiseOr where U: core::convert::From<T>
pub fn vyre_primitives::BitwiseOr::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::BitwiseOr where U: core::convert::Into<T>
pub type vyre_primitives::BitwiseOr::Error = core::convert::Infallible
pub fn vyre_primitives::BitwiseOr::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::BitwiseOr where U: core::convert::TryFrom<T>
pub type vyre_primitives::BitwiseOr::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::BitwiseOr::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::BitwiseOr where T: core::clone::Clone
pub type vyre_primitives::BitwiseOr::Owned = T
pub fn vyre_primitives::BitwiseOr::clone_into(&self, target: &mut T)
pub fn vyre_primitives::BitwiseOr::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::BitwiseOr where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::BitwiseOr::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::BitwiseOr where T: ?core::marker::Sized
pub fn vyre_primitives::BitwiseOr::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::BitwiseOr where T: ?core::marker::Sized
pub fn vyre_primitives::BitwiseOr::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::BitwiseOr where T: core::clone::Clone
pub unsafe fn vyre_primitives::BitwiseOr::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::BitwiseOr
pub fn vyre_primitives::BitwiseOr::from(t: T) -> T
pub struct vyre_primitives::BitwiseXor
impl core::clone::Clone for vyre_primitives::BitwiseXor
pub fn vyre_primitives::BitwiseXor::clone(&self) -> vyre_primitives::BitwiseXor
impl core::cmp::Eq for vyre_primitives::BitwiseXor
impl core::cmp::PartialEq for vyre_primitives::BitwiseXor
pub fn vyre_primitives::BitwiseXor::eq(&self, other: &vyre_primitives::BitwiseXor) -> bool
impl core::default::Default for vyre_primitives::BitwiseXor
pub fn vyre_primitives::BitwiseXor::default() -> vyre_primitives::BitwiseXor
impl core::fmt::Debug for vyre_primitives::BitwiseXor
pub fn vyre_primitives::BitwiseXor::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::BitwiseXor
pub fn vyre_primitives::BitwiseXor::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::BitwiseXor
impl core::marker::StructuralPartialEq for vyre_primitives::BitwiseXor
impl core::marker::Freeze for vyre_primitives::BitwiseXor
impl core::marker::Send for vyre_primitives::BitwiseXor
impl core::marker::Sync for vyre_primitives::BitwiseXor
impl core::marker::Unpin for vyre_primitives::BitwiseXor
impl core::marker::UnsafeUnpin for vyre_primitives::BitwiseXor
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::BitwiseXor
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::BitwiseXor
impl<T, U> core::convert::Into<U> for vyre_primitives::BitwiseXor where U: core::convert::From<T>
pub fn vyre_primitives::BitwiseXor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::BitwiseXor where U: core::convert::Into<T>
pub type vyre_primitives::BitwiseXor::Error = core::convert::Infallible
pub fn vyre_primitives::BitwiseXor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::BitwiseXor where U: core::convert::TryFrom<T>
pub type vyre_primitives::BitwiseXor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::BitwiseXor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::BitwiseXor where T: core::clone::Clone
pub type vyre_primitives::BitwiseXor::Owned = T
pub fn vyre_primitives::BitwiseXor::clone_into(&self, target: &mut T)
pub fn vyre_primitives::BitwiseXor::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::BitwiseXor where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::BitwiseXor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::BitwiseXor where T: ?core::marker::Sized
pub fn vyre_primitives::BitwiseXor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::BitwiseXor where T: ?core::marker::Sized
pub fn vyre_primitives::BitwiseXor::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::BitwiseXor where T: core::clone::Clone
pub unsafe fn vyre_primitives::BitwiseXor::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::BitwiseXor
pub fn vyre_primitives::BitwiseXor::from(t: T) -> T
pub struct vyre_primitives::Clz
impl core::clone::Clone for vyre_primitives::Clz
pub fn vyre_primitives::Clz::clone(&self) -> vyre_primitives::Clz
impl core::cmp::Eq for vyre_primitives::Clz
impl core::cmp::PartialEq for vyre_primitives::Clz
pub fn vyre_primitives::Clz::eq(&self, other: &vyre_primitives::Clz) -> bool
impl core::default::Default for vyre_primitives::Clz
pub fn vyre_primitives::Clz::default() -> vyre_primitives::Clz
impl core::fmt::Debug for vyre_primitives::Clz
pub fn vyre_primitives::Clz::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::Clz
pub fn vyre_primitives::Clz::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::Clz
impl core::marker::StructuralPartialEq for vyre_primitives::Clz
impl core::marker::Freeze for vyre_primitives::Clz
impl core::marker::Send for vyre_primitives::Clz
impl core::marker::Sync for vyre_primitives::Clz
impl core::marker::Unpin for vyre_primitives::Clz
impl core::marker::UnsafeUnpin for vyre_primitives::Clz
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::Clz
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::Clz
impl<T, U> core::convert::Into<U> for vyre_primitives::Clz where U: core::convert::From<T>
pub fn vyre_primitives::Clz::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::Clz where U: core::convert::Into<T>
pub type vyre_primitives::Clz::Error = core::convert::Infallible
pub fn vyre_primitives::Clz::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::Clz where U: core::convert::TryFrom<T>
pub type vyre_primitives::Clz::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::Clz::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::Clz where T: core::clone::Clone
pub type vyre_primitives::Clz::Owned = T
pub fn vyre_primitives::Clz::clone_into(&self, target: &mut T)
pub fn vyre_primitives::Clz::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::Clz where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::Clz::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::Clz where T: ?core::marker::Sized
pub fn vyre_primitives::Clz::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::Clz where T: ?core::marker::Sized
pub fn vyre_primitives::Clz::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::Clz where T: core::clone::Clone
pub unsafe fn vyre_primitives::Clz::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::Clz
pub fn vyre_primitives::Clz::from(t: T) -> T
pub struct vyre_primitives::CompareEq
impl core::clone::Clone for vyre_primitives::CompareEq
pub fn vyre_primitives::CompareEq::clone(&self) -> vyre_primitives::CompareEq
impl core::cmp::Eq for vyre_primitives::CompareEq
impl core::cmp::PartialEq for vyre_primitives::CompareEq
pub fn vyre_primitives::CompareEq::eq(&self, other: &vyre_primitives::CompareEq) -> bool
impl core::default::Default for vyre_primitives::CompareEq
pub fn vyre_primitives::CompareEq::default() -> vyre_primitives::CompareEq
impl core::fmt::Debug for vyre_primitives::CompareEq
pub fn vyre_primitives::CompareEq::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::CompareEq
pub fn vyre_primitives::CompareEq::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::CompareEq
impl core::marker::StructuralPartialEq for vyre_primitives::CompareEq
impl core::marker::Freeze for vyre_primitives::CompareEq
impl core::marker::Send for vyre_primitives::CompareEq
impl core::marker::Sync for vyre_primitives::CompareEq
impl core::marker::Unpin for vyre_primitives::CompareEq
impl core::marker::UnsafeUnpin for vyre_primitives::CompareEq
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::CompareEq
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::CompareEq
impl<T, U> core::convert::Into<U> for vyre_primitives::CompareEq where U: core::convert::From<T>
pub fn vyre_primitives::CompareEq::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::CompareEq where U: core::convert::Into<T>
pub type vyre_primitives::CompareEq::Error = core::convert::Infallible
pub fn vyre_primitives::CompareEq::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::CompareEq where U: core::convert::TryFrom<T>
pub type vyre_primitives::CompareEq::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::CompareEq::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::CompareEq where T: core::clone::Clone
pub type vyre_primitives::CompareEq::Owned = T
pub fn vyre_primitives::CompareEq::clone_into(&self, target: &mut T)
pub fn vyre_primitives::CompareEq::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::CompareEq where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::CompareEq::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::CompareEq where T: ?core::marker::Sized
pub fn vyre_primitives::CompareEq::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::CompareEq where T: ?core::marker::Sized
pub fn vyre_primitives::CompareEq::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::CompareEq where T: core::clone::Clone
pub unsafe fn vyre_primitives::CompareEq::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::CompareEq
pub fn vyre_primitives::CompareEq::from(t: T) -> T
pub struct vyre_primitives::CompareLt
impl core::clone::Clone for vyre_primitives::CompareLt
pub fn vyre_primitives::CompareLt::clone(&self) -> vyre_primitives::CompareLt
impl core::cmp::Eq for vyre_primitives::CompareLt
impl core::cmp::PartialEq for vyre_primitives::CompareLt
pub fn vyre_primitives::CompareLt::eq(&self, other: &vyre_primitives::CompareLt) -> bool
impl core::default::Default for vyre_primitives::CompareLt
pub fn vyre_primitives::CompareLt::default() -> vyre_primitives::CompareLt
impl core::fmt::Debug for vyre_primitives::CompareLt
pub fn vyre_primitives::CompareLt::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::CompareLt
pub fn vyre_primitives::CompareLt::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::CompareLt
impl core::marker::StructuralPartialEq for vyre_primitives::CompareLt
impl core::marker::Freeze for vyre_primitives::CompareLt
impl core::marker::Send for vyre_primitives::CompareLt
impl core::marker::Sync for vyre_primitives::CompareLt
impl core::marker::Unpin for vyre_primitives::CompareLt
impl core::marker::UnsafeUnpin for vyre_primitives::CompareLt
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::CompareLt
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::CompareLt
impl<T, U> core::convert::Into<U> for vyre_primitives::CompareLt where U: core::convert::From<T>
pub fn vyre_primitives::CompareLt::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::CompareLt where U: core::convert::Into<T>
pub type vyre_primitives::CompareLt::Error = core::convert::Infallible
pub fn vyre_primitives::CompareLt::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::CompareLt where U: core::convert::TryFrom<T>
pub type vyre_primitives::CompareLt::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::CompareLt::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::CompareLt where T: core::clone::Clone
pub type vyre_primitives::CompareLt::Owned = T
pub fn vyre_primitives::CompareLt::clone_into(&self, target: &mut T)
pub fn vyre_primitives::CompareLt::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::CompareLt where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::CompareLt::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::CompareLt where T: ?core::marker::Sized
pub fn vyre_primitives::CompareLt::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::CompareLt where T: ?core::marker::Sized
pub fn vyre_primitives::CompareLt::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::CompareLt where T: core::clone::Clone
pub unsafe fn vyre_primitives::CompareLt::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::CompareLt
pub fn vyre_primitives::CompareLt::from(t: T) -> T
pub struct vyre_primitives::Gather
impl core::clone::Clone for vyre_primitives::Gather
pub fn vyre_primitives::Gather::clone(&self) -> vyre_primitives::Gather
impl core::cmp::Eq for vyre_primitives::Gather
impl core::cmp::PartialEq for vyre_primitives::Gather
pub fn vyre_primitives::Gather::eq(&self, other: &vyre_primitives::Gather) -> bool
impl core::default::Default for vyre_primitives::Gather
pub fn vyre_primitives::Gather::default() -> vyre_primitives::Gather
impl core::fmt::Debug for vyre_primitives::Gather
pub fn vyre_primitives::Gather::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::Gather
pub fn vyre_primitives::Gather::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::Gather
impl core::marker::StructuralPartialEq for vyre_primitives::Gather
impl core::marker::Freeze for vyre_primitives::Gather
impl core::marker::Send for vyre_primitives::Gather
impl core::marker::Sync for vyre_primitives::Gather
impl core::marker::Unpin for vyre_primitives::Gather
impl core::marker::UnsafeUnpin for vyre_primitives::Gather
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::Gather
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::Gather
impl<T, U> core::convert::Into<U> for vyre_primitives::Gather where U: core::convert::From<T>
pub fn vyre_primitives::Gather::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::Gather where U: core::convert::Into<T>
pub type vyre_primitives::Gather::Error = core::convert::Infallible
pub fn vyre_primitives::Gather::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::Gather where U: core::convert::TryFrom<T>
pub type vyre_primitives::Gather::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::Gather::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::Gather where T: core::clone::Clone
pub type vyre_primitives::Gather::Owned = T
pub fn vyre_primitives::Gather::clone_into(&self, target: &mut T)
pub fn vyre_primitives::Gather::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::Gather where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::Gather::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::Gather where T: ?core::marker::Sized
pub fn vyre_primitives::Gather::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::Gather where T: ?core::marker::Sized
pub fn vyre_primitives::Gather::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::Gather where T: core::clone::Clone
pub unsafe fn vyre_primitives::Gather::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::Gather
pub fn vyre_primitives::Gather::from(t: T) -> T
pub struct vyre_primitives::HashBlake3
impl core::clone::Clone for vyre_primitives::HashBlake3
pub fn vyre_primitives::HashBlake3::clone(&self) -> vyre_primitives::HashBlake3
impl core::cmp::Eq for vyre_primitives::HashBlake3
impl core::cmp::PartialEq for vyre_primitives::HashBlake3
pub fn vyre_primitives::HashBlake3::eq(&self, other: &vyre_primitives::HashBlake3) -> bool
impl core::default::Default for vyre_primitives::HashBlake3
pub fn vyre_primitives::HashBlake3::default() -> vyre_primitives::HashBlake3
impl core::fmt::Debug for vyre_primitives::HashBlake3
pub fn vyre_primitives::HashBlake3::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::HashBlake3
pub fn vyre_primitives::HashBlake3::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::HashBlake3
impl core::marker::StructuralPartialEq for vyre_primitives::HashBlake3
impl core::marker::Freeze for vyre_primitives::HashBlake3
impl core::marker::Send for vyre_primitives::HashBlake3
impl core::marker::Sync for vyre_primitives::HashBlake3
impl core::marker::Unpin for vyre_primitives::HashBlake3
impl core::marker::UnsafeUnpin for vyre_primitives::HashBlake3
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::HashBlake3
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::HashBlake3
impl<T, U> core::convert::Into<U> for vyre_primitives::HashBlake3 where U: core::convert::From<T>
pub fn vyre_primitives::HashBlake3::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::HashBlake3 where U: core::convert::Into<T>
pub type vyre_primitives::HashBlake3::Error = core::convert::Infallible
pub fn vyre_primitives::HashBlake3::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::HashBlake3 where U: core::convert::TryFrom<T>
pub type vyre_primitives::HashBlake3::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::HashBlake3::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::HashBlake3 where T: core::clone::Clone
pub type vyre_primitives::HashBlake3::Owned = T
pub fn vyre_primitives::HashBlake3::clone_into(&self, target: &mut T)
pub fn vyre_primitives::HashBlake3::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::HashBlake3 where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::HashBlake3::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::HashBlake3 where T: ?core::marker::Sized
pub fn vyre_primitives::HashBlake3::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::HashBlake3 where T: ?core::marker::Sized
pub fn vyre_primitives::HashBlake3::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::HashBlake3 where T: core::clone::Clone
pub unsafe fn vyre_primitives::HashBlake3::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::HashBlake3
pub fn vyre_primitives::HashBlake3::from(t: T) -> T
pub struct vyre_primitives::HashFnv1a
impl core::clone::Clone for vyre_primitives::HashFnv1a
pub fn vyre_primitives::HashFnv1a::clone(&self) -> vyre_primitives::HashFnv1a
impl core::cmp::Eq for vyre_primitives::HashFnv1a
impl core::cmp::PartialEq for vyre_primitives::HashFnv1a
pub fn vyre_primitives::HashFnv1a::eq(&self, other: &vyre_primitives::HashFnv1a) -> bool
impl core::default::Default for vyre_primitives::HashFnv1a
pub fn vyre_primitives::HashFnv1a::default() -> vyre_primitives::HashFnv1a
impl core::fmt::Debug for vyre_primitives::HashFnv1a
pub fn vyre_primitives::HashFnv1a::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::HashFnv1a
pub fn vyre_primitives::HashFnv1a::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::HashFnv1a
impl core::marker::StructuralPartialEq for vyre_primitives::HashFnv1a
impl core::marker::Freeze for vyre_primitives::HashFnv1a
impl core::marker::Send for vyre_primitives::HashFnv1a
impl core::marker::Sync for vyre_primitives::HashFnv1a
impl core::marker::Unpin for vyre_primitives::HashFnv1a
impl core::marker::UnsafeUnpin for vyre_primitives::HashFnv1a
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::HashFnv1a
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::HashFnv1a
impl<T, U> core::convert::Into<U> for vyre_primitives::HashFnv1a where U: core::convert::From<T>
pub fn vyre_primitives::HashFnv1a::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::HashFnv1a where U: core::convert::Into<T>
pub type vyre_primitives::HashFnv1a::Error = core::convert::Infallible
pub fn vyre_primitives::HashFnv1a::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::HashFnv1a where U: core::convert::TryFrom<T>
pub type vyre_primitives::HashFnv1a::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::HashFnv1a::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::HashFnv1a where T: core::clone::Clone
pub type vyre_primitives::HashFnv1a::Owned = T
pub fn vyre_primitives::HashFnv1a::clone_into(&self, target: &mut T)
pub fn vyre_primitives::HashFnv1a::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::HashFnv1a where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::HashFnv1a::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::HashFnv1a where T: ?core::marker::Sized
pub fn vyre_primitives::HashFnv1a::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::HashFnv1a where T: ?core::marker::Sized
pub fn vyre_primitives::HashFnv1a::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::HashFnv1a where T: core::clone::Clone
pub unsafe fn vyre_primitives::HashFnv1a::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::HashFnv1a
pub fn vyre_primitives::HashFnv1a::from(t: T) -> T
pub struct vyre_primitives::PatternMatchDfa
pub vyre_primitives::PatternMatchDfa::dfa: alloc::vec::Vec<u8>
impl core::clone::Clone for vyre_primitives::PatternMatchDfa
pub fn vyre_primitives::PatternMatchDfa::clone(&self) -> vyre_primitives::PatternMatchDfa
impl core::cmp::Eq for vyre_primitives::PatternMatchDfa
impl core::cmp::PartialEq for vyre_primitives::PatternMatchDfa
pub fn vyre_primitives::PatternMatchDfa::eq(&self, other: &vyre_primitives::PatternMatchDfa) -> bool
impl core::default::Default for vyre_primitives::PatternMatchDfa
pub fn vyre_primitives::PatternMatchDfa::default() -> vyre_primitives::PatternMatchDfa
impl core::fmt::Debug for vyre_primitives::PatternMatchDfa
pub fn vyre_primitives::PatternMatchDfa::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::PatternMatchDfa
pub fn vyre_primitives::PatternMatchDfa::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_primitives::PatternMatchDfa
impl core::marker::Freeze for vyre_primitives::PatternMatchDfa
impl core::marker::Send for vyre_primitives::PatternMatchDfa
impl core::marker::Sync for vyre_primitives::PatternMatchDfa
impl core::marker::Unpin for vyre_primitives::PatternMatchDfa
impl core::marker::UnsafeUnpin for vyre_primitives::PatternMatchDfa
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::PatternMatchDfa
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::PatternMatchDfa
impl<T, U> core::convert::Into<U> for vyre_primitives::PatternMatchDfa where U: core::convert::From<T>
pub fn vyre_primitives::PatternMatchDfa::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::PatternMatchDfa where U: core::convert::Into<T>
pub type vyre_primitives::PatternMatchDfa::Error = core::convert::Infallible
pub fn vyre_primitives::PatternMatchDfa::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::PatternMatchDfa where U: core::convert::TryFrom<T>
pub type vyre_primitives::PatternMatchDfa::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::PatternMatchDfa::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::PatternMatchDfa where T: core::clone::Clone
pub type vyre_primitives::PatternMatchDfa::Owned = T
pub fn vyre_primitives::PatternMatchDfa::clone_into(&self, target: &mut T)
pub fn vyre_primitives::PatternMatchDfa::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::PatternMatchDfa where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::PatternMatchDfa::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::PatternMatchDfa where T: ?core::marker::Sized
pub fn vyre_primitives::PatternMatchDfa::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::PatternMatchDfa where T: ?core::marker::Sized
pub fn vyre_primitives::PatternMatchDfa::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::PatternMatchDfa where T: core::clone::Clone
pub unsafe fn vyre_primitives::PatternMatchDfa::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::PatternMatchDfa
pub fn vyre_primitives::PatternMatchDfa::from(t: T) -> T
pub struct vyre_primitives::PatternMatchLiteral
pub vyre_primitives::PatternMatchLiteral::literal: alloc::vec::Vec<u8>
impl core::clone::Clone for vyre_primitives::PatternMatchLiteral
pub fn vyre_primitives::PatternMatchLiteral::clone(&self) -> vyre_primitives::PatternMatchLiteral
impl core::cmp::Eq for vyre_primitives::PatternMatchLiteral
impl core::cmp::PartialEq for vyre_primitives::PatternMatchLiteral
pub fn vyre_primitives::PatternMatchLiteral::eq(&self, other: &vyre_primitives::PatternMatchLiteral) -> bool
impl core::default::Default for vyre_primitives::PatternMatchLiteral
pub fn vyre_primitives::PatternMatchLiteral::default() -> vyre_primitives::PatternMatchLiteral
impl core::fmt::Debug for vyre_primitives::PatternMatchLiteral
pub fn vyre_primitives::PatternMatchLiteral::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::PatternMatchLiteral
pub fn vyre_primitives::PatternMatchLiteral::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::StructuralPartialEq for vyre_primitives::PatternMatchLiteral
impl core::marker::Freeze for vyre_primitives::PatternMatchLiteral
impl core::marker::Send for vyre_primitives::PatternMatchLiteral
impl core::marker::Sync for vyre_primitives::PatternMatchLiteral
impl core::marker::Unpin for vyre_primitives::PatternMatchLiteral
impl core::marker::UnsafeUnpin for vyre_primitives::PatternMatchLiteral
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::PatternMatchLiteral
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::PatternMatchLiteral
impl<T, U> core::convert::Into<U> for vyre_primitives::PatternMatchLiteral where U: core::convert::From<T>
pub fn vyre_primitives::PatternMatchLiteral::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::PatternMatchLiteral where U: core::convert::Into<T>
pub type vyre_primitives::PatternMatchLiteral::Error = core::convert::Infallible
pub fn vyre_primitives::PatternMatchLiteral::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::PatternMatchLiteral where U: core::convert::TryFrom<T>
pub type vyre_primitives::PatternMatchLiteral::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::PatternMatchLiteral::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::PatternMatchLiteral where T: core::clone::Clone
pub type vyre_primitives::PatternMatchLiteral::Owned = T
pub fn vyre_primitives::PatternMatchLiteral::clone_into(&self, target: &mut T)
pub fn vyre_primitives::PatternMatchLiteral::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::PatternMatchLiteral where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::PatternMatchLiteral::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::PatternMatchLiteral where T: ?core::marker::Sized
pub fn vyre_primitives::PatternMatchLiteral::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::PatternMatchLiteral where T: ?core::marker::Sized
pub fn vyre_primitives::PatternMatchLiteral::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::PatternMatchLiteral where T: core::clone::Clone
pub unsafe fn vyre_primitives::PatternMatchLiteral::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::PatternMatchLiteral
pub fn vyre_primitives::PatternMatchLiteral::from(t: T) -> T
pub struct vyre_primitives::Popcount
impl core::clone::Clone for vyre_primitives::Popcount
pub fn vyre_primitives::Popcount::clone(&self) -> vyre_primitives::Popcount
impl core::cmp::Eq for vyre_primitives::Popcount
impl core::cmp::PartialEq for vyre_primitives::Popcount
pub fn vyre_primitives::Popcount::eq(&self, other: &vyre_primitives::Popcount) -> bool
impl core::default::Default for vyre_primitives::Popcount
pub fn vyre_primitives::Popcount::default() -> vyre_primitives::Popcount
impl core::fmt::Debug for vyre_primitives::Popcount
pub fn vyre_primitives::Popcount::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::Popcount
pub fn vyre_primitives::Popcount::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::Popcount
impl core::marker::StructuralPartialEq for vyre_primitives::Popcount
impl core::marker::Freeze for vyre_primitives::Popcount
impl core::marker::Send for vyre_primitives::Popcount
impl core::marker::Sync for vyre_primitives::Popcount
impl core::marker::Unpin for vyre_primitives::Popcount
impl core::marker::UnsafeUnpin for vyre_primitives::Popcount
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::Popcount
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::Popcount
impl<T, U> core::convert::Into<U> for vyre_primitives::Popcount where U: core::convert::From<T>
pub fn vyre_primitives::Popcount::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::Popcount where U: core::convert::Into<T>
pub type vyre_primitives::Popcount::Error = core::convert::Infallible
pub fn vyre_primitives::Popcount::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::Popcount where U: core::convert::TryFrom<T>
pub type vyre_primitives::Popcount::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::Popcount::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::Popcount where T: core::clone::Clone
pub type vyre_primitives::Popcount::Owned = T
pub fn vyre_primitives::Popcount::clone_into(&self, target: &mut T)
pub fn vyre_primitives::Popcount::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::Popcount where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::Popcount::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::Popcount where T: ?core::marker::Sized
pub fn vyre_primitives::Popcount::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::Popcount where T: ?core::marker::Sized
pub fn vyre_primitives::Popcount::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::Popcount where T: core::clone::Clone
pub unsafe fn vyre_primitives::Popcount::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::Popcount
pub fn vyre_primitives::Popcount::from(t: T) -> T
pub struct vyre_primitives::Reduce
pub vyre_primitives::Reduce::combine: vyre_primitives::CombineOp
impl core::clone::Clone for vyre_primitives::Reduce
pub fn vyre_primitives::Reduce::clone(&self) -> vyre_primitives::Reduce
impl core::cmp::Eq for vyre_primitives::Reduce
impl core::cmp::PartialEq for vyre_primitives::Reduce
pub fn vyre_primitives::Reduce::eq(&self, other: &vyre_primitives::Reduce) -> bool
impl core::default::Default for vyre_primitives::Reduce
pub fn vyre_primitives::Reduce::default() -> vyre_primitives::Reduce
impl core::fmt::Debug for vyre_primitives::Reduce
pub fn vyre_primitives::Reduce::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::Reduce
pub fn vyre_primitives::Reduce::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::Reduce
impl core::marker::StructuralPartialEq for vyre_primitives::Reduce
impl core::marker::Freeze for vyre_primitives::Reduce
impl core::marker::Send for vyre_primitives::Reduce
impl core::marker::Sync for vyre_primitives::Reduce
impl core::marker::Unpin for vyre_primitives::Reduce
impl core::marker::UnsafeUnpin for vyre_primitives::Reduce
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::Reduce
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::Reduce
impl<T, U> core::convert::Into<U> for vyre_primitives::Reduce where U: core::convert::From<T>
pub fn vyre_primitives::Reduce::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::Reduce where U: core::convert::Into<T>
pub type vyre_primitives::Reduce::Error = core::convert::Infallible
pub fn vyre_primitives::Reduce::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::Reduce where U: core::convert::TryFrom<T>
pub type vyre_primitives::Reduce::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::Reduce::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::Reduce where T: core::clone::Clone
pub type vyre_primitives::Reduce::Owned = T
pub fn vyre_primitives::Reduce::clone_into(&self, target: &mut T)
pub fn vyre_primitives::Reduce::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::Reduce where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::Reduce::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::Reduce where T: ?core::marker::Sized
pub fn vyre_primitives::Reduce::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::Reduce where T: ?core::marker::Sized
pub fn vyre_primitives::Reduce::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::Reduce where T: core::clone::Clone
pub unsafe fn vyre_primitives::Reduce::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::Reduce
pub fn vyre_primitives::Reduce::from(t: T) -> T
pub struct vyre_primitives::RegionId(pub u32)
impl core::clone::Clone for vyre_primitives::RegionId
pub fn vyre_primitives::RegionId::clone(&self) -> vyre_primitives::RegionId
impl core::cmp::Eq for vyre_primitives::RegionId
impl core::cmp::Ord for vyre_primitives::RegionId
pub fn vyre_primitives::RegionId::cmp(&self, other: &vyre_primitives::RegionId) -> core::cmp::Ordering
impl core::cmp::PartialEq for vyre_primitives::RegionId
pub fn vyre_primitives::RegionId::eq(&self, other: &vyre_primitives::RegionId) -> bool
impl core::cmp::PartialOrd for vyre_primitives::RegionId
pub fn vyre_primitives::RegionId::partial_cmp(&self, other: &vyre_primitives::RegionId) -> core::option::Option<core::cmp::Ordering>
impl core::fmt::Debug for vyre_primitives::RegionId
pub fn vyre_primitives::RegionId::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::RegionId
pub fn vyre_primitives::RegionId::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::RegionId
impl core::marker::StructuralPartialEq for vyre_primitives::RegionId
impl core::marker::Freeze for vyre_primitives::RegionId
impl core::marker::Send for vyre_primitives::RegionId
impl core::marker::Sync for vyre_primitives::RegionId
impl core::marker::Unpin for vyre_primitives::RegionId
impl core::marker::UnsafeUnpin for vyre_primitives::RegionId
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::RegionId
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::RegionId
impl<T, U> core::convert::Into<U> for vyre_primitives::RegionId where U: core::convert::From<T>
pub fn vyre_primitives::RegionId::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::RegionId where U: core::convert::Into<T>
pub type vyre_primitives::RegionId::Error = core::convert::Infallible
pub fn vyre_primitives::RegionId::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::RegionId where U: core::convert::TryFrom<T>
pub type vyre_primitives::RegionId::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::RegionId::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::RegionId where T: core::clone::Clone
pub type vyre_primitives::RegionId::Owned = T
pub fn vyre_primitives::RegionId::clone_into(&self, target: &mut T)
pub fn vyre_primitives::RegionId::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::RegionId where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::RegionId::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::RegionId where T: ?core::marker::Sized
pub fn vyre_primitives::RegionId::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::RegionId where T: ?core::marker::Sized
pub fn vyre_primitives::RegionId::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::RegionId where T: core::clone::Clone
pub unsafe fn vyre_primitives::RegionId::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::RegionId
pub fn vyre_primitives::RegionId::from(t: T) -> T
pub struct vyre_primitives::Scan
pub vyre_primitives::Scan::combine: vyre_primitives::CombineOp
impl core::clone::Clone for vyre_primitives::Scan
pub fn vyre_primitives::Scan::clone(&self) -> vyre_primitives::Scan
impl core::cmp::Eq for vyre_primitives::Scan
impl core::cmp::PartialEq for vyre_primitives::Scan
pub fn vyre_primitives::Scan::eq(&self, other: &vyre_primitives::Scan) -> bool
impl core::default::Default for vyre_primitives::Scan
pub fn vyre_primitives::Scan::default() -> vyre_primitives::Scan
impl core::fmt::Debug for vyre_primitives::Scan
pub fn vyre_primitives::Scan::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::Scan
pub fn vyre_primitives::Scan::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::Scan
impl core::marker::StructuralPartialEq for vyre_primitives::Scan
impl core::marker::Freeze for vyre_primitives::Scan
impl core::marker::Send for vyre_primitives::Scan
impl core::marker::Sync for vyre_primitives::Scan
impl core::marker::Unpin for vyre_primitives::Scan
impl core::marker::UnsafeUnpin for vyre_primitives::Scan
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::Scan
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::Scan
impl<T, U> core::convert::Into<U> for vyre_primitives::Scan where U: core::convert::From<T>
pub fn vyre_primitives::Scan::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::Scan where U: core::convert::Into<T>
pub type vyre_primitives::Scan::Error = core::convert::Infallible
pub fn vyre_primitives::Scan::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::Scan where U: core::convert::TryFrom<T>
pub type vyre_primitives::Scan::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::Scan::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::Scan where T: core::clone::Clone
pub type vyre_primitives::Scan::Owned = T
pub fn vyre_primitives::Scan::clone_into(&self, target: &mut T)
pub fn vyre_primitives::Scan::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::Scan where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::Scan::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::Scan where T: ?core::marker::Sized
pub fn vyre_primitives::Scan::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::Scan where T: ?core::marker::Sized
pub fn vyre_primitives::Scan::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::Scan where T: core::clone::Clone
pub unsafe fn vyre_primitives::Scan::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::Scan
pub fn vyre_primitives::Scan::from(t: T) -> T
pub struct vyre_primitives::Scatter
impl core::clone::Clone for vyre_primitives::Scatter
pub fn vyre_primitives::Scatter::clone(&self) -> vyre_primitives::Scatter
impl core::cmp::Eq for vyre_primitives::Scatter
impl core::cmp::PartialEq for vyre_primitives::Scatter
pub fn vyre_primitives::Scatter::eq(&self, other: &vyre_primitives::Scatter) -> bool
impl core::default::Default for vyre_primitives::Scatter
pub fn vyre_primitives::Scatter::default() -> vyre_primitives::Scatter
impl core::fmt::Debug for vyre_primitives::Scatter
pub fn vyre_primitives::Scatter::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::Scatter
pub fn vyre_primitives::Scatter::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::Scatter
impl core::marker::StructuralPartialEq for vyre_primitives::Scatter
impl core::marker::Freeze for vyre_primitives::Scatter
impl core::marker::Send for vyre_primitives::Scatter
impl core::marker::Sync for vyre_primitives::Scatter
impl core::marker::Unpin for vyre_primitives::Scatter
impl core::marker::UnsafeUnpin for vyre_primitives::Scatter
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::Scatter
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::Scatter
impl<T, U> core::convert::Into<U> for vyre_primitives::Scatter where U: core::convert::From<T>
pub fn vyre_primitives::Scatter::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::Scatter where U: core::convert::Into<T>
pub type vyre_primitives::Scatter::Error = core::convert::Infallible
pub fn vyre_primitives::Scatter::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::Scatter where U: core::convert::TryFrom<T>
pub type vyre_primitives::Scatter::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::Scatter::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::Scatter where T: core::clone::Clone
pub type vyre_primitives::Scatter::Owned = T
pub fn vyre_primitives::Scatter::clone_into(&self, target: &mut T)
pub fn vyre_primitives::Scatter::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::Scatter where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::Scatter::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::Scatter where T: ?core::marker::Sized
pub fn vyre_primitives::Scatter::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::Scatter where T: ?core::marker::Sized
pub fn vyre_primitives::Scatter::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::Scatter where T: core::clone::Clone
pub unsafe fn vyre_primitives::Scatter::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::Scatter
pub fn vyre_primitives::Scatter::from(t: T) -> T
pub struct vyre_primitives::ShiftLeft
impl core::clone::Clone for vyre_primitives::ShiftLeft
pub fn vyre_primitives::ShiftLeft::clone(&self) -> vyre_primitives::ShiftLeft
impl core::cmp::Eq for vyre_primitives::ShiftLeft
impl core::cmp::PartialEq for vyre_primitives::ShiftLeft
pub fn vyre_primitives::ShiftLeft::eq(&self, other: &vyre_primitives::ShiftLeft) -> bool
impl core::default::Default for vyre_primitives::ShiftLeft
pub fn vyre_primitives::ShiftLeft::default() -> vyre_primitives::ShiftLeft
impl core::fmt::Debug for vyre_primitives::ShiftLeft
pub fn vyre_primitives::ShiftLeft::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::ShiftLeft
pub fn vyre_primitives::ShiftLeft::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::ShiftLeft
impl core::marker::StructuralPartialEq for vyre_primitives::ShiftLeft
impl core::marker::Freeze for vyre_primitives::ShiftLeft
impl core::marker::Send for vyre_primitives::ShiftLeft
impl core::marker::Sync for vyre_primitives::ShiftLeft
impl core::marker::Unpin for vyre_primitives::ShiftLeft
impl core::marker::UnsafeUnpin for vyre_primitives::ShiftLeft
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::ShiftLeft
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::ShiftLeft
impl<T, U> core::convert::Into<U> for vyre_primitives::ShiftLeft where U: core::convert::From<T>
pub fn vyre_primitives::ShiftLeft::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::ShiftLeft where U: core::convert::Into<T>
pub type vyre_primitives::ShiftLeft::Error = core::convert::Infallible
pub fn vyre_primitives::ShiftLeft::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::ShiftLeft where U: core::convert::TryFrom<T>
pub type vyre_primitives::ShiftLeft::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::ShiftLeft::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::ShiftLeft where T: core::clone::Clone
pub type vyre_primitives::ShiftLeft::Owned = T
pub fn vyre_primitives::ShiftLeft::clone_into(&self, target: &mut T)
pub fn vyre_primitives::ShiftLeft::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::ShiftLeft where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::ShiftLeft::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::ShiftLeft where T: ?core::marker::Sized
pub fn vyre_primitives::ShiftLeft::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::ShiftLeft where T: ?core::marker::Sized
pub fn vyre_primitives::ShiftLeft::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::ShiftLeft where T: core::clone::Clone
pub unsafe fn vyre_primitives::ShiftLeft::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::ShiftLeft
pub fn vyre_primitives::ShiftLeft::from(t: T) -> T
pub struct vyre_primitives::ShiftRight
impl core::clone::Clone for vyre_primitives::ShiftRight
pub fn vyre_primitives::ShiftRight::clone(&self) -> vyre_primitives::ShiftRight
impl core::cmp::Eq for vyre_primitives::ShiftRight
impl core::cmp::PartialEq for vyre_primitives::ShiftRight
pub fn vyre_primitives::ShiftRight::eq(&self, other: &vyre_primitives::ShiftRight) -> bool
impl core::default::Default for vyre_primitives::ShiftRight
pub fn vyre_primitives::ShiftRight::default() -> vyre_primitives::ShiftRight
impl core::fmt::Debug for vyre_primitives::ShiftRight
pub fn vyre_primitives::ShiftRight::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::ShiftRight
pub fn vyre_primitives::ShiftRight::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::ShiftRight
impl core::marker::StructuralPartialEq for vyre_primitives::ShiftRight
impl core::marker::Freeze for vyre_primitives::ShiftRight
impl core::marker::Send for vyre_primitives::ShiftRight
impl core::marker::Sync for vyre_primitives::ShiftRight
impl core::marker::Unpin for vyre_primitives::ShiftRight
impl core::marker::UnsafeUnpin for vyre_primitives::ShiftRight
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::ShiftRight
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::ShiftRight
impl<T, U> core::convert::Into<U> for vyre_primitives::ShiftRight where U: core::convert::From<T>
pub fn vyre_primitives::ShiftRight::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::ShiftRight where U: core::convert::Into<T>
pub type vyre_primitives::ShiftRight::Error = core::convert::Infallible
pub fn vyre_primitives::ShiftRight::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::ShiftRight where U: core::convert::TryFrom<T>
pub type vyre_primitives::ShiftRight::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::ShiftRight::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::ShiftRight where T: core::clone::Clone
pub type vyre_primitives::ShiftRight::Owned = T
pub fn vyre_primitives::ShiftRight::clone_into(&self, target: &mut T)
pub fn vyre_primitives::ShiftRight::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::ShiftRight where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::ShiftRight::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::ShiftRight where T: ?core::marker::Sized
pub fn vyre_primitives::ShiftRight::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::ShiftRight where T: ?core::marker::Sized
pub fn vyre_primitives::ShiftRight::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::ShiftRight where T: core::clone::Clone
pub unsafe fn vyre_primitives::ShiftRight::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::ShiftRight
pub fn vyre_primitives::ShiftRight::from(t: T) -> T
pub struct vyre_primitives::Shuffle
impl core::clone::Clone for vyre_primitives::Shuffle
pub fn vyre_primitives::Shuffle::clone(&self) -> vyre_primitives::Shuffle
impl core::cmp::Eq for vyre_primitives::Shuffle
impl core::cmp::PartialEq for vyre_primitives::Shuffle
pub fn vyre_primitives::Shuffle::eq(&self, other: &vyre_primitives::Shuffle) -> bool
impl core::default::Default for vyre_primitives::Shuffle
pub fn vyre_primitives::Shuffle::default() -> vyre_primitives::Shuffle
impl core::fmt::Debug for vyre_primitives::Shuffle
pub fn vyre_primitives::Shuffle::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::hash::Hash for vyre_primitives::Shuffle
pub fn vyre_primitives::Shuffle::hash<__H: core::hash::Hasher>(&self, state: &mut __H)
impl core::marker::Copy for vyre_primitives::Shuffle
impl core::marker::StructuralPartialEq for vyre_primitives::Shuffle
impl core::marker::Freeze for vyre_primitives::Shuffle
impl core::marker::Send for vyre_primitives::Shuffle
impl core::marker::Sync for vyre_primitives::Shuffle
impl core::marker::Unpin for vyre_primitives::Shuffle
impl core::marker::UnsafeUnpin for vyre_primitives::Shuffle
impl core::panic::unwind_safe::RefUnwindSafe for vyre_primitives::Shuffle
impl core::panic::unwind_safe::UnwindSafe for vyre_primitives::Shuffle
impl<T, U> core::convert::Into<U> for vyre_primitives::Shuffle where U: core::convert::From<T>
pub fn vyre_primitives::Shuffle::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_primitives::Shuffle where U: core::convert::Into<T>
pub type vyre_primitives::Shuffle::Error = core::convert::Infallible
pub fn vyre_primitives::Shuffle::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_primitives::Shuffle where U: core::convert::TryFrom<T>
pub type vyre_primitives::Shuffle::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_primitives::Shuffle::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_primitives::Shuffle where T: core::clone::Clone
pub type vyre_primitives::Shuffle::Owned = T
pub fn vyre_primitives::Shuffle::clone_into(&self, target: &mut T)
pub fn vyre_primitives::Shuffle::to_owned(&self) -> T
impl<T> core::any::Any for vyre_primitives::Shuffle where T: 'static + ?core::marker::Sized
pub fn vyre_primitives::Shuffle::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_primitives::Shuffle where T: ?core::marker::Sized
pub fn vyre_primitives::Shuffle::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_primitives::Shuffle where T: ?core::marker::Sized
pub fn vyre_primitives::Shuffle::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_primitives::Shuffle where T: core::clone::Clone
pub unsafe fn vyre_primitives::Shuffle::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_primitives::Shuffle
pub fn vyre_primitives::Shuffle::from(t: T) -> T

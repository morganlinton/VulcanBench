pub mod vyre_driver_wgpu
pub mod vyre_driver_wgpu::buffer
pub struct vyre_driver_wgpu::buffer::BindGroupCache
impl vyre_driver_wgpu::buffer::BindGroupCache
pub fn vyre_driver_wgpu::buffer::BindGroupCache::get_or_create(&self, layout_id: usize, handles: &[vyre_driver_wgpu::buffer::GpuBufferHandle], factory: impl core::ops::function::FnOnce() -> wgpu::api::bind_group::BindGroup) -> alloc::sync::Arc<wgpu::api::bind_group::BindGroup>
pub fn vyre_driver_wgpu::buffer::BindGroupCache::new() -> Self
pub fn vyre_driver_wgpu::buffer::BindGroupCache::stats(&self) -> vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCache::with_cap(cap: usize) -> Self
impl core::clone::Clone for vyre_driver_wgpu::buffer::BindGroupCache
pub fn vyre_driver_wgpu::buffer::BindGroupCache::clone(&self) -> vyre_driver_wgpu::buffer::BindGroupCache
impl core::default::Default for vyre_driver_wgpu::buffer::BindGroupCache
pub fn vyre_driver_wgpu::buffer::BindGroupCache::default() -> Self
impl core::fmt::Debug for vyre_driver_wgpu::buffer::BindGroupCache
pub fn vyre_driver_wgpu::buffer::BindGroupCache::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::buffer::BindGroupCache
impl core::marker::Send for vyre_driver_wgpu::buffer::BindGroupCache
impl core::marker::Sync for vyre_driver_wgpu::buffer::BindGroupCache
impl core::marker::Unpin for vyre_driver_wgpu::buffer::BindGroupCache
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::buffer::BindGroupCache
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::buffer::BindGroupCache
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::buffer::BindGroupCache
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::buffer::BindGroupCache where U: core::convert::From<T>
pub fn vyre_driver_wgpu::buffer::BindGroupCache::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::buffer::BindGroupCache where U: core::convert::Into<T>
pub type vyre_driver_wgpu::buffer::BindGroupCache::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::buffer::BindGroupCache::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::buffer::BindGroupCache where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::buffer::BindGroupCache::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::buffer::BindGroupCache::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::buffer::BindGroupCache where T: core::clone::Clone
pub type vyre_driver_wgpu::buffer::BindGroupCache::Owned = T
pub fn vyre_driver_wgpu::buffer::BindGroupCache::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::buffer::BindGroupCache::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::buffer::BindGroupCache where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCache::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::buffer::BindGroupCache where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCache::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::buffer::BindGroupCache where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCache::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::buffer::BindGroupCache where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::buffer::BindGroupCache::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::buffer::BindGroupCache
pub fn vyre_driver_wgpu::buffer::BindGroupCache::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::buffer::BindGroupCache
pub fn vyre_driver_wgpu::buffer::BindGroupCache::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::buffer::BindGroupCache
pub fn vyre_driver_wgpu::buffer::BindGroupCache::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::buffer::BindGroupCache
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::buffer::BindGroupCache
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::buffer::BindGroupCache
pub type vyre_driver_wgpu::buffer::BindGroupCache::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::buffer::BindGroupCache where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::buffer::BindGroupCache where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::buffer::BindGroupCache where T: core::marker::Sync
pub struct vyre_driver_wgpu::buffer::BindGroupCacheStats
pub vyre_driver_wgpu::buffer::BindGroupCacheStats::entries: usize
pub vyre_driver_wgpu::buffer::BindGroupCacheStats::evictions: usize
pub vyre_driver_wgpu::buffer::BindGroupCacheStats::hits: usize
pub vyre_driver_wgpu::buffer::BindGroupCacheStats::misses: usize
impl core::clone::Clone for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::clone(&self) -> vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::cmp::Eq for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::cmp::PartialEq for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::eq(&self, other: &vyre_driver_wgpu::buffer::BindGroupCacheStats) -> bool
impl core::default::Default for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::default() -> vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::fmt::Debug for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::Freeze for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::Send for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::Sync for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::Unpin for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::buffer::BindGroupCacheStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::buffer::BindGroupCacheStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::buffer::BindGroupCacheStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::buffer::BindGroupCacheStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::buffer::BindGroupCacheStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::buffer::BindGroupCacheStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::buffer::BindGroupCacheStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::buffer::BindGroupCacheStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: core::clone::Clone
pub type vyre_driver_wgpu::buffer::BindGroupCacheStats::Owned = T
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::buffer::BindGroupCacheStats::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub type vyre_driver_wgpu::buffer::BindGroupCacheStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: core::marker::Sync
pub struct vyre_driver_wgpu::buffer::BufferPool
impl vyre_driver_wgpu::buffer::BufferPool
pub fn vyre_driver_wgpu::buffer::BufferPool::acquire(&self, len: u64, usage: wgpu_types::BufferUsages) -> core::result::Result<vyre_driver_wgpu::buffer::GpuBufferHandle, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::buffer::BufferPool::device(&self) -> &wgpu::api::device::Device
pub fn vyre_driver_wgpu::buffer::BufferPool::new(device: wgpu::api::device::Device, queue: wgpu::api::queue::Queue, config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> Self
pub fn vyre_driver_wgpu::buffer::BufferPool::queue(&self) -> &wgpu::api::queue::Queue
pub fn vyre_driver_wgpu::buffer::BufferPool::release(&self, handle: vyre_driver_wgpu::buffer::GpuBufferHandle)
pub fn vyre_driver_wgpu::buffer::BufferPool::stats(&self) -> vyre_driver_wgpu::buffer::BufferPoolStats
pub fn vyre_driver_wgpu::buffer::BufferPool::with_tiering(device: wgpu::api::device::Device, queue: wgpu::api::queue::Queue, config: &vyre_driver::backend::dispatch_config::DispatchConfig, tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::clone::Clone for vyre_driver_wgpu::buffer::BufferPool
pub fn vyre_driver_wgpu::buffer::BufferPool::clone(&self) -> vyre_driver_wgpu::buffer::BufferPool
impl core::fmt::Debug for vyre_driver_wgpu::buffer::BufferPool
pub fn vyre_driver_wgpu::buffer::BufferPool::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::buffer::BufferPool
impl core::marker::Send for vyre_driver_wgpu::buffer::BufferPool
impl core::marker::Sync for vyre_driver_wgpu::buffer::BufferPool
impl core::marker::Unpin for vyre_driver_wgpu::buffer::BufferPool
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::buffer::BufferPool
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::buffer::BufferPool
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::buffer::BufferPool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::buffer::BufferPool where U: core::convert::From<T>
pub fn vyre_driver_wgpu::buffer::BufferPool::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::buffer::BufferPool where U: core::convert::Into<T>
pub type vyre_driver_wgpu::buffer::BufferPool::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::buffer::BufferPool::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::buffer::BufferPool where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::buffer::BufferPool::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::buffer::BufferPool::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::buffer::BufferPool where T: core::clone::Clone
pub type vyre_driver_wgpu::buffer::BufferPool::Owned = T
pub fn vyre_driver_wgpu::buffer::BufferPool::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::buffer::BufferPool::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::buffer::BufferPool where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BufferPool::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::buffer::BufferPool where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BufferPool::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::buffer::BufferPool where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BufferPool::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::buffer::BufferPool where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::buffer::BufferPool::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::buffer::BufferPool
pub fn vyre_driver_wgpu::buffer::BufferPool::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::buffer::BufferPool
pub fn vyre_driver_wgpu::buffer::BufferPool::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::buffer::BufferPool
pub fn vyre_driver_wgpu::buffer::BufferPool::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::buffer::BufferPool
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::buffer::BufferPool
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::buffer::BufferPool
pub type vyre_driver_wgpu::buffer::BufferPool::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::buffer::BufferPool where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::buffer::BufferPool where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::buffer::BufferPool where T: core::marker::Sync
pub struct vyre_driver_wgpu::buffer::BufferPoolStats
pub vyre_driver_wgpu::buffer::BufferPoolStats::allocations: usize
pub vyre_driver_wgpu::buffer::BufferPoolStats::evictions: usize
pub vyre_driver_wgpu::buffer::BufferPoolStats::hits: usize
pub vyre_driver_wgpu::buffer::BufferPoolStats::releases: usize
pub vyre_driver_wgpu::buffer::BufferPoolStats::retained_bytes: usize
impl core::clone::Clone for vyre_driver_wgpu::buffer::BufferPoolStats
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::clone(&self) -> vyre_driver_wgpu::buffer::BufferPoolStats
impl core::cmp::Eq for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::cmp::PartialEq for vyre_driver_wgpu::buffer::BufferPoolStats
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::eq(&self, other: &vyre_driver_wgpu::buffer::BufferPoolStats) -> bool
impl core::default::Default for vyre_driver_wgpu::buffer::BufferPoolStats
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::default() -> vyre_driver_wgpu::buffer::BufferPoolStats
impl core::fmt::Debug for vyre_driver_wgpu::buffer::BufferPoolStats
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::marker::Freeze for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::marker::Send for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::marker::Sync for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::marker::Unpin for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::buffer::BufferPoolStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::buffer::BufferPoolStats
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::buffer::BufferPoolStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::buffer::BufferPoolStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::buffer::BufferPoolStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::buffer::BufferPoolStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::buffer::BufferPoolStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::buffer::BufferPoolStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::buffer::BufferPoolStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::buffer::BufferPoolStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::buffer::BufferPoolStats where T: core::clone::Clone
pub type vyre_driver_wgpu::buffer::BufferPoolStats::Owned = T
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::buffer::BufferPoolStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::buffer::BufferPoolStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::buffer::BufferPoolStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::buffer::BufferPoolStats where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::buffer::BufferPoolStats::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::buffer::BufferPoolStats
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::buffer::BufferPoolStats
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::buffer::BufferPoolStats
pub fn vyre_driver_wgpu::buffer::BufferPoolStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::buffer::BufferPoolStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::buffer::BufferPoolStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::buffer::BufferPoolStats
pub type vyre_driver_wgpu::buffer::BufferPoolStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::buffer::BufferPoolStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::buffer::BufferPoolStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::buffer::BufferPoolStats where T: core::marker::Sync
pub struct vyre_driver_wgpu::buffer::GpuBufferHandle
impl vyre_driver_wgpu::buffer::GpuBufferHandle
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::alloc(device: &wgpu::api::device::Device, len: u64, usage: wgpu_types::BufferUsages) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::allocation_len(&self) -> u64
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::buffer(&self) -> &wgpu::api::buffer::Buffer
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::buffer_arc(&self) -> alloc::sync::Arc<wgpu::api::buffer::Buffer>
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::byte_len(&self) -> u64
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::element_count(&self) -> usize
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::from_resident_id(id: u64) -> core::option::Option<Self>
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::id(&self) -> u64
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::readback(&self, device: &wgpu::api::device::Device, queue: &wgpu::api::queue::Queue, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::readback_prefix(&self, device: &wgpu::api::device::Device, queue: &wgpu::api::queue::Queue, len: u64, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::readback_range(&self, device: &wgpu::api::device::Device, queue: &wgpu::api::queue::Queue, byte_offset: u64, len: u64, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::upload(device: &wgpu::api::device::Device, queue: &wgpu::api::queue::Queue, bytes: &[u8], usage: wgpu_types::BufferUsages) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::usage(&self) -> wgpu_types::BufferUsages
impl core::clone::Clone for vyre_driver_wgpu::buffer::GpuBufferHandle
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::clone(&self) -> vyre_driver_wgpu::buffer::GpuBufferHandle
impl core::convert::From<vyre_driver_wgpu::buffer::GpuBufferHandle> for vyre_driver_wgpu::engine::graph::GpuResource
pub fn vyre_driver_wgpu::engine::graph::GpuResource::from(handle: vyre_driver_wgpu::buffer::GpuBufferHandle) -> Self
impl core::fmt::Debug for vyre_driver_wgpu::buffer::GpuBufferHandle
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::buffer::GpuBufferHandle
impl core::marker::Send for vyre_driver_wgpu::buffer::GpuBufferHandle
impl core::marker::Sync for vyre_driver_wgpu::buffer::GpuBufferHandle
impl core::marker::Unpin for vyre_driver_wgpu::buffer::GpuBufferHandle
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::buffer::GpuBufferHandle
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::buffer::GpuBufferHandle
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::buffer::GpuBufferHandle
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::buffer::GpuBufferHandle where U: core::convert::From<T>
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::buffer::GpuBufferHandle where U: core::convert::Into<T>
pub type vyre_driver_wgpu::buffer::GpuBufferHandle::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::buffer::GpuBufferHandle where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::buffer::GpuBufferHandle::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::buffer::GpuBufferHandle where T: core::clone::Clone
pub type vyre_driver_wgpu::buffer::GpuBufferHandle::Owned = T
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::buffer::GpuBufferHandle where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::buffer::GpuBufferHandle where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::buffer::GpuBufferHandle where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::buffer::GpuBufferHandle where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::buffer::GpuBufferHandle::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::buffer::GpuBufferHandle
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::buffer::GpuBufferHandle
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::buffer::GpuBufferHandle
pub fn vyre_driver_wgpu::buffer::GpuBufferHandle::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::buffer::GpuBufferHandle
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::buffer::GpuBufferHandle
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::buffer::GpuBufferHandle
pub type vyre_driver_wgpu::buffer::GpuBufferHandle::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::buffer::GpuBufferHandle where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::buffer::GpuBufferHandle where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::buffer::GpuBufferHandle where T: core::marker::Sync
pub struct vyre_driver_wgpu::buffer::StagingBufferPool
impl vyre_driver_wgpu::buffer::StagingBufferPool
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::acquire(&self, device: &wgpu::api::device::Device, size: u64, usage: wgpu_types::BufferUsages) -> wgpu::api::buffer::Buffer
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::new() -> Self
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::release(&self, buffer: wgpu::api::buffer::Buffer, size: u64, usage: wgpu_types::BufferUsages)
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::stats(&self) -> vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::clone::Clone for vyre_driver_wgpu::buffer::StagingBufferPool
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::clone(&self) -> vyre_driver_wgpu::buffer::StagingBufferPool
impl core::default::Default for vyre_driver_wgpu::buffer::StagingBufferPool
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::default() -> vyre_driver_wgpu::buffer::StagingBufferPool
impl core::fmt::Debug for vyre_driver_wgpu::buffer::StagingBufferPool
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::buffer::StagingBufferPool
impl core::marker::Send for vyre_driver_wgpu::buffer::StagingBufferPool
impl core::marker::Sync for vyre_driver_wgpu::buffer::StagingBufferPool
impl core::marker::Unpin for vyre_driver_wgpu::buffer::StagingBufferPool
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::buffer::StagingBufferPool
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::buffer::StagingBufferPool
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::buffer::StagingBufferPool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::buffer::StagingBufferPool where U: core::convert::From<T>
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::buffer::StagingBufferPool where U: core::convert::Into<T>
pub type vyre_driver_wgpu::buffer::StagingBufferPool::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::buffer::StagingBufferPool where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::buffer::StagingBufferPool::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::buffer::StagingBufferPool where T: core::clone::Clone
pub type vyre_driver_wgpu::buffer::StagingBufferPool::Owned = T
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::buffer::StagingBufferPool where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::buffer::StagingBufferPool where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::buffer::StagingBufferPool where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::buffer::StagingBufferPool where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::buffer::StagingBufferPool::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::buffer::StagingBufferPool
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::buffer::StagingBufferPool
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::buffer::StagingBufferPool
pub fn vyre_driver_wgpu::buffer::StagingBufferPool::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::buffer::StagingBufferPool
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::buffer::StagingBufferPool
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::buffer::StagingBufferPool
pub type vyre_driver_wgpu::buffer::StagingBufferPool::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::buffer::StagingBufferPool where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::buffer::StagingBufferPool where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::buffer::StagingBufferPool where T: core::marker::Sync
pub struct vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub vyre_driver_wgpu::buffer::StagingBufferPoolStats::allocations: usize
pub vyre_driver_wgpu::buffer::StagingBufferPoolStats::hits: usize
impl core::clone::Clone for vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::clone(&self) -> vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::cmp::Eq for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::cmp::PartialEq for vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::eq(&self, other: &vyre_driver_wgpu::buffer::StagingBufferPoolStats) -> bool
impl core::default::Default for vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::default() -> vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::fmt::Debug for vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::marker::Freeze for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::marker::Send for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::marker::Sync for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::marker::Unpin for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::buffer::StagingBufferPoolStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::buffer::StagingBufferPoolStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::buffer::StagingBufferPoolStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::buffer::StagingBufferPoolStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::buffer::StagingBufferPoolStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::buffer::StagingBufferPoolStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::buffer::StagingBufferPoolStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::buffer::StagingBufferPoolStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::buffer::StagingBufferPoolStats where T: core::clone::Clone
pub type vyre_driver_wgpu::buffer::StagingBufferPoolStats::Owned = T
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::buffer::StagingBufferPoolStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::buffer::StagingBufferPoolStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::buffer::StagingBufferPoolStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::buffer::StagingBufferPoolStats where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub fn vyre_driver_wgpu::buffer::StagingBufferPoolStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::buffer::StagingBufferPoolStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::buffer::StagingBufferPoolStats
pub type vyre_driver_wgpu::buffer::StagingBufferPoolStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::buffer::StagingBufferPoolStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::buffer::StagingBufferPoolStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::buffer::StagingBufferPoolStats where T: core::marker::Sync
pub mod vyre_driver_wgpu::emit
pub struct vyre_driver_wgpu::emit::WgpuBindingAssignment
pub vyre_driver_wgpu::emit::WgpuBindingAssignment::access: vyre_spec::buffer_access::BufferAccess
pub vyre_driver_wgpu::emit::WgpuBindingAssignment::binding: u32
pub vyre_driver_wgpu::emit::WgpuBindingAssignment::element: vyre_spec::data_type::DataType
pub vyre_driver_wgpu::emit::WgpuBindingAssignment::group: u32
pub vyre_driver_wgpu::emit::WgpuBindingAssignment::kind: vyre_foundation::ir_inner::model::program::MemoryKind
pub vyre_driver_wgpu::emit::WgpuBindingAssignment::name: alloc::sync::Arc<str>
impl core::clone::Clone for vyre_driver_wgpu::emit::WgpuBindingAssignment
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::clone(&self) -> vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::cmp::Eq for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::cmp::PartialEq for vyre_driver_wgpu::emit::WgpuBindingAssignment
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::eq(&self, other: &vyre_driver_wgpu::emit::WgpuBindingAssignment) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::emit::WgpuBindingAssignment
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::marker::Freeze for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::marker::Send for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::marker::Sync for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::marker::Unpin for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::emit::WgpuBindingAssignment where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::emit::WgpuBindingAssignment where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::emit::WgpuBindingAssignment where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::emit::WgpuBindingAssignment where U: core::convert::From<T>
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::emit::WgpuBindingAssignment where U: core::convert::Into<T>
pub type vyre_driver_wgpu::emit::WgpuBindingAssignment::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::emit::WgpuBindingAssignment where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::emit::WgpuBindingAssignment::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::emit::WgpuBindingAssignment where T: core::clone::Clone
pub type vyre_driver_wgpu::emit::WgpuBindingAssignment::Owned = T
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::emit::WgpuBindingAssignment where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::emit::WgpuBindingAssignment where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::emit::WgpuBindingAssignment where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::emit::WgpuBindingAssignment where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::emit::WgpuBindingAssignment::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::emit::WgpuBindingAssignment
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::emit::WgpuBindingAssignment
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::emit::WgpuBindingAssignment
pub fn vyre_driver_wgpu::emit::WgpuBindingAssignment::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::emit::WgpuBindingAssignment
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::emit::WgpuBindingAssignment
pub type vyre_driver_wgpu::emit::WgpuBindingAssignment::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::emit::WgpuBindingAssignment where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::emit::WgpuBindingAssignment where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::emit::WgpuBindingAssignment where T: core::marker::Sync
pub struct vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub vyre_driver_wgpu::emit::WgpuDispatchGeometry::workgroup_size: [u32; 3]
pub vyre_driver_wgpu::emit::WgpuDispatchGeometry::workgroups: [u32; 3]
impl core::clone::Clone for vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::clone(&self) -> vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::cmp::Eq for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::cmp::PartialEq for vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::eq(&self, other: &vyre_driver_wgpu::emit::WgpuDispatchGeometry) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::marker::Freeze for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::marker::Send for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::marker::Sync for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::marker::Unpin for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::emit::WgpuDispatchGeometry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::emit::WgpuDispatchGeometry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::emit::WgpuDispatchGeometry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::emit::WgpuDispatchGeometry where U: core::convert::From<T>
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::emit::WgpuDispatchGeometry where U: core::convert::Into<T>
pub type vyre_driver_wgpu::emit::WgpuDispatchGeometry::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::emit::WgpuDispatchGeometry where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::emit::WgpuDispatchGeometry::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::emit::WgpuDispatchGeometry where T: core::clone::Clone
pub type vyre_driver_wgpu::emit::WgpuDispatchGeometry::Owned = T
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::emit::WgpuDispatchGeometry where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::emit::WgpuDispatchGeometry where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::emit::WgpuDispatchGeometry where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::emit::WgpuDispatchGeometry where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub fn vyre_driver_wgpu::emit::WgpuDispatchGeometry::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::emit::WgpuDispatchGeometry
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub type vyre_driver_wgpu::emit::WgpuDispatchGeometry::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::emit::WgpuDispatchGeometry where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::emit::WgpuDispatchGeometry where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::emit::WgpuDispatchGeometry where T: core::marker::Sync
pub struct vyre_driver_wgpu::emit::WgpuProgram
pub vyre_driver_wgpu::emit::WgpuProgram::bindings: alloc::vec::Vec<vyre_driver_wgpu::emit::WgpuBindingAssignment>
pub vyre_driver_wgpu::emit::WgpuProgram::dispatch_geometry: vyre_driver_wgpu::emit::WgpuDispatchGeometry
pub vyre_driver_wgpu::emit::WgpuProgram::module: naga::ir::Module
pub vyre_driver_wgpu::emit::WgpuProgram::workgroup_size: [u32; 3]
impl vyre_driver_wgpu::emit::WgpuProgram
pub fn vyre_driver_wgpu::emit::WgpuProgram::from_program(program: &vyre_foundation::ir_inner::model::program::core::Program, config: &vyre_driver::backend::dispatch_config::DispatchConfig, enabled_features: &vyre_driver_wgpu::runtime::device::EnabledFeatures) -> core::result::Result<Self, vyre_foundation::lower::LoweringError>
impl core::clone::Clone for vyre_driver_wgpu::emit::WgpuProgram
pub fn vyre_driver_wgpu::emit::WgpuProgram::clone(&self) -> vyre_driver_wgpu::emit::WgpuProgram
impl core::fmt::Debug for vyre_driver_wgpu::emit::WgpuProgram
pub fn vyre_driver_wgpu::emit::WgpuProgram::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::emit::WgpuProgram
impl core::marker::Send for vyre_driver_wgpu::emit::WgpuProgram
impl core::marker::Sync for vyre_driver_wgpu::emit::WgpuProgram
impl core::marker::Unpin for vyre_driver_wgpu::emit::WgpuProgram
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::emit::WgpuProgram
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::emit::WgpuProgram
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::emit::WgpuProgram
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::emit::WgpuProgram where U: core::convert::From<T>
pub fn vyre_driver_wgpu::emit::WgpuProgram::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::emit::WgpuProgram where U: core::convert::Into<T>
pub type vyre_driver_wgpu::emit::WgpuProgram::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::emit::WgpuProgram::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::emit::WgpuProgram where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::emit::WgpuProgram::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::emit::WgpuProgram::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::emit::WgpuProgram where T: core::clone::Clone
pub type vyre_driver_wgpu::emit::WgpuProgram::Owned = T
pub fn vyre_driver_wgpu::emit::WgpuProgram::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::emit::WgpuProgram::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::emit::WgpuProgram where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuProgram::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::emit::WgpuProgram where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuProgram::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::emit::WgpuProgram where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::emit::WgpuProgram::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::emit::WgpuProgram where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::emit::WgpuProgram::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::emit::WgpuProgram
pub fn vyre_driver_wgpu::emit::WgpuProgram::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::emit::WgpuProgram
pub fn vyre_driver_wgpu::emit::WgpuProgram::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::emit::WgpuProgram
pub fn vyre_driver_wgpu::emit::WgpuProgram::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::emit::WgpuProgram
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::emit::WgpuProgram
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::emit::WgpuProgram
pub type vyre_driver_wgpu::emit::WgpuProgram::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::emit::WgpuProgram where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::emit::WgpuProgram where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::emit::WgpuProgram where T: core::marker::Sync
pub fn vyre_driver_wgpu::emit::lower(program: &vyre_foundation::ir_inner::model::program::core::Program) -> core::result::Result<alloc::string::String, vyre_foundation::lower::LoweringError>
pub fn vyre_driver_wgpu::emit::lower_with_config(program: &vyre_foundation::ir_inner::model::program::core::Program, config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::string::String, vyre_foundation::lower::LoweringError>
pub mod vyre_driver_wgpu::engine
pub mod vyre_driver_wgpu::engine::graph
pub enum vyre_driver_wgpu::engine::graph::GpuResource
pub vyre_driver_wgpu::engine::graph::GpuResource::Borrowed(alloc::vec::Vec<u8>)
pub vyre_driver_wgpu::engine::graph::GpuResource::Resident(vyre_driver_wgpu::buffer::GpuBufferHandle)
impl core::clone::Clone for vyre_driver_wgpu::engine::graph::GpuResource
pub fn vyre_driver_wgpu::engine::graph::GpuResource::clone(&self) -> vyre_driver_wgpu::engine::graph::GpuResource
impl core::convert::From<alloc::vec::Vec<u8>> for vyre_driver_wgpu::engine::graph::GpuResource
pub fn vyre_driver_wgpu::engine::graph::GpuResource::from(bytes: alloc::vec::Vec<u8>) -> Self
impl core::convert::From<vyre_driver_wgpu::buffer::GpuBufferHandle> for vyre_driver_wgpu::engine::graph::GpuResource
pub fn vyre_driver_wgpu::engine::graph::GpuResource::from(handle: vyre_driver_wgpu::buffer::GpuBufferHandle) -> Self
impl core::marker::Freeze for vyre_driver_wgpu::engine::graph::GpuResource
impl core::marker::Send for vyre_driver_wgpu::engine::graph::GpuResource
impl core::marker::Sync for vyre_driver_wgpu::engine::graph::GpuResource
impl core::marker::Unpin for vyre_driver_wgpu::engine::graph::GpuResource
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::graph::GpuResource
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::graph::GpuResource
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::graph::GpuResource
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::graph::GpuResource where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::graph::GpuResource::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::graph::GpuResource where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::graph::GpuResource::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::graph::GpuResource::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::graph::GpuResource where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::graph::GpuResource::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::graph::GpuResource::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::graph::GpuResource where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::graph::GpuResource::Owned = T
pub fn vyre_driver_wgpu::engine::graph::GpuResource::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::graph::GpuResource::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::graph::GpuResource where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::GpuResource::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::graph::GpuResource where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::GpuResource::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::graph::GpuResource where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::GpuResource::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::graph::GpuResource where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::graph::GpuResource::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::graph::GpuResource
pub fn vyre_driver_wgpu::engine::graph::GpuResource::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::graph::GpuResource
pub fn vyre_driver_wgpu::engine::graph::GpuResource::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::graph::GpuResource
pub fn vyre_driver_wgpu::engine::graph::GpuResource::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::graph::GpuResource
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::graph::GpuResource
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::graph::GpuResource
pub type vyre_driver_wgpu::engine::graph::GpuResource::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::graph::GpuResource where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::graph::GpuResource where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::graph::GpuResource where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl vyre_driver_wgpu::engine::graph::GpuDispatchGraph
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::dispatch(&self, config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<alloc::vec::Vec<u8>>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::is_empty(&self) -> bool
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::len(&self) -> usize
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::new() -> Self
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::push(&mut self, pipeline: vyre_driver_wgpu::pipeline::WgpuPipeline, input: impl core::convert::Into<vyre_driver_wgpu::engine::graph::GpuResource>)
impl core::default::Default for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::default() -> vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl core::marker::Freeze for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl core::marker::Send for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl core::marker::Sync for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl core::marker::Unpin for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::graph::GpuDispatchGraph::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::graph::GpuDispatchGraph::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
pub fn vyre_driver_wgpu::engine::graph::GpuDispatchGraph::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::graph::GpuDispatchGraph
pub type vyre_driver_wgpu::engine::graph::GpuDispatchGraph::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::graph::GpuDispatchGraph where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::graph::LaunchAccounting
pub vyre_driver_wgpu::engine::graph::LaunchAccounting::graph_submissions: usize
pub vyre_driver_wgpu::engine::graph::LaunchAccounting::sequential_submissions: usize
impl vyre_driver_wgpu::engine::graph::LaunchAccounting
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::reduction_factor(self) -> usize
impl core::clone::Clone for vyre_driver_wgpu::engine::graph::LaunchAccounting
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::clone(&self) -> vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::cmp::Eq for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::graph::LaunchAccounting
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::eq(&self, other: &vyre_driver_wgpu::engine::graph::LaunchAccounting) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::engine::graph::LaunchAccounting
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::marker::Freeze for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::marker::Send for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::marker::Sync for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::marker::Unpin for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::graph::LaunchAccounting where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::graph::LaunchAccounting where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::graph::LaunchAccounting where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::graph::LaunchAccounting where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::graph::LaunchAccounting where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::graph::LaunchAccounting::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::graph::LaunchAccounting where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::graph::LaunchAccounting::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::graph::LaunchAccounting where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::graph::LaunchAccounting::Owned = T
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::graph::LaunchAccounting where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::graph::LaunchAccounting where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::graph::LaunchAccounting where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::graph::LaunchAccounting where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::graph::LaunchAccounting::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::graph::LaunchAccounting
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::graph::LaunchAccounting
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::graph::LaunchAccounting
pub fn vyre_driver_wgpu::engine::graph::LaunchAccounting::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::graph::LaunchAccounting
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::graph::LaunchAccounting
pub type vyre_driver_wgpu::engine::graph::LaunchAccounting::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::graph::LaunchAccounting where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::graph::LaunchAccounting where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::graph::LaunchAccounting where T: core::marker::Sync
pub fn vyre_driver_wgpu::engine::graph::launch_accounting(op_count: usize) -> vyre_driver_wgpu::engine::graph::LaunchAccounting
pub mod vyre_driver_wgpu::engine::multi_gpu
pub struct vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
pub vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem::config: &'a vyre_driver::backend::dispatch_config::DispatchConfig
pub vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem::cost: u64
pub vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem::id: usize
pub vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem::inputs: &'a [&'a [u8]]
pub vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem::program: &'a vyre_foundation::ir_inner::model::program::core::Program
impl<'a> core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<'a> core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<'a> core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<'a> core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<'a> core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<'a> !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<'a> !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
pub fn vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>
pub type vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a>::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'a> where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
pub vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::device_index: usize
pub vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::queued_cost: u64
impl core::clone::Clone for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::clone(&self) -> vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::cmp::Eq for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::eq(&self, other: &vyre_driver_wgpu::engine::multi_gpu::DeviceLoad) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::Owned = T
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
pub fn vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad
pub type vyre_driver_wgpu::engine::multi_gpu::DeviceLoad::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::DeviceLoad where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
pub vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::config: vyre_driver::backend::dispatch_config::DispatchConfig
pub vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::cost: u64
pub vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::id: usize
pub vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::inputs: alloc::vec::Vec<alloc::vec::Vec<u8>>
pub vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::program: vyre_foundation::ir_inner::model::program::core::Program
impl !core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem
pub type vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
pub vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::adapter_index: usize
pub vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::id: usize
pub vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::outputs: alloc::vec::Vec<alloc::vec::Vec<u8>>
impl core::fmt::Debug for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
pub fn vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput
pub type vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::multi_gpu::LiveGpu
pub vyre_driver_wgpu::engine::multi_gpu::LiveGpu::adapter_index: usize
pub vyre_driver_wgpu::engine::multi_gpu::LiveGpu::info: wgpu_types::AdapterInfo
impl core::clone::Clone for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::clone(&self) -> vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::cmp::Eq for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::eq(&self, other: &vyre_driver_wgpu::engine::multi_gpu::LiveGpu) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::LiveGpu::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::LiveGpu::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::multi_gpu::LiveGpu::Owned = T
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
pub fn vyre_driver_wgpu::engine::multi_gpu::LiveGpu::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::LiveGpu
pub type vyre_driver_wgpu::engine::multi_gpu::LiveGpu::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::LiveGpu where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::acquire_all() -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::acquire_indices(indices: &[usize]) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::adapter_indices(&self) -> alloc::vec::Vec<usize>
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::dispatch_batch(&mut self, items: alloc::vec::Vec<vyre_driver_wgpu::engine::multi_gpu::GpuWorkItem>) -> core::result::Result<alloc::vec::Vec<vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::dispatch_borrowed_batch(&mut self, items: &[vyre_driver_wgpu::engine::multi_gpu::BorrowedGpuWorkItem<'_>]) -> core::result::Result<alloc::vec::Vec<core::result::Result<vyre_driver_wgpu::engine::multi_gpu::GpuWorkOutput, vyre_driver::backend::error::BackendError>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::enumerate_live_gpus() -> alloc::vec::Vec<vyre_driver_wgpu::engine::multi_gpu::LiveGpu>
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::is_empty(&self) -> bool
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::len(&self) -> usize
impl core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
pub fn vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor
pub type vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::MultiGpuExecutor where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::multi_gpu::Partition
pub vyre_driver_wgpu::engine::multi_gpu::Partition::device_index: usize
pub vyre_driver_wgpu::engine::multi_gpu::Partition::item_ids: alloc::vec::Vec<usize>
pub vyre_driver_wgpu::engine::multi_gpu::Partition::total_cost: u64
impl core::clone::Clone for vyre_driver_wgpu::engine::multi_gpu::Partition
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::clone(&self) -> vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::cmp::Eq for vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::multi_gpu::Partition
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::eq(&self, other: &vyre_driver_wgpu::engine::multi_gpu::Partition) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::engine::multi_gpu::Partition
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::Partition
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::Partition
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::Partition where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::Partition where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::Partition where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::Partition where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::Partition where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::Partition::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::Partition where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::Partition::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::multi_gpu::Partition where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::multi_gpu::Partition::Owned = T
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::Partition where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::Partition where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::Partition where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::multi_gpu::Partition where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::multi_gpu::Partition::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::Partition
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::Partition
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::Partition
pub fn vyre_driver_wgpu::engine::multi_gpu::Partition::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::Partition
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::Partition
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::Partition
pub type vyre_driver_wgpu::engine::multi_gpu::Partition::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::Partition where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::Partition where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::Partition where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::assign(&mut self, key: &[u8], cost: u64) -> core::result::Result<core::option::Option<u32>, vyre_driver_wgpu::engine::multi_gpu::stream_shard::StreamShardError>
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::load(&self) -> &[u64]
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::new(n_gpus: u32, spill_threshold: u64) -> core::result::Result<Self, vyre_driver_wgpu::engine::multi_gpu::stream_shard::StreamShardError>
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::seed_load(&mut self, device: u32, cost: u64) -> core::result::Result<(), vyre_driver_wgpu::engine::multi_gpu::stream_shard::StreamShardError>
impl core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
pub fn vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator
pub type vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::StreamShardAllocator where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
pub vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::cost: u64
pub vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::id: usize
impl core::clone::Clone for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::clone(&self) -> vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::cmp::Eq for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::eq(&self, other: &vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::marker::Freeze for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::marker::Send for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::marker::Sync for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::marker::Unpin for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::Owned = T
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
pub fn vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem
pub type vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem where T: core::marker::Sync
pub fn vyre_driver_wgpu::engine::multi_gpu::live_gpu_loads() -> core::result::Result<alloc::vec::Vec<vyre_driver_wgpu::engine::multi_gpu::DeviceLoad>, alloc::string::String>
pub fn vyre_driver_wgpu::engine::multi_gpu::partition_work_stealing(devices: &[vyre_driver_wgpu::engine::multi_gpu::DeviceLoad], items: &[vyre_driver_wgpu::engine::multi_gpu::WeightedWorkItem]) -> core::result::Result<alloc::vec::Vec<vyre_driver_wgpu::engine::multi_gpu::Partition>, alloc::string::String>
pub fn vyre_driver_wgpu::engine::multi_gpu::shard_by_blake3(key: &[u8], n_gpus: u32) -> core::result::Result<u32, vyre_driver_wgpu::engine::multi_gpu::stream_shard::StreamShardError>
pub mod vyre_driver_wgpu::engine::persistent
pub struct vyre_driver_wgpu::engine::persistent::PersistentKernelReport
pub vyre_driver_wgpu::engine::persistent::PersistentKernelReport::kernel_launches: u32
pub vyre_driver_wgpu::engine::persistent::PersistentKernelReport::results: alloc::vec::Vec<vyre_driver_wgpu::engine::persistent::WorkResult>
impl core::clone::Clone for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::clone(&self) -> vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::cmp::Eq for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::eq(&self, other: &vyre_driver_wgpu::engine::persistent::PersistentKernelReport) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::marker::Freeze for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::marker::Send for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::marker::Sync for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::marker::Unpin for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::persistent::PersistentKernelReport::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::persistent::PersistentKernelReport::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::persistent::PersistentKernelReport::Owned = T
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
pub fn vyre_driver_wgpu::engine::persistent::PersistentKernelReport::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::persistent::PersistentKernelReport
pub type vyre_driver_wgpu::engine::persistent::PersistentKernelReport::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::persistent::PersistentKernelReport where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
pub vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::id: u32
pub vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::payload: alloc::vec::Vec<u8>
impl core::clone::Clone for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::clone(&self) -> vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::cmp::Eq for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::eq(&self, other: &vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::marker::Freeze for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::marker::Send for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::marker::Sync for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::marker::Unpin for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::Owned = T
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
pub fn vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem
pub type vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::persistent::PersistentQueue
impl vyre_driver_wgpu::engine::persistent::PersistentQueue
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::is_empty(&self) -> bool
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::len(&self) -> usize
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::new() -> Self
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::push(&mut self, item: vyre_driver_wgpu::engine::persistent::PersistentPayloadWorkItem)
impl core::clone::Clone for vyre_driver_wgpu::engine::persistent::PersistentQueue
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::clone(&self) -> vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::cmp::Eq for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::persistent::PersistentQueue
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::eq(&self, other: &vyre_driver_wgpu::engine::persistent::PersistentQueue) -> bool
impl core::default::Default for vyre_driver_wgpu::engine::persistent::PersistentQueue
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::default() -> vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::fmt::Debug for vyre_driver_wgpu::engine::persistent::PersistentQueue
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::marker::Freeze for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::marker::Send for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::marker::Sync for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::marker::Unpin for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentQueue where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentQueue where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::persistent::PersistentQueue where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::persistent::PersistentQueue where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::persistent::PersistentQueue where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::persistent::PersistentQueue::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::persistent::PersistentQueue where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::persistent::PersistentQueue::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::persistent::PersistentQueue where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::persistent::PersistentQueue::Owned = T
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::persistent::PersistentQueue where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::persistent::PersistentQueue where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::persistent::PersistentQueue where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::persistent::PersistentQueue where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::persistent::PersistentQueue::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::persistent::PersistentQueue
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::persistent::PersistentQueue
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::persistent::PersistentQueue
pub fn vyre_driver_wgpu::engine::persistent::PersistentQueue::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::persistent::PersistentQueue
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::persistent::PersistentQueue
pub type vyre_driver_wgpu::engine::persistent::PersistentQueue::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::persistent::PersistentQueue where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::persistent::PersistentQueue where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::persistent::PersistentQueue where T: core::marker::Sync
pub struct vyre_driver_wgpu::engine::persistent::WorkResult
pub vyre_driver_wgpu::engine::persistent::WorkResult::id: u32
pub vyre_driver_wgpu::engine::persistent::WorkResult::payload: alloc::vec::Vec<u8>
impl core::clone::Clone for vyre_driver_wgpu::engine::persistent::WorkResult
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::clone(&self) -> vyre_driver_wgpu::engine::persistent::WorkResult
impl core::cmp::Eq for vyre_driver_wgpu::engine::persistent::WorkResult
impl core::cmp::PartialEq for vyre_driver_wgpu::engine::persistent::WorkResult
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::eq(&self, other: &vyre_driver_wgpu::engine::persistent::WorkResult) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::engine::persistent::WorkResult
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::engine::persistent::WorkResult
impl core::marker::Freeze for vyre_driver_wgpu::engine::persistent::WorkResult
impl core::marker::Send for vyre_driver_wgpu::engine::persistent::WorkResult
impl core::marker::Sync for vyre_driver_wgpu::engine::persistent::WorkResult
impl core::marker::Unpin for vyre_driver_wgpu::engine::persistent::WorkResult
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::persistent::WorkResult
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::persistent::WorkResult
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::persistent::WorkResult
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::engine::persistent::WorkResult where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::persistent::WorkResult where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::engine::persistent::WorkResult where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::persistent::WorkResult where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::persistent::WorkResult where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::persistent::WorkResult::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::persistent::WorkResult where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::persistent::WorkResult::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::engine::persistent::WorkResult where T: core::clone::Clone
pub type vyre_driver_wgpu::engine::persistent::WorkResult::Owned = T
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::engine::persistent::WorkResult where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::persistent::WorkResult where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::persistent::WorkResult where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::engine::persistent::WorkResult where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::engine::persistent::WorkResult::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::persistent::WorkResult
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::persistent::WorkResult
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::persistent::WorkResult
pub fn vyre_driver_wgpu::engine::persistent::WorkResult::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::persistent::WorkResult
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::persistent::WorkResult
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::persistent::WorkResult
pub type vyre_driver_wgpu::engine::persistent::WorkResult::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::persistent::WorkResult where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::persistent::WorkResult where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::persistent::WorkResult where T: core::marker::Sync
pub fn vyre_driver_wgpu::engine::persistent::run_persistent_kernel(backend: &vyre_driver_wgpu::WgpuBackend, program: &vyre_foundation::ir_inner::model::program::core::Program, config: &vyre_driver::backend::dispatch_config::DispatchConfig, queue: vyre_driver_wgpu::engine::persistent::PersistentQueue) -> core::result::Result<vyre_driver_wgpu::engine::persistent::PersistentKernelReport, vyre_driver::backend::error::BackendError>
pub mod vyre_driver_wgpu::engine::streaming
pub mod vyre_driver_wgpu::engine::streaming::async_copy
pub struct vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::async_load<F>(&mut self, tag: impl core::convert::Into<alloc::string::String>, copy: F) -> core::result::Result<(), vyre_driver::backend::error::BackendError> where F: core::ops::function::FnOnce() -> core::result::Result<(), vyre_driver::backend::error::BackendError> + core::marker::Send + 'static
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::async_wait(&mut self, tag: &str) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::new() -> Self
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::overlap_copy_compute<C, G>(&mut self, tag: impl core::convert::Into<alloc::string::String>, copy: C, compute: G) -> core::result::Result<(), vyre_driver::backend::error::BackendError> where C: core::ops::function::FnOnce() -> core::result::Result<(), vyre_driver::backend::error::BackendError> + core::marker::Send + 'static, G: core::ops::function::FnOnce() -> core::result::Result<(), vyre_driver::backend::error::BackendError>
impl core::default::Default for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::default() -> vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl core::ops::drop::Drop for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::drop(&mut self)
impl core::marker::Freeze for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl core::marker::Send for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl !core::marker::Sync for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl core::marker::Unpin for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
pub fn vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams
pub type vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::streaming::async_copy::AsyncCopyStreams where T: core::marker::Send
pub struct vyre_driver_wgpu::engine::streaming::HostIngressStream
impl vyre_driver_wgpu::engine::streaming::HostIngressStream
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::finish(&mut self) -> core::result::Result<core::option::Option<alloc::vec::Vec<alloc::vec::Vec<u8>>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::from_runner<F>(runner: F, config: vyre_driver::backend::dispatch_config::DispatchConfig) -> Self where F: core::ops::function::Fn(alloc::vec::Vec<u8>, vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, vyre_driver::backend::error::BackendError> + core::marker::Send + core::marker::Sync + 'static
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::new(pipeline: vyre_driver_wgpu::pipeline::WgpuPipeline, config: vyre_driver::backend::dispatch_config::DispatchConfig) -> Self
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::push_chunk(&mut self, bytes: alloc::vec::Vec<u8>) -> core::result::Result<core::option::Option<alloc::vec::Vec<alloc::vec::Vec<u8>>>, vyre_driver::backend::error::BackendError>
impl core::marker::Freeze for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl core::marker::Send for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl core::marker::Sync for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl core::marker::Unpin for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::engine::streaming::HostIngressStream where U: core::convert::From<T>
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::engine::streaming::HostIngressStream where U: core::convert::Into<T>
pub type vyre_driver_wgpu::engine::streaming::HostIngressStream::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::engine::streaming::HostIngressStream where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::engine::streaming::HostIngressStream::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::engine::streaming::HostIngressStream where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::engine::streaming::HostIngressStream where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::engine::streaming::HostIngressStream where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::engine::streaming::HostIngressStream
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::engine::streaming::HostIngressStream
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::engine::streaming::HostIngressStream
pub fn vyre_driver_wgpu::engine::streaming::HostIngressStream::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::engine::streaming::HostIngressStream
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::engine::streaming::HostIngressStream
pub type vyre_driver_wgpu::engine::streaming::HostIngressStream::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::engine::streaming::HostIngressStream where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::engine::streaming::HostIngressStream where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::engine::streaming::HostIngressStream where T: core::marker::Sync
pub mod vyre_driver_wgpu::ext
pub mod vyre_driver_wgpu::megakernel
pub struct vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<'a> vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::dispatch_megakernel(&self, work_items: &[vyre_runtime::megakernel::planner::caps::MegakernelWorkItem], config: &vyre_runtime::megakernel::planner::config::MegakernelConfig) -> core::result::Result<vyre_runtime::megakernel::planner::caps::MegakernelReport, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::dispatch_megakernel_bytes(&self, work_queue_bytes: &[u8], config: &vyre_runtime::megakernel::planner::config::MegakernelConfig) -> core::result::Result<vyre_runtime::megakernel::planner::caps::MegakernelReport, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::dispatch_megakernel_with_io_queue(&self, work_items: &[vyre_runtime::megakernel::planner::caps::MegakernelWorkItem], config: &vyre_runtime::megakernel::planner::config::MegakernelConfig, io_queue_bytes: alloc::vec::Vec<u8>) -> core::result::Result<vyre_runtime::megakernel::planner::caps::MegakernelReport, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::new(backend: &'a dyn vyre_driver::backend::vyre_backend::VyreBackend) -> Self
impl vyre_runtime::megakernel::MegakernelDispatch for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'_>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'_>::dispatch_megakernel(&self, work_queue: &[vyre_runtime::megakernel::planner::caps::MegakernelWorkItem], config: &vyre_runtime::megakernel::planner::config::MegakernelConfig) -> core::result::Result<vyre_runtime::megakernel::planner::caps::MegakernelReport, vyre_driver::backend::error::BackendError>
impl<'a> core::marker::Freeze for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<'a> core::marker::Send for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<'a> core::marker::Sync for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<'a> core::marker::Unpin for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<'a> core::marker::UnsafeUnpin for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<'a> !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<'a> !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where U: core::convert::From<T>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where U: core::convert::Into<T>
pub type vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
pub fn vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>
pub type vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a>::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::megakernel::WgpuMegakernelDispatcher<'a> where T: core::marker::Sync
pub mod vyre_driver_wgpu::pipeline
pub use vyre_driver_wgpu::pipeline::IndirectDispatch
pub use vyre_driver_wgpu::pipeline::OutputLayout
pub use vyre_driver_wgpu::pipeline::output_layout_from_program
pub mod vyre_driver_wgpu::pipeline::persistent
pub struct vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub vyre_driver_wgpu::pipeline::persistent::DispatchItem::inputs: &'a [vyre_driver_wgpu::buffer::GpuBufferHandle]
pub vyre_driver_wgpu::pipeline::persistent::DispatchItem::outputs: &'a [vyre_driver_wgpu::buffer::GpuBufferHandle]
pub vyre_driver_wgpu::pipeline::persistent::DispatchItem::params: core::option::Option<&'a vyre_driver_wgpu::buffer::GpuBufferHandle>
pub vyre_driver_wgpu::pipeline::persistent::DispatchItem::workgroups: [u32; 3]
impl<'a> core::marker::Freeze for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> core::marker::Send for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> core::marker::Sync for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> core::marker::Unpin for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> core::marker::UnsafeUnpin for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where U: core::convert::From<T>
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where U: core::convert::Into<T>
pub type vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub type vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: core::marker::Sync
pub struct vyre_driver_wgpu::pipeline::BindGroupCacheStats
pub vyre_driver_wgpu::pipeline::BindGroupCacheStats::entries: usize
pub vyre_driver_wgpu::pipeline::BindGroupCacheStats::evictions: usize
pub vyre_driver_wgpu::pipeline::BindGroupCacheStats::hits: usize
pub vyre_driver_wgpu::pipeline::BindGroupCacheStats::misses: usize
impl core::clone::Clone for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::clone(&self) -> vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::cmp::Eq for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::cmp::PartialEq for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::eq(&self, other: &vyre_driver_wgpu::buffer::BindGroupCacheStats) -> bool
impl core::default::Default for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::default() -> vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::fmt::Debug for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::Freeze for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::Send for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::Sync for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::Unpin for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::buffer::BindGroupCacheStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::buffer::BindGroupCacheStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::buffer::BindGroupCacheStats where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::buffer::BindGroupCacheStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::buffer::BindGroupCacheStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::buffer::BindGroupCacheStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::buffer::BindGroupCacheStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::buffer::BindGroupCacheStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: core::clone::Clone
pub type vyre_driver_wgpu::buffer::BindGroupCacheStats::Owned = T
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::buffer::BindGroupCacheStats::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::buffer::BindGroupCacheStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::buffer::BindGroupCacheStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::buffer::BindGroupCacheStats
pub type vyre_driver_wgpu::buffer::BindGroupCacheStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::buffer::BindGroupCacheStats where T: core::marker::Sync
pub struct vyre_driver_wgpu::pipeline::DispatchItem<'a>
pub vyre_driver_wgpu::pipeline::DispatchItem::inputs: &'a [vyre_driver_wgpu::buffer::GpuBufferHandle]
pub vyre_driver_wgpu::pipeline::DispatchItem::outputs: &'a [vyre_driver_wgpu::buffer::GpuBufferHandle]
pub vyre_driver_wgpu::pipeline::DispatchItem::params: core::option::Option<&'a vyre_driver_wgpu::buffer::GpuBufferHandle>
pub vyre_driver_wgpu::pipeline::DispatchItem::workgroups: [u32; 3]
impl<'a> core::marker::Freeze for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> core::marker::Send for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> core::marker::Sync for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> core::marker::Unpin for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> core::marker::UnsafeUnpin for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<'a> !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where U: core::convert::From<T>
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where U: core::convert::Into<T>
pub type vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub fn vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>
pub type vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a>::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::pipeline::persistent::DispatchItem<'a> where T: core::marker::Sync
pub struct vyre_driver_wgpu::pipeline::WgpuPipeline
impl vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::bind_group_cache_stats(&self) -> vyre_driver_wgpu::buffer::BindGroupCacheStats
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent(&self, inputs: &[vyre_driver_wgpu::buffer::GpuBufferHandle], outputs: &mut [vyre_driver_wgpu::buffer::GpuBufferHandle], params: core::option::Option<&vyre_driver_wgpu::buffer::GpuBufferHandle>, workgroups: [u32; 3]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent_batched(&self, items: &[vyre_driver_wgpu::pipeline::persistent::DispatchItem<'_>]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent_borrowed(&self, inputs: &[&vyre_driver_wgpu::buffer::GpuBufferHandle], outputs: &[&vyre_driver_wgpu::buffer::GpuBufferHandle], params: core::option::Option<&vyre_driver_wgpu::buffer::GpuBufferHandle>, workgroups: [u32; 3]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
impl vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::compile(program: &vyre_foundation::ir_inner::model::program::core::Program) -> core::result::Result<alloc::sync::Arc<Self>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::compile_with_config(program: &vyre_foundation::ir_inner::model::program::core::Program, config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::sync::Arc<Self>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::execution_plan(&self) -> &vyre_foundation::execution_plan::ExecutionPlan
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::push_chunk(&self, bytes: &[u8], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, vyre_driver::backend::error::BackendError>
impl vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_coalesced(&self, inputs: &[alloc::vec::Vec<u8>], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<alloc::vec::Vec<u8>>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_coalesced_borrowed(&self, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<alloc::vec::Vec<u8>>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_compound_v2(requests: &[(&vyre_driver_wgpu::pipeline::WgpuPipeline, vyre_driver::backend::resource::Resource)], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<alloc::vec::Vec<u8>>>, vyre_driver::backend::error::BackendError>
impl vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::prerecord_borrowed_dispatch(&self, inputs: &[&[u8]], workgroups: [u32; 3]) -> core::result::Result<vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::prerecord_persistent_dispatch(&self, inputs: &[vyre_driver_wgpu::buffer::GpuBufferHandle], outputs: &[vyre_driver_wgpu::buffer::GpuBufferHandle], params: core::option::Option<&vyre_driver_wgpu::buffer::GpuBufferHandle>, workgroups: [u32; 3]) -> core::result::Result<vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch, vyre_driver::backend::error::BackendError>
impl core::clone::Clone for vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::clone(&self) -> vyre_driver_wgpu::pipeline::WgpuPipeline
impl core::fmt::Debug for vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl vyre_driver::backend::compiled_pipeline::CompiledPipeline for vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch(&self, inputs: &[alloc::vec::Vec<u8>], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_borrowed(&self, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_borrowed_batched(&self, batches: &[&[&[u8]]], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<vyre_driver::backend::dispatch_result::OutputBuffers>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_borrowed_batched_into(&self, batches: &[&[&[u8]]], config: &vyre_driver::backend::dispatch_config::DispatchConfig, batch_outputs: &mut alloc::vec::Vec<vyre_driver::backend::dispatch_result::OutputBuffers>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_borrowed_into(&self, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig, outputs: &mut vyre_driver::backend::dispatch_result::OutputBuffers) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_borrowed_timed(&self, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<vyre_driver::backend::dispatch_result::TimedDispatchResult, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent_handles(&self, inputs: &[vyre_driver::backend::resource::Resource], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<vyre_driver::backend::dispatch_result::OutputBuffers, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent_handles_batched(&self, batches: &[&[vyre_driver::backend::resource::Resource]], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<vyre_driver::backend::dispatch_result::OutputBuffers>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent_handles_batched_into(&self, batches: &[&[vyre_driver::backend::resource::Resource]], config: &vyre_driver::backend::dispatch_config::DispatchConfig, batch_outputs: &mut alloc::vec::Vec<vyre_driver::backend::dispatch_result::OutputBuffers>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent_handles_into(&self, inputs: &[vyre_driver::backend::resource::Resource], config: &vyre_driver::backend::dispatch_config::DispatchConfig, outputs: &mut vyre_driver::backend::dispatch_result::OutputBuffers) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent_handles_timed(&self, inputs: &[vyre_driver::backend::resource::Resource], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<vyre_driver::backend::dispatch_result::TimedDispatchResult, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::dispatch_persistent_resource_outputs(&self, inputs: &[vyre_driver::backend::resource::Resource], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<vyre_driver::backend::resource::Resource>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::id(&self) -> &str
impl vyre_driver::backend::private::Sealed for vyre_driver_wgpu::pipeline::WgpuPipeline
impl core::marker::Freeze for vyre_driver_wgpu::pipeline::WgpuPipeline
impl core::marker::Send for vyre_driver_wgpu::pipeline::WgpuPipeline
impl core::marker::Sync for vyre_driver_wgpu::pipeline::WgpuPipeline
impl core::marker::Unpin for vyre_driver_wgpu::pipeline::WgpuPipeline
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::pipeline::WgpuPipeline
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::pipeline::WgpuPipeline
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::pipeline::WgpuPipeline
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::pipeline::WgpuPipeline where U: core::convert::From<T>
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::pipeline::WgpuPipeline where U: core::convert::Into<T>
pub type vyre_driver_wgpu::pipeline::WgpuPipeline::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::pipeline::WgpuPipeline where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::pipeline::WgpuPipeline::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::pipeline::WgpuPipeline where T: core::clone::Clone
pub type vyre_driver_wgpu::pipeline::WgpuPipeline::Owned = T
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::pipeline::WgpuPipeline where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::pipeline::WgpuPipeline where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::pipeline::WgpuPipeline where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::pipeline::WgpuPipeline where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::pipeline::WgpuPipeline::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::pipeline::WgpuPipeline
pub fn vyre_driver_wgpu::pipeline::WgpuPipeline::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::pipeline::WgpuPipeline
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::pipeline::WgpuPipeline
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::pipeline::WgpuPipeline
pub type vyre_driver_wgpu::pipeline::WgpuPipeline::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::pipeline::WgpuPipeline where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::pipeline::WgpuPipeline where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::pipeline::WgpuPipeline where T: core::marker::Sync
pub mod vyre_driver_wgpu::runtime
pub mod vyre_driver_wgpu::runtime::adapter_caps_probe
pub fn vyre_driver_wgpu::runtime::adapter_caps_probe::from_backend(adapter_info: &wgpu_types::AdapterInfo, device_limits: &wgpu_types::Limits, enabled: &vyre_driver_wgpu::runtime::device::EnabledFeatures) -> vyre_foundation::optimizer::ctx::AdapterCaps
pub fn vyre_driver_wgpu::runtime::adapter_caps_probe::from_backend_profile(adapter_info: &wgpu_types::AdapterInfo, device_limits: &wgpu_types::Limits, enabled: &vyre_driver_wgpu::runtime::device::EnabledFeatures) -> vyre_driver::device_profile::DeviceProfile
pub fn vyre_driver_wgpu::runtime::adapter_caps_probe::probe(adapter: &wgpu::api::adapter::Adapter) -> vyre_foundation::optimizer::ctx::AdapterCaps
pub fn vyre_driver_wgpu::runtime::adapter_caps_probe::probe_profile(adapter: &wgpu::api::adapter::Adapter) -> vyre_driver::device_profile::DeviceProfile
pub mod vyre_driver_wgpu::runtime::aot
pub struct vyre_driver_wgpu::runtime::aot::AotArtifact
pub vyre_driver_wgpu::runtime::aot::AotArtifact::cache_hit: bool
pub vyre_driver_wgpu::runtime::aot::AotArtifact::key: alloc::string::String
pub vyre_driver_wgpu::runtime::aot::AotArtifact::wgsl: alloc::string::String
impl core::clone::Clone for vyre_driver_wgpu::runtime::aot::AotArtifact
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::clone(&self) -> vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::cmp::Eq for vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::aot::AotArtifact
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::eq(&self, other: &vyre_driver_wgpu::runtime::aot::AotArtifact) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::runtime::aot::AotArtifact
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::marker::Freeze for vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::marker::Send for vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::marker::Sync for vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::marker::Unpin for vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::aot::AotArtifact
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::aot::AotArtifact
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::aot::AotArtifact where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::aot::AotArtifact where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::aot::AotArtifact where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::aot::AotArtifact where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::aot::AotArtifact where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::aot::AotArtifact::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::aot::AotArtifact where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::aot::AotArtifact::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::aot::AotArtifact where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::aot::AotArtifact::Owned = T
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::aot::AotArtifact where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::aot::AotArtifact where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::aot::AotArtifact where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::aot::AotArtifact where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::aot::AotArtifact::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::aot::AotArtifact
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::aot::AotArtifact
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::aot::AotArtifact
pub fn vyre_driver_wgpu::runtime::aot::AotArtifact::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::aot::AotArtifact
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::aot::AotArtifact
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::aot::AotArtifact
pub type vyre_driver_wgpu::runtime::aot::AotArtifact::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::aot::AotArtifact where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::aot::AotArtifact where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::aot::AotArtifact where T: core::marker::Sync
pub fn vyre_driver_wgpu::runtime::aot::backend_fingerprint() -> alloc::string::String
pub fn vyre_driver_wgpu::runtime::aot::cache_dir() -> std::path::PathBuf
pub fn vyre_driver_wgpu::runtime::aot::cache_key(spec_hash: &str, backend_fingerprint: &str) -> alloc::string::String
pub fn vyre_driver_wgpu::runtime::aot::load_or_compile(program: &vyre_foundation::ir_inner::model::program::core::Program, fingerprint: &str) -> core::result::Result<vyre_driver_wgpu::runtime::aot::AotArtifact, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::aot::load_or_compile_with_config(program: &vyre_foundation::ir_inner::model::program::core::Program, fingerprint: &str, config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<vyre_driver_wgpu::runtime::aot::AotArtifact, vyre_driver::backend::error::BackendError>
pub mod vyre_driver_wgpu::runtime::cache
pub mod vyre_driver_wgpu::runtime::cache::lru
pub struct vyre_driver_wgpu::runtime::cache::lru::AccessMeta
pub vyre_driver_wgpu::runtime::cache::lru::AccessMeta::frequency: u32
pub vyre_driver_wgpu::runtime::cache::lru::AccessMeta::last_access: u64
pub vyre_driver_wgpu::runtime::cache::lru::AccessMeta::size: u64
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::clone(&self) -> vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::default::Default for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::default() -> vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::lru::AccessMeta::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::lru::AccessMeta::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::lru::AccessMeta::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessMeta::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::lru::AccessMeta
pub type vyre_driver_wgpu::runtime::cache::lru::AccessMeta::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::lru::AccessMeta where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::hot_set(&self, n: usize) -> alloc::vec::Vec<u64>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::new() -> Self
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::record(&mut self, key: u64)
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::stats(&self, key: u64) -> core::option::Option<vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_new() -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::default::Default for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::default() -> Self
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
impl<K, V> vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where K: core::hash::Hash + core::cmp::Eq + core::marker::Copy, V: core::default::Default
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::ensure(&mut self, key: K) -> &mut V
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::ensure_front(&mut self, key: K) -> &mut V
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::get(&self, key: &K) -> core::option::Option<&V>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::hottest(&self, n: usize) -> alloc::vec::Vec<K>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::iter_coldest(&self) -> impl core::iter::traits::iterator::Iterator<Item = (&K, &V)> + '_
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::iter_hottest(&self) -> impl core::iter::traits::iterator::Iterator<Item = (&K, &V)> + '_
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::new() -> Self
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::remove(&mut self, key: &K)
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::touch(&mut self, key: K)
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::try_new() -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::try_with_capacity(capacity: usize) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::try_with_reserved_capacity(capacity: usize) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::with_capacity(capacity: usize) -> Self
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::with_reserved_capacity(capacity: usize) -> Self
impl<K, V> core::default::Default for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where K: core::hash::Hash + core::cmp::Eq + core::marker::Copy, V: core::default::Default
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::default() -> Self
impl<K, V> core::marker::Freeze for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
impl<K, V> core::marker::Send for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where K: core::marker::Send, V: core::marker::Send
impl<K, V> core::marker::Sync for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where K: core::marker::Sync, V: core::marker::Sync
impl<K, V> core::marker::Unpin for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where K: core::marker::Unpin, V: core::marker::Unpin
impl<K, V> core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
impl<K, V> core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where K: core::panic::unwind_safe::RefUnwindSafe, V: core::panic::unwind_safe::RefUnwindSafe
impl<K, V> core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where K: core::panic::unwind_safe::UnwindSafe, V: core::panic::unwind_safe::UnwindSafe
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
pub fn vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>
pub type vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V>::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::lru::IntrusiveLru<K, V> where T: core::marker::Sync
pub const vyre_driver_wgpu::runtime::cache::lru::DEFAULT_INTRUSIVE_LRU_CAPACITY: usize
pub mod vyre_driver_wgpu::runtime::cache::tiered_cache
#[non_exhaustive] pub enum vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::CapacityAccountingOverflow
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::EntryTooLarge
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::KeyNotFound
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone(&self) -> vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::cmp::Eq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::eq(&self, other: &vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError) -> bool
impl core::error::Error for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::frequency: u32
pub vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::last_access: u64
pub vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::size: u64
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::key: u64
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::size: u64
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::tier: usize
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::clone(&self) -> vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::cmp::Eq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::eq(&self, other: &vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::capacity: u64
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::name: alloc::string::String
pub vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::used: u64
impl vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::new(name: impl core::convert::Into<alloc::string::String>, capacity: u64) -> Self
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::try_new(name: impl core::convert::Into<alloc::string::String>, capacity: u64) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::promote_threshold: u32
impl vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub const vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::DEFAULT_THRESHOLD: u32
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::new(promote_threshold: u32) -> Self
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone(&self) -> vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::cmp::Eq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::eq(&self, other: &vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy) -> bool
impl core::default::Default for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::default() -> Self
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::demote(&mut self, key: u64) -> core::result::Result<(), vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::evict_coldest(&mut self) -> core::option::Option<u64>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::get(&self, key: u64) -> core::option::Option<&vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::insert(&mut self, key: u64, size: u64) -> core::result::Result<(), vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::promote(&mut self, key: u64) -> core::result::Result<(), vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::record_access(&mut self, key: u64)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::try_with_policy(tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>, policy: vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::with_policy(tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>, policy: vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy) -> Self
impl vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::new(tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>) -> Self
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::try_new(tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: core::marker::Sync
#[non_exhaustive] pub enum vyre_driver_wgpu::runtime::cache::CacheError
pub vyre_driver_wgpu::runtime::cache::CacheError::CapacityAccountingOverflow
pub vyre_driver_wgpu::runtime::cache::CacheError::EntryTooLarge
pub vyre_driver_wgpu::runtime::cache::CacheError::KeyNotFound
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone(&self) -> vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::cmp::Eq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::eq(&self, other: &vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError) -> bool
impl core::error::Error for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::AccessStats
pub vyre_driver_wgpu::runtime::cache::AccessStats::frequency: u32
pub vyre_driver_wgpu::runtime::cache::AccessStats::last_access: u64
pub vyre_driver_wgpu::runtime::cache::AccessStats::size: u64
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::AccessTracker
impl vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::hot_set(&self, n: usize) -> alloc::vec::Vec<u64>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::new() -> Self
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::record(&mut self, key: u64)
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::stats(&self, key: u64) -> core::option::Option<vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_new() -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::default::Default for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::default() -> Self
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::CacheEntry
pub vyre_driver_wgpu::runtime::cache::CacheEntry::key: u64
pub vyre_driver_wgpu::runtime::cache::CacheEntry::size: u64
pub vyre_driver_wgpu::runtime::cache::CacheEntry::tier: usize
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::clone(&self) -> vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::cmp::Eq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::eq(&self, other: &vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::CacheTier
pub vyre_driver_wgpu::runtime::cache::CacheTier::capacity: u64
pub vyre_driver_wgpu::runtime::cache::CacheTier::name: alloc::string::String
pub vyre_driver_wgpu::runtime::cache::CacheTier::used: u64
impl vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::new(name: impl core::convert::Into<alloc::string::String>, capacity: u64) -> Self
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::try_new(name: impl core::convert::Into<alloc::string::String>, capacity: u64) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::LruPolicy
pub vyre_driver_wgpu::runtime::cache::LruPolicy::promote_threshold: u32
impl vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub const vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::DEFAULT_THRESHOLD: u32
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::new(promote_threshold: u32) -> Self
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone(&self) -> vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::cmp::Eq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::eq(&self, other: &vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy) -> bool
impl core::default::Default for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::default() -> Self
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::cache::TieredCache
impl vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::demote(&mut self, key: u64) -> core::result::Result<(), vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::evict_coldest(&mut self) -> core::option::Option<u64>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::get(&self, key: u64) -> core::option::Option<&vyre_driver_wgpu::runtime::cache::tiered_cache::CacheEntry>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::insert(&mut self, key: u64, size: u64) -> core::result::Result<(), vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::promote(&mut self, key: u64) -> core::result::Result<(), vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::record_access(&mut self, key: u64)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::try_with_policy(tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>, policy: vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::with_policy(tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>, policy: vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy) -> Self
impl vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::new(tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>) -> Self
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::try_new(tiers: alloc::vec::Vec<vyre_driver_wgpu::runtime::cache::tiered_cache::CacheTier>) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::TieredCache where T: core::marker::Sync
pub mod vyre_driver_wgpu::runtime::device
pub struct vyre_driver_wgpu::runtime::device::AdapterCriteria
pub vyre_driver_wgpu::runtime::device::AdapterCriteria::device_type: core::option::Option<wgpu_types::DeviceType>
pub vyre_driver_wgpu::runtime::device::AdapterCriteria::name_contains: core::option::Option<alloc::string::String>
pub vyre_driver_wgpu::runtime::device::AdapterCriteria::power: core::option::Option<wgpu_types::PowerPreference>
pub vyre_driver_wgpu::runtime::device::AdapterCriteria::vendor: core::option::Option<u32>
impl vyre_driver_wgpu::runtime::device::AdapterCriteria
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::high_performance() -> Self
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::low_power() -> Self
impl core::clone::Clone for vyre_driver_wgpu::runtime::device::AdapterCriteria
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::clone(&self) -> vyre_driver_wgpu::runtime::device::AdapterCriteria
impl core::default::Default for vyre_driver_wgpu::runtime::device::AdapterCriteria
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::default() -> vyre_driver_wgpu::runtime::device::AdapterCriteria
impl core::fmt::Debug for vyre_driver_wgpu::runtime::device::AdapterCriteria
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl core::marker::Send for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl core::marker::Sync for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl core::marker::Unpin for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::device::AdapterCriteria where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::device::AdapterCriteria where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::device::AdapterCriteria::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::device::AdapterCriteria where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::device::AdapterCriteria::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::device::AdapterCriteria where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::device::AdapterCriteria::Owned = T
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::device::AdapterCriteria where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::device::AdapterCriteria where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::device::AdapterCriteria where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::device::AdapterCriteria where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::device::AdapterCriteria::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::device::AdapterCriteria
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::device::AdapterCriteria
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::device::AdapterCriteria
pub fn vyre_driver_wgpu::runtime::device::AdapterCriteria::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::device::AdapterCriteria
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::device::AdapterCriteria
pub type vyre_driver_wgpu::runtime::device::AdapterCriteria::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::device::AdapterCriteria where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::device::AdapterCriteria where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::device::AdapterCriteria where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub vyre_driver_wgpu::runtime::device::AdapterProbeReport::missing: alloc::vec::Vec<alloc::string::String>
pub vyre_driver_wgpu::runtime::device::AdapterProbeReport::probed: alloc::vec::Vec<alloc::string::String>
impl core::clone::Clone for vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::clone(&self) -> vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::cmp::Eq for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::eq(&self, other: &vyre_driver_wgpu::runtime::device::AdapterProbeReport) -> bool
impl core::default::Default for vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::default() -> vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::fmt::Debug for vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::marker::Freeze for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::marker::Send for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::marker::Sync for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::marker::Unpin for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::device::AdapterProbeReport where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::device::AdapterProbeReport where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::device::AdapterProbeReport where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::device::AdapterProbeReport where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::device::AdapterProbeReport where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::device::AdapterProbeReport::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::device::AdapterProbeReport where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::device::AdapterProbeReport::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::device::AdapterProbeReport where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::device::AdapterProbeReport::Owned = T
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::device::AdapterProbeReport where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::device::AdapterProbeReport where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::device::AdapterProbeReport where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::device::AdapterProbeReport where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub fn vyre_driver_wgpu::runtime::device::AdapterProbeReport::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::device::AdapterProbeReport
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub type vyre_driver_wgpu::runtime::device::AdapterProbeReport::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::device::AdapterProbeReport where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::device::AdapterProbeReport where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::device::AdapterProbeReport where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::device::EnabledFeatures
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::indirect_first_instance: bool
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::max_storage_buffer_binding_size: u64
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::max_subgroup_size: u32
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::max_workgroup_size: [u32; 3]
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::min_subgroup_size: u32
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::pipeline_cache: bool
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::push_constants: bool
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::shader_f16: bool
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::subgroup: bool
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::subgroup_barrier: bool
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::timestamp_query: bool
pub vyre_driver_wgpu::runtime::device::EnabledFeatures::timestamp_query_inside_encoders: bool
impl core::clone::Clone for vyre_driver_wgpu::runtime::device::EnabledFeatures
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::clone(&self) -> vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::cmp::Eq for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::device::EnabledFeatures
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::eq(&self, other: &vyre_driver_wgpu::runtime::device::EnabledFeatures) -> bool
impl core::default::Default for vyre_driver_wgpu::runtime::device::EnabledFeatures
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::default() -> vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::fmt::Debug for vyre_driver_wgpu::runtime::device::EnabledFeatures
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::marker::Freeze for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::marker::Send for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::marker::Sync for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::marker::Unpin for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::device::EnabledFeatures where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::device::EnabledFeatures where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::device::EnabledFeatures where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::device::EnabledFeatures where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::device::EnabledFeatures where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::device::EnabledFeatures::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::device::EnabledFeatures where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::device::EnabledFeatures::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::device::EnabledFeatures where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::device::EnabledFeatures::Owned = T
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::device::EnabledFeatures where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::device::EnabledFeatures where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::device::EnabledFeatures where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::device::EnabledFeatures where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::device::EnabledFeatures::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::device::EnabledFeatures
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::device::EnabledFeatures
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::device::EnabledFeatures
pub fn vyre_driver_wgpu::runtime::device::EnabledFeatures::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::device::EnabledFeatures
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::device::EnabledFeatures
pub type vyre_driver_wgpu::runtime::device::EnabledFeatures::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::device::EnabledFeatures where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::device::EnabledFeatures where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::device::EnabledFeatures where T: core::marker::Sync
pub async fn vyre_driver_wgpu::runtime::device::acquire_gpu() -> vyre_foundation::error::Result<((wgpu::api::device::Device, wgpu::api::queue::Queue), wgpu_types::AdapterInfo, vyre_driver_wgpu::runtime::device::EnabledFeatures)>
pub async fn vyre_driver_wgpu::runtime::device::acquire_gpu_for_adapter(index: usize) -> vyre_foundation::error::Result<((wgpu::api::device::Device, wgpu::api::queue::Queue), wgpu_types::AdapterInfo, vyre_driver_wgpu::runtime::device::EnabledFeatures)>
pub fn vyre_driver_wgpu::runtime::device::adapter_for_info(expected: &wgpu_types::AdapterInfo) -> vyre_foundation::error::Result<wgpu::api::adapter::Adapter>
pub fn vyre_driver_wgpu::runtime::device::adapter_index_from_env() -> vyre_foundation::error::Result<core::option::Option<usize>>
pub fn vyre_driver_wgpu::runtime::device::adapter_probe_report() -> vyre_driver_wgpu::runtime::device::AdapterProbeReport
pub fn vyre_driver_wgpu::runtime::device::cached_adapter_info() -> vyre_foundation::error::Result<&'static wgpu_types::AdapterInfo>
pub fn vyre_driver_wgpu::runtime::device::cached_device() -> vyre_foundation::error::Result<alloc::sync::Arc<(wgpu::api::device::Device, wgpu::api::queue::Queue)>>
pub fn vyre_driver_wgpu::runtime::device::enumerate_adapters() -> alloc::vec::Vec<wgpu_types::AdapterInfo>
pub fn vyre_driver_wgpu::runtime::device::has_real_gpu_adapter() -> bool
pub fn vyre_driver_wgpu::runtime::device::init_device() -> vyre_foundation::error::Result<((wgpu::api::device::Device, wgpu::api::queue::Queue), wgpu_types::AdapterInfo, vyre_driver_wgpu::runtime::device::EnabledFeatures)>
pub fn vyre_driver_wgpu::runtime::device::init_device_for_adapter(index: usize) -> vyre_foundation::error::Result<((wgpu::api::device::Device, wgpu::api::queue::Queue), wgpu_types::AdapterInfo, vyre_driver_wgpu::runtime::device::EnabledFeatures)>
pub fn vyre_driver_wgpu::runtime::device::select_adapter(criteria: &vyre_driver_wgpu::runtime::device::AdapterCriteria) -> vyre_foundation::error::Result<(usize, wgpu_types::AdapterInfo)>
pub mod vyre_driver_wgpu::runtime::indirect
pub struct vyre_driver_wgpu::runtime::indirect::IndirectArgs
pub vyre_driver_wgpu::runtime::indirect::IndirectArgs::buffer: alloc::sync::Arc<wgpu::api::buffer::Buffer>
pub vyre_driver_wgpu::runtime::indirect::IndirectArgs::offset: u64
impl vyre_driver_wgpu::runtime::indirect::IndirectArgs
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::from_handle(handle: &vyre_driver_wgpu::buffer::GpuBufferHandle, offset: u64) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::marker::Freeze for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl core::marker::Send for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl core::marker::Sync for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl core::marker::Unpin for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::indirect::IndirectArgs where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::indirect::IndirectArgs where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::indirect::IndirectArgs::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::indirect::IndirectArgs where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::indirect::IndirectArgs::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::indirect::IndirectArgs where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::indirect::IndirectArgs where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::indirect::IndirectArgs where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::indirect::IndirectArgs
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::indirect::IndirectArgs
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::indirect::IndirectArgs
pub fn vyre_driver_wgpu::runtime::indirect::IndirectArgs::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::indirect::IndirectArgs
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::indirect::IndirectArgs
pub type vyre_driver_wgpu::runtime::indirect::IndirectArgs::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::indirect::IndirectArgs where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::indirect::IndirectArgs where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::indirect::IndirectArgs where T: core::marker::Sync
pub const vyre_driver_wgpu::runtime::indirect::INDIRECT_ARGS_BYTES: u64
pub fn vyre_driver_wgpu::runtime::indirect::dispatch_indirect<'a>(pass: &mut wgpu::api::compute_pass::ComputePass<'a>, args: &'a vyre_driver_wgpu::runtime::indirect::IndirectArgs)
pub mod vyre_driver_wgpu::runtime::prerecorded
pub struct vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
pub vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::bind_groups: alloc::vec::Vec<alloc::sync::Arc<wgpu::api::bind_group::BindGroup>>
pub vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::cb: std::sync::poison::mutex::Mutex<core::option::Option<wgpu::api::command_buffer::CommandBuffer>>
pub vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::device: wgpu::api::device::Device
pub vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::handles: alloc::vec::Vec<vyre_driver_wgpu::buffer::GpuBufferHandle>
pub vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::output_handles: alloc::vec::Vec<vyre_driver_wgpu::buffer::GpuBufferHandle>
pub vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::queue: wgpu::api::queue::Queue
impl vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::read_output(&self, index: usize) -> core::result::Result<alloc::vec::Vec<u8>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::read_output_into(&self, index: usize, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::replay(&self, queue: &wgpu::api::queue::Queue) -> core::result::Result<wgpu::api::queue::SubmissionIndex, vyre_driver::backend::error::BackendError>
impl !core::marker::Freeze for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl core::marker::Send for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl core::marker::Sync for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl core::marker::Unpin for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
pub fn vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch
pub type vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::prerecorded::PrerecordedDispatch where T: core::marker::Sync
pub mod vyre_driver_wgpu::runtime::readback_ring
pub enum vyre_driver_wgpu::runtime::readback_ring::SlotState
pub vyre_driver_wgpu::runtime::readback_ring::SlotState::Error
pub vyre_driver_wgpu::runtime::readback_ring::SlotState::Free
pub vyre_driver_wgpu::runtime::readback_ring::SlotState::Pending
pub vyre_driver_wgpu::runtime::readback_ring::SlotState::Ready
impl core::clone::Clone for vyre_driver_wgpu::runtime::readback_ring::SlotState
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::clone(&self) -> vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::cmp::Eq for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::readback_ring::SlotState
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::eq(&self, other: &vyre_driver_wgpu::runtime::readback_ring::SlotState) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::runtime::readback_ring::SlotState
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::marker::Freeze for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::marker::Send for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::marker::Sync for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::marker::Unpin for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::readback_ring::SlotState where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::readback_ring::SlotState where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::readback_ring::SlotState where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::readback_ring::SlotState where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::readback_ring::SlotState where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::readback_ring::SlotState::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::readback_ring::SlotState where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::readback_ring::SlotState::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::readback_ring::SlotState where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::readback_ring::SlotState::Owned = T
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::readback_ring::SlotState where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::readback_ring::SlotState where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::readback_ring::SlotState where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::readback_ring::SlotState where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::readback_ring::SlotState::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::readback_ring::SlotState
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::readback_ring::SlotState
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::readback_ring::SlotState
pub fn vyre_driver_wgpu::runtime::readback_ring::SlotState::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::readback_ring::SlotState
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::readback_ring::SlotState
pub type vyre_driver_wgpu::runtime::readback_ring::SlotState::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::readback_ring::SlotState where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::readback_ring::SlotState where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::readback_ring::SlotState where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::readback_ring::GpuSlot
pub vyre_driver_wgpu::runtime::readback_ring::GpuSlot::buffer: wgpu::api::buffer::Buffer
pub vyre_driver_wgpu::runtime::readback_ring::GpuSlot::state: alloc::sync::Arc<core::sync::atomic::AtomicU8>
impl !core::marker::Freeze for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl core::marker::Send for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl core::marker::Sync for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl core::marker::Unpin for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::readback_ring::GpuSlot::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::readback_ring::GpuSlot::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
pub fn vyre_driver_wgpu::runtime::readback_ring::GpuSlot::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::readback_ring::GpuSlot
pub type vyre_driver_wgpu::runtime::readback_ring::GpuSlot::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::readback_ring::GpuSlot where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::arm_ticket(&self, ticket: &vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket) -> core::result::Result<(crossbeam_channel::channel::Receiver<vyre_driver_wgpu::runtime::readback_ring::MapResult>, alloc::sync::Arc<core::sync::atomic::AtomicBool>), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::collect_slot(&self, device: &wgpu::api::device::Device, idx: usize) -> core::result::Result<core::option::Option<alloc::vec::Vec<u8>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::collect_slot_into(&self, device: &wgpu::api::device::Device, idx: usize, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<core::option::Option<usize>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::new(device: &wgpu::api::device::Device, size: usize, buffer_size: u64) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::record_copy(&self, device: &wgpu::api::device::Device, encoder: &mut wgpu::api::command_encoder::CommandEncoder, src_buffer: &wgpu::api::buffer::Buffer, src_offset: u64, byte_len: u64) -> core::result::Result<vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::submit_readback(&self, device: &wgpu::api::device::Device, queue: &wgpu::api::queue::Queue, src_buffer: &wgpu::api::buffer::Buffer, src_offset: u64, byte_len: u64) -> core::result::Result<usize, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::with_mapped_ticket<R>(&self, ticket: &vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket, visitor: impl core::ops::function::FnOnce(&[u8]) -> core::result::Result<R, vyre_driver::backend::error::BackendError>) -> core::result::Result<R, vyre_driver::backend::error::BackendError>
impl !core::marker::Freeze for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl core::marker::Send for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl core::marker::Sync for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl core::marker::Unpin for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackRing::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::readback_ring::ReadbackRing where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::existing_ring_for(&self, byte_len: u64) -> core::result::Result<core::option::Option<alloc::sync::Arc<vyre_driver_wgpu::runtime::readback_ring::ReadbackRing>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::new() -> Self
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::ring_for(&self, device: &wgpu::api::device::Device, byte_len: u64) -> core::result::Result<alloc::sync::Arc<vyre_driver_wgpu::runtime::readback_ring::ReadbackRing>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::slots_per_ring(&self) -> usize
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::with_requested_slots(raw_slots: core::option::Option<&str>) -> Self
impl core::default::Default for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::default() -> Self
impl core::marker::Freeze for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl core::marker::Send for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl core::marker::Sync for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl core::marker::Unpin for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::readback_ring::ReadbackRingSet where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl core::marker::Freeze for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl core::marker::Send for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl core::marker::Sync for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl core::marker::Unpin for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
pub fn vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket
pub type vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::readback_ring::ReadbackTicket where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::readback_ring::RingStats
pub vyre_driver_wgpu::runtime::readback_ring::RingStats::dispatches: core::sync::atomic::AtomicU64
pub vyre_driver_wgpu::runtime::readback_ring::RingStats::peak_inflight: core::sync::atomic::AtomicU64
pub vyre_driver_wgpu::runtime::readback_ring::RingStats::readback_stalls: core::sync::atomic::AtomicU64
impl vyre_driver_wgpu::runtime::readback_ring::RingStats
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::record_dispatch(&self) -> u64
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::record_stall(&self)
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::update_peak(&self, current: u64)
impl core::default::Default for vyre_driver_wgpu::runtime::readback_ring::RingStats
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::default() -> vyre_driver_wgpu::runtime::readback_ring::RingStats
impl core::fmt::Debug for vyre_driver_wgpu::runtime::readback_ring::RingStats
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl !core::marker::Freeze for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl core::marker::Send for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl core::marker::Sync for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl core::marker::Unpin for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::readback_ring::RingStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::readback_ring::RingStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::readback_ring::RingStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::readback_ring::RingStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::readback_ring::RingStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::readback_ring::RingStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::readback_ring::RingStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::readback_ring::RingStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::readback_ring::RingStats
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::readback_ring::RingStats
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::readback_ring::RingStats
pub fn vyre_driver_wgpu::runtime::readback_ring::RingStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::readback_ring::RingStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::readback_ring::RingStats
pub type vyre_driver_wgpu::runtime::readback_ring::RingStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::readback_ring::RingStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::readback_ring::RingStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::readback_ring::RingStats where T: core::marker::Sync
pub type vyre_driver_wgpu::runtime::readback_ring::MapResult = core::result::Result<(), wgpu::api::buffer::BufferAsyncError>
pub mod vyre_driver_wgpu::runtime::router
#[non_exhaustive] pub enum vyre_driver_wgpu::runtime::router::Override<'a>
pub vyre_driver_wgpu::runtime::router::Override::Explicit(&'a str)
pub vyre_driver_wgpu::runtime::router::Override::FromEnv
pub vyre_driver_wgpu::runtime::router::Override::None
impl<'a> core::clone::Clone for vyre_driver_wgpu::runtime::router::Override<'a>
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::clone(&self) -> vyre_driver_wgpu::runtime::router::Override<'a>
impl<'a> core::fmt::Debug for vyre_driver_wgpu::runtime::router::Override<'a>
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl<'a> core::marker::Copy for vyre_driver_wgpu::runtime::router::Override<'a>
impl<'a> core::marker::Freeze for vyre_driver_wgpu::runtime::router::Override<'a>
impl<'a> core::marker::Send for vyre_driver_wgpu::runtime::router::Override<'a>
impl<'a> core::marker::Sync for vyre_driver_wgpu::runtime::router::Override<'a>
impl<'a> core::marker::Unpin for vyre_driver_wgpu::runtime::router::Override<'a>
impl<'a> core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::router::Override<'a>
impl<'a> core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::router::Override<'a>
impl<'a> core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::router::Override<'a>
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::router::Override<'a> where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::router::Override<'a> where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::router::Override<'a>::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::router::Override<'a> where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::router::Override<'a>::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::router::Override<'a> where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::router::Override<'a>::Owned = T
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::router::Override<'a> where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::router::Override<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::router::Override<'a> where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::router::Override<'a> where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::router::Override<'a>::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::router::Override<'a>
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::router::Override<'a>
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::router::Override<'a>
pub fn vyre_driver_wgpu::runtime::router::Override<'a>::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::router::Override<'a>
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::router::Override<'a>
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::router::Override<'a>
pub type vyre_driver_wgpu::runtime::router::Override<'a>::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::router::Override<'a> where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::router::Override<'a> where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::router::Override<'a> where T: core::marker::Sync
#[non_exhaustive] pub enum vyre_driver_wgpu::runtime::router::Reason
pub vyre_driver_wgpu::runtime::router::Reason::EnvOverride
pub vyre_driver_wgpu::runtime::router::Reason::Precedence
impl core::clone::Clone for vyre_driver_wgpu::runtime::router::Reason
pub fn vyre_driver_wgpu::runtime::router::Reason::clone(&self) -> vyre_driver_wgpu::runtime::router::Reason
impl core::cmp::Eq for vyre_driver_wgpu::runtime::router::Reason
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::router::Reason
pub fn vyre_driver_wgpu::runtime::router::Reason::eq(&self, other: &vyre_driver_wgpu::runtime::router::Reason) -> bool
impl core::fmt::Debug for vyre_driver_wgpu::runtime::router::Reason
pub fn vyre_driver_wgpu::runtime::router::Reason::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::router::Reason
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::router::Reason
impl core::marker::Freeze for vyre_driver_wgpu::runtime::router::Reason
impl core::marker::Send for vyre_driver_wgpu::runtime::router::Reason
impl core::marker::Sync for vyre_driver_wgpu::runtime::router::Reason
impl core::marker::Unpin for vyre_driver_wgpu::runtime::router::Reason
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::router::Reason
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::router::Reason
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::router::Reason
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::router::Reason where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::Reason::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::router::Reason where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::router::Reason where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::Reason::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::router::Reason::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::router::Reason where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::router::Reason::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::router::Reason where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::router::Reason::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::router::Reason::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::router::Reason where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::router::Reason::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::router::Reason::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::router::Reason where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::router::Reason::Owned = T
pub fn vyre_driver_wgpu::runtime::router::Reason::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::router::Reason::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::router::Reason where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::Reason::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::router::Reason where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::Reason::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::router::Reason where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::Reason::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::router::Reason where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::router::Reason::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::router::Reason
pub fn vyre_driver_wgpu::runtime::router::Reason::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::router::Reason
pub fn vyre_driver_wgpu::runtime::router::Reason::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::router::Reason
pub fn vyre_driver_wgpu::runtime::router::Reason::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::router::Reason
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::router::Reason
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::router::Reason
pub type vyre_driver_wgpu::runtime::router::Reason::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::router::Reason where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::router::Reason where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::router::Reason where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::router::BackendRouter
impl vyre_driver_wgpu::runtime::router::BackendRouter
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::enumerate_by_precedence() -> alloc::vec::Vec<&'static vyre_driver::backend::registry::inventory_streams::BackendRegistration>
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::new() -> Self
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::pick(&self, program: &vyre_foundation::ir_inner::model::program::core::Program) -> core::result::Result<vyre_driver_wgpu::runtime::router::RouterDecision, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::pick_with_override(&self, _program: &vyre_foundation::ir_inner::model::program::core::Program, source: vyre_driver_wgpu::runtime::router::Override<'_>) -> core::result::Result<vyre_driver_wgpu::runtime::router::RouterDecision, vyre_driver::backend::error::BackendError>
impl core::default::Default for vyre_driver_wgpu::runtime::router::BackendRouter
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::default() -> vyre_driver_wgpu::runtime::router::BackendRouter
impl core::marker::Freeze for vyre_driver_wgpu::runtime::router::BackendRouter
impl core::marker::Send for vyre_driver_wgpu::runtime::router::BackendRouter
impl core::marker::Sync for vyre_driver_wgpu::runtime::router::BackendRouter
impl core::marker::Unpin for vyre_driver_wgpu::runtime::router::BackendRouter
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::router::BackendRouter
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::router::BackendRouter
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::router::BackendRouter
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::router::BackendRouter where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::router::BackendRouter where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::router::BackendRouter::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::router::BackendRouter where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::router::BackendRouter::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::router::BackendRouter where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::router::BackendRouter where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::router::BackendRouter where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::router::BackendRouter
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::router::BackendRouter
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::router::BackendRouter
pub fn vyre_driver_wgpu::runtime::router::BackendRouter::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::router::BackendRouter
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::router::BackendRouter
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::router::BackendRouter
pub type vyre_driver_wgpu::runtime::router::BackendRouter::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::router::BackendRouter where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::router::BackendRouter where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::router::BackendRouter where T: core::marker::Sync
pub struct vyre_driver_wgpu::runtime::router::RouterDecision
pub vyre_driver_wgpu::runtime::router::RouterDecision::backend: &'static str
pub vyre_driver_wgpu::runtime::router::RouterDecision::reason: vyre_driver_wgpu::runtime::router::Reason
impl core::clone::Clone for vyre_driver_wgpu::runtime::router::RouterDecision
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::clone(&self) -> vyre_driver_wgpu::runtime::router::RouterDecision
impl core::fmt::Debug for vyre_driver_wgpu::runtime::router::RouterDecision
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::runtime::router::RouterDecision
impl core::marker::Send for vyre_driver_wgpu::runtime::router::RouterDecision
impl core::marker::Sync for vyre_driver_wgpu::runtime::router::RouterDecision
impl core::marker::Unpin for vyre_driver_wgpu::runtime::router::RouterDecision
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::router::RouterDecision
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::router::RouterDecision
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::router::RouterDecision
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::router::RouterDecision where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::router::RouterDecision where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::router::RouterDecision::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::router::RouterDecision where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::router::RouterDecision::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::router::RouterDecision where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::router::RouterDecision::Owned = T
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::router::RouterDecision where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::router::RouterDecision where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::router::RouterDecision where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::router::RouterDecision where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::router::RouterDecision::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::router::RouterDecision
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::router::RouterDecision
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::router::RouterDecision
pub fn vyre_driver_wgpu::runtime::router::RouterDecision::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::router::RouterDecision
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::router::RouterDecision
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::router::RouterDecision
pub type vyre_driver_wgpu::runtime::router::RouterDecision::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::router::RouterDecision where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::router::RouterDecision where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::router::RouterDecision where T: core::marker::Sync
pub mod vyre_driver_wgpu::runtime::serializer
pub mod vyre_driver_wgpu::runtime::serializer::decode_parts
pub fn vyre_driver_wgpu::runtime::serializer::decode_parts::decode_parts(bytes: &[u8]) -> vyre_foundation::error::Result<alloc::vec::Vec<&[u8]>>
pub mod vyre_driver_wgpu::runtime::serializer::encode_parts
pub const vyre_driver_wgpu::runtime::serializer::encode_parts::MAX_SERIALIZED_PART_BYTES: usize
pub fn vyre_driver_wgpu::runtime::serializer::encode_parts::encode_parts(parts: &[&[u8]]) -> vyre_foundation::error::Result<alloc::vec::Vec<u8>>
pub const vyre_driver_wgpu::runtime::serializer::MAX_SERIALIZED_PART_BYTES: usize
pub fn vyre_driver_wgpu::runtime::serializer::decode_parts(bytes: &[u8]) -> vyre_foundation::error::Result<alloc::vec::Vec<&[u8]>>
pub fn vyre_driver_wgpu::runtime::serializer::encode_parts(parts: &[&[u8]]) -> vyre_foundation::error::Result<alloc::vec::Vec<u8>>
pub mod vyre_driver_wgpu::runtime::shader
pub mod vyre_driver_wgpu::runtime::shader::compile_compute_pipeline
pub fn vyre_driver_wgpu::runtime::shader::compile_compute_pipeline::compile_compute_pipeline(device: &wgpu::api::device::Device, label: &str, wgsl_source: &str, entry_point: &str) -> vyre_foundation::error::Result<wgpu::api::compute_pipeline::ComputePipeline>
pub fn vyre_driver_wgpu::runtime::shader::compile_compute_pipeline::compile_compute_pipeline_with_layout(device: &wgpu::api::device::Device, label: &str, wgsl_source: &str, entry_point: &str, layout: core::option::Option<&wgpu::api::pipeline_layout::PipelineLayout>) -> vyre_foundation::error::Result<wgpu::api::compute_pipeline::ComputePipeline>
#[non_exhaustive] pub enum vyre_driver_wgpu::runtime::CacheError
pub vyre_driver_wgpu::runtime::CacheError::CapacityAccountingOverflow
pub vyre_driver_wgpu::runtime::CacheError::EntryTooLarge
pub vyre_driver_wgpu::runtime::CacheError::KeyNotFound
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone(&self) -> vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::cmp::Eq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::eq(&self, other: &vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError) -> bool
impl core::error::Error for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::fmt::Display for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::to_owned(&self) -> T
impl<T> alloc::string::ToString for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::fmt::Display + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::to_string(&self) -> alloc::string::String
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::CacheError where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::AccessStats
pub vyre_driver_wgpu::runtime::AccessStats::frequency: u32
pub vyre_driver_wgpu::runtime::AccessStats::last_access: u64
pub vyre_driver_wgpu::runtime::AccessStats::size: u64
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::AccessTracker
impl vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::hot_set(&self, n: usize) -> alloc::vec::Vec<u64>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::new() -> Self
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::record(&mut self, key: u64)
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::stats(&self, key: u64) -> core::option::Option<vyre_driver_wgpu::runtime::cache::tiered_cache::AccessStats>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_new() -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
impl core::default::Default for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::default() -> Self
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub fn vyre_driver_wgpu::runtime::cache::lru::AccessTracker::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::lru::AccessTracker
pub type vyre_driver_wgpu::runtime::cache::lru::AccessTracker::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::lru::AccessTracker where T: core::marker::Sync
#[non_exhaustive] pub struct vyre_driver_wgpu::runtime::LruPolicy
pub vyre_driver_wgpu::runtime::LruPolicy::promote_threshold: u32
impl vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub const vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::DEFAULT_THRESHOLD: u32
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::new(promote_threshold: u32) -> Self
impl core::clone::Clone for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone(&self) -> vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::cmp::Eq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::cmp::PartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::eq(&self, other: &vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy) -> bool
impl core::default::Default for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::default() -> Self
impl core::fmt::Debug for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Copy for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::StructuralPartialEq for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Freeze for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Send for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Sync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::Unpin for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<Q, K> equivalent::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
impl<Q, K> hashbrown::Equivalent<K> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where Q: core::cmp::Eq + ?core::marker::Sized, K: core::borrow::Borrow<Q> + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::equivalent(&self, key: &K) -> bool
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::From<T>
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::Into<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::clone::Clone
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Owned = T
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub fn vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy
pub type vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::runtime::cache::tiered_cache::LruPolicy where T: core::marker::Sync
pub fn vyre_driver_wgpu::runtime::bg_entry(binding: u32, buffer: &wgpu::api::buffer::Buffer) -> wgpu::api::bind_group::BindGroupEntry<'_>
pub fn vyre_driver_wgpu::runtime::cached_adapter_info() -> vyre_foundation::error::Result<&'static wgpu_types::AdapterInfo>
pub fn vyre_driver_wgpu::runtime::cached_device() -> vyre_foundation::error::Result<alloc::sync::Arc<(wgpu::api::device::Device, wgpu::api::queue::Queue)>>
pub fn vyre_driver_wgpu::runtime::compile_compute_pipeline(device: &wgpu::api::device::Device, label: &str, wgsl_source: &str, entry_point: &str) -> vyre_foundation::error::Result<wgpu::api::compute_pipeline::ComputePipeline>
pub fn vyre_driver_wgpu::runtime::compile_compute_pipeline_with_layout(device: &wgpu::api::device::Device, label: &str, wgsl_source: &str, entry_point: &str, layout: core::option::Option<&wgpu::api::pipeline_layout::PipelineLayout>) -> vyre_foundation::error::Result<wgpu::api::compute_pipeline::ComputePipeline>
pub fn vyre_driver_wgpu::runtime::init_device() -> vyre_foundation::error::Result<((wgpu::api::device::Device, wgpu::api::queue::Queue), wgpu_types::AdapterInfo, vyre_driver_wgpu::runtime::device::EnabledFeatures)>
pub mod vyre_driver_wgpu::spirv_backend
pub struct vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl vyre_driver_wgpu::spirv_backend::SpirvEmitter
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::default_flags() -> naga::back::spv::WriterFlags
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::emit(module: &naga::ir::Module, entry: &str) -> core::result::Result<alloc::vec::Vec<u32>, alloc::string::String>
impl core::marker::Freeze for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl core::marker::Send for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl core::marker::Sync for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl core::marker::Unpin for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::spirv_backend::SpirvEmitter where U: core::convert::From<T>
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::spirv_backend::SpirvEmitter where U: core::convert::Into<T>
pub type vyre_driver_wgpu::spirv_backend::SpirvEmitter::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::spirv_backend::SpirvEmitter where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::spirv_backend::SpirvEmitter::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::spirv_backend::SpirvEmitter where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::spirv_backend::SpirvEmitter where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::spirv_backend::SpirvEmitter where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::spirv_backend::SpirvEmitter
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::spirv_backend::SpirvEmitter
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::spirv_backend::SpirvEmitter
pub fn vyre_driver_wgpu::spirv_backend::SpirvEmitter::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::spirv_backend::SpirvEmitter
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::spirv_backend::SpirvEmitter
pub type vyre_driver_wgpu::spirv_backend::SpirvEmitter::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::spirv_backend::SpirvEmitter where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::spirv_backend::SpirvEmitter where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::spirv_backend::SpirvEmitter where T: core::marker::Sync
pub const vyre_driver_wgpu::spirv_backend::SPIRV_BACKEND_ID: &str
pub struct vyre_driver_wgpu::DispatchArena
impl vyre_driver_wgpu::DispatchArena
pub fn vyre_driver_wgpu::DispatchArena::new(device: wgpu::api::device::Device, queue: wgpu::api::queue::Queue, config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> Self
impl core::clone::Clone for vyre_driver_wgpu::DispatchArena
pub fn vyre_driver_wgpu::DispatchArena::clone(&self) -> vyre_driver_wgpu::DispatchArena
impl core::fmt::Debug for vyre_driver_wgpu::DispatchArena
pub fn vyre_driver_wgpu::DispatchArena::fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::DispatchArena
impl core::marker::Send for vyre_driver_wgpu::DispatchArena
impl core::marker::Sync for vyre_driver_wgpu::DispatchArena
impl core::marker::Unpin for vyre_driver_wgpu::DispatchArena
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::DispatchArena
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::DispatchArena
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::DispatchArena
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::DispatchArena where U: core::convert::From<T>
pub fn vyre_driver_wgpu::DispatchArena::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::DispatchArena where U: core::convert::Into<T>
pub type vyre_driver_wgpu::DispatchArena::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::DispatchArena::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::DispatchArena where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::DispatchArena::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::DispatchArena::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::DispatchArena where T: core::clone::Clone
pub type vyre_driver_wgpu::DispatchArena::Owned = T
pub fn vyre_driver_wgpu::DispatchArena::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::DispatchArena::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::DispatchArena where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::DispatchArena::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::DispatchArena where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::DispatchArena::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::DispatchArena where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::DispatchArena::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::DispatchArena where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::DispatchArena::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::DispatchArena
pub fn vyre_driver_wgpu::DispatchArena::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::DispatchArena
pub fn vyre_driver_wgpu::DispatchArena::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::DispatchArena
pub fn vyre_driver_wgpu::DispatchArena::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::DispatchArena
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::DispatchArena
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::DispatchArena
pub type vyre_driver_wgpu::DispatchArena::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::DispatchArena where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::DispatchArena where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::DispatchArena where T: core::marker::Sync
pub struct vyre_driver_wgpu::WgpuBackend
impl vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::acquire() -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::acquire_adapter(index: usize) -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::adapter_info(&self) -> &wgpu_types::AdapterInfo
pub fn vyre_driver_wgpu::WgpuBackend::compile_persistent(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::sync::Arc<vyre_driver_wgpu::pipeline::WgpuPipeline>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::compile_streaming(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, config: vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<vyre_driver_wgpu::engine::streaming::HostIngressStream, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::device_limits(&self) -> &wgpu_types::Limits
pub fn vyre_driver_wgpu::WgpuBackend::device_queue(&self) -> alloc::sync::Arc<(wgpu::api::device::Device, wgpu::api::queue::Queue)>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_batch(&self, jobs: &[(vyre_foundation::ir_inner::model::program::core::Program, alloc::vec::Vec<alloc::vec::Vec<u8>>, vyre_driver::backend::dispatch_config::DispatchConfig)]) -> core::result::Result<alloc::vec::Vec<core::result::Result<vyre_driver::backend::dispatch_result::OutputBuffers, vyre_driver::backend::error::BackendError>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_borrowed_batch(&self, jobs: &[(&vyre_foundation::ir_inner::model::program::core::Program, &[&[u8]], &vyre_driver::backend::dispatch_config::DispatchConfig)]) -> core::result::Result<alloc::vec::Vec<core::result::Result<vyre_driver::backend::dispatch_result::OutputBuffers, vyre_driver::backend::error::BackendError>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_borrowed_batch_into(&self, jobs: &[(&vyre_foundation::ir_inner::model::program::core::Program, &[&[u8]], &vyre_driver::backend::dispatch_config::DispatchConfig)], outputs: &mut [vyre_driver::backend::dispatch_result::OutputBuffers]) -> core::result::Result<alloc::vec::Vec<core::result::Result<(), vyre_driver::backend::error::BackendError>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_borrowed_for_each_mapped_output<F>(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig, visitor: F) -> core::result::Result<(), vyre_driver::backend::error::BackendError> where F: core::ops::function::FnMut(usize, &[u8]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_borrowed_for_each_pod_output<T, F>(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig, visitor: F) -> core::result::Result<(), vyre_driver::backend::error::BackendError> where T: bytemuck::pod::Pod, F: core::ops::function::FnMut(usize, &[T]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_speculative_prefilter_confirm<F>(&self, speculator: &vyre_driver::speculate::AdaptiveSpeculator, plan: vyre_driver::speculate::SpeculativeDispatchPlan<'_>, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig, confirm_serial: F) -> core::result::Result<vyre_driver::speculate::SpeculativeDispatchOutcome, vyre_driver::backend::error::BackendError> where F: core::ops::function::FnMut(vyre_driver::backend::dispatch_result::OutputBuffers) -> core::result::Result<vyre_driver::backend::dispatch_result::OutputBuffers, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::force_device_lost(&self) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::invalidate_impacted_disk_cache(&self, intervention_mask: &[u32], rule_adj: &[u32], state: &[u32], join_rules: &[u32], n: u32, max_iterations: u32, pipeline_lineage_cell: &[u32], cache_keys: &[alloc::string::String]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::invalidate_impacted_pipeline_cache(&self, intervention_mask: &[u32], rule_adj: &[u32], state: &[u32], join_rules: &[u32], n: u32, max_iterations: u32, pipeline_lineage_cell: &[u32], pipeline_keys: &[[u8; 32]]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::invalidate_pipeline_cache_for_changed_op(&self, changed_op_handle: u32, pipeline_lineage_cell: &[u32], pipeline_keys: &[[u8; 32]]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::new() -> core::result::Result<Self, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::shared() -> core::result::Result<alloc::sync::Arc<Self>, vyre_driver::backend::error::BackendError>
impl vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::adapter_caps(&self) -> vyre_foundation::optimizer::ctx::AdapterCaps
pub fn vyre_driver_wgpu::WgpuBackend::device_profile(&self) -> vyre_driver::device_profile::DeviceProfile
pub fn vyre_driver_wgpu::WgpuBackend::stats(&self) -> vyre_driver_wgpu::WgpuBackendStats
impl vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::allocate_wgpu_device_buffer(&self, byte_len: usize) -> core::result::Result<alloc::boxed::Box<dyn vyre_driver::backend::device_buffer::DeviceBuffer>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::download_wgpu_device_buffer(&self, buffer: &dyn vyre_driver::backend::device_buffer::DeviceBuffer) -> core::result::Result<alloc::vec::Vec<u8>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::free_wgpu_device_buffer(&self, buffer: alloc::boxed::Box<dyn vyre_driver::backend::device_buffer::DeviceBuffer>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::upload_wgpu_device_buffer(&self, buffer: &mut dyn vyre_driver::backend::device_buffer::DeviceBuffer, bytes: &[u8]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
impl vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::compile(&self, program: &vyre_foundation::ir_inner::model::program::core::Program) -> core::result::Result<vyre_driver_wgpu::WgpuIR, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_compiled(&self, compiled: &vyre_driver_wgpu::WgpuIR, inputs: &[vyre_driver::backend::capability::MemoryRef<'_>], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<vyre_driver::backend::capability::Memory>, vyre_driver::backend::error::BackendError>
impl vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_wgsl(&self, wgsl: &str, input: &[u8], output_size: usize, workgroup_size: u32) -> core::result::Result<alloc::vec::Vec<u8>, alloc::string::String>
impl vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::lower_to_backend_ir(&self, program: &vyre_foundation::ir_inner::model::program::core::Program) -> core::result::Result<vyre_driver_wgpu::emit::WgpuProgram, vyre_foundation::lower::LoweringError>
pub fn vyre_driver_wgpu::WgpuBackend::lower_to_target<'a>(&self, bir: &'a vyre_driver_wgpu::emit::WgpuProgram) -> &'a naga::ir::Module
impl core::clone::Clone for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::clone(&self) -> vyre_driver_wgpu::WgpuBackend
impl core::fmt::Debug for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl vyre_driver::backend::capability::Executable for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::dispatch(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[vyre_driver::backend::capability::MemoryRef<'_>], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<vyre_driver::backend::capability::Memory>, vyre_driver::backend::error::BackendError>
impl vyre_driver::backend::private::Sealed for vyre_driver_wgpu::WgpuBackend
impl vyre_driver::backend::vyre_backend::VyreBackend for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::allocate_device_buffer(&self, byte_len: usize) -> core::result::Result<alloc::boxed::Box<dyn vyre_driver::backend::device_buffer::DeviceBuffer>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::allocate_resident(&self, byte_len: usize) -> core::result::Result<vyre_driver::backend::resource::Resource, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::compile_native(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<core::option::Option<alloc::sync::Arc<dyn vyre_driver::backend::compiled_pipeline::CompiledPipeline>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::device_lost(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::device_profile(&self) -> vyre_driver::device_profile::DeviceProfile
pub fn vyre_driver_wgpu::WgpuBackend::dispatch(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[alloc::vec::Vec<u8>], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_async(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[alloc::vec::Vec<u8>], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::boxed::Box<dyn vyre_driver::backend::pending_dispatch::PendingDispatch>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_borrowed(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_borrowed_async(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<alloc::boxed::Box<dyn vyre_driver::backend::pending_dispatch::PendingDispatch>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_borrowed_into(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig, outputs: &mut vyre_driver::backend::dispatch_result::OutputBuffers) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_borrowed_timed(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[&[u8]], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<vyre_driver::backend::dispatch_result::TimedDispatchResult, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_resident_timed(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, resources: &[vyre_driver::backend::resource::Resource], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<vyre_driver::backend::dispatch_result::TimedDispatchResult, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::dispatch_with_device_buffers(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[&dyn vyre_driver::backend::device_buffer::DeviceBuffer], outputs: &mut [&mut dyn vyre_driver::backend::device_buffer::DeviceBuffer], config: &vyre_driver::backend::dispatch_config::DispatchConfig) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::download_device_buffer(&self, buffer: &dyn vyre_driver::backend::device_buffer::DeviceBuffer) -> core::result::Result<alloc::vec::Vec<u8>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::download_resident(&self, resource: &vyre_driver::backend::resource::Resource) -> core::result::Result<alloc::vec::Vec<u8>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::download_resident_into(&self, resource: &vyre_driver::backend::resource::Resource, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::download_resident_range(&self, resource: &vyre_driver::backend::resource::Resource, byte_offset: usize, byte_len: usize) -> core::result::Result<alloc::vec::Vec<u8>, vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::download_resident_range_into(&self, resource: &vyre_driver::backend::resource::Resource, byte_offset: usize, byte_len: usize, out: &mut alloc::vec::Vec<u8>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::download_resident_ranges_into(&self, ranges: &[(&vyre_driver::backend::resource::Resource, usize, usize)], outputs: &mut [&mut alloc::vec::Vec<u8>]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::flush(&self) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::free_device_buffer(&self, buffer: alloc::boxed::Box<dyn vyre_driver::backend::device_buffer::DeviceBuffer>) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::free_resident(&self, resource: vyre_driver::backend::resource::Resource) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::id(&self) -> &'static str
pub fn vyre_driver_wgpu::WgpuBackend::is_distributed(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::max_compute_invocations_per_workgroup(&self) -> u32
pub fn vyre_driver_wgpu::WgpuBackend::max_compute_workgroups_per_dimension(&self) -> u32
pub fn vyre_driver_wgpu::WgpuBackend::max_storage_buffer_bytes(&self) -> u64
pub fn vyre_driver_wgpu::WgpuBackend::max_workgroup_size(&self) -> [u32; 3]
pub fn vyre_driver_wgpu::WgpuBackend::pipeline_cache_snapshot(&self) -> core::option::Option<vyre_driver::pipeline::PipelineCacheSnapshot>
pub fn vyre_driver_wgpu::WgpuBackend::subgroup_size(&self) -> core::option::Option<u32>
pub fn vyre_driver_wgpu::WgpuBackend::supported_ops(&self) -> &std::collections::hash::set::HashSet<vyre_foundation::ir_inner::model::node_kind::OpId>
pub fn vyre_driver_wgpu::WgpuBackend::supports_async_compute(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_bf16(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_f16(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_indirect_dispatch(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_persistent_thread_dispatch(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_speculation(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_subgroup_ops(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_tensor_cores(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::try_recover(&self) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::upload_device_buffer(&self, buffer: &mut dyn vyre_driver::backend::device_buffer::DeviceBuffer, bytes: &[u8]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::upload_resident(&self, resource: &vyre_driver::backend::resource::Resource, bytes: &[u8]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::upload_resident_at(&self, resource: &vyre_driver::backend::resource::Resource, dst_offset_bytes: usize, bytes: &[u8]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::upload_resident_at_many(&self, uploads: &[(&vyre_driver::backend::resource::Resource, usize, &[u8])]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::upload_resident_many(&self, uploads: &[(&vyre_driver::backend::resource::Resource, &[u8])]) -> core::result::Result<(), vyre_driver::backend::error::BackendError>
pub fn vyre_driver_wgpu::WgpuBackend::version(&self) -> &'static str
impl vyre_foundation::validate::options::BackendValidationCapabilities for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::backend_name(&self) -> &'static str
pub fn vyre_driver_wgpu::WgpuBackend::supports_cast_target(&self, target: &vyre_spec::data_type::DataType) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_distributed_collectives(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_indirect_dispatch(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_specialization_constants(&self) -> bool
pub fn vyre_driver_wgpu::WgpuBackend::supports_subgroup_ops(&self) -> bool
impl vyre_self_substrate::optimizer::dispatcher::OptimizerDispatcher for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::dispatch(&self, program: &vyre_foundation::ir_inner::model::program::core::Program, inputs: &[alloc::vec::Vec<u8>], grid_override: core::option::Option<[u32; 3]>) -> core::result::Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, vyre_self_substrate::optimizer::dispatcher::DispatchError>
impl core::marker::Freeze for vyre_driver_wgpu::WgpuBackend
impl core::marker::Send for vyre_driver_wgpu::WgpuBackend
impl core::marker::Sync for vyre_driver_wgpu::WgpuBackend
impl core::marker::Unpin for vyre_driver_wgpu::WgpuBackend
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::WgpuBackend
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::WgpuBackend
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::WgpuBackend
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::WgpuBackend where U: core::convert::From<T>
pub fn vyre_driver_wgpu::WgpuBackend::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::WgpuBackend where U: core::convert::Into<T>
pub type vyre_driver_wgpu::WgpuBackend::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::WgpuBackend::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::WgpuBackend where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::WgpuBackend::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::WgpuBackend::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::WgpuBackend where T: core::clone::Clone
pub type vyre_driver_wgpu::WgpuBackend::Owned = T
pub fn vyre_driver_wgpu::WgpuBackend::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::WgpuBackend::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::WgpuBackend where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuBackend::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::WgpuBackend where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuBackend::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::WgpuBackend where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuBackend::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::WgpuBackend where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::WgpuBackend::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::WgpuBackend
pub fn vyre_driver_wgpu::WgpuBackend::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::WgpuBackend
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::WgpuBackend
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::WgpuBackend
pub type vyre_driver_wgpu::WgpuBackend::Output = T
impl<T> vyre_driver::backend::capability::Backend for vyre_driver_wgpu::WgpuBackend where T: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuBackend::id(&self) -> &'static str
pub fn vyre_driver_wgpu::WgpuBackend::supported_ops(&self) -> &std::collections::hash::set::HashSet<alloc::sync::Arc<str>>
pub fn vyre_driver_wgpu::WgpuBackend::version(&self) -> &'static str
impl<T> vyre_driver::backend::typed_dispatch::TypedDispatchExt for vyre_driver_wgpu::WgpuBackend where T: vyre_driver::backend::vyre_backend::VyreBackend + ?core::marker::Sized
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::WgpuBackend where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::WgpuBackend where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::WgpuBackend where T: core::marker::Sync
pub struct vyre_driver_wgpu::WgpuBackendStats
pub vyre_driver_wgpu::WgpuBackendStats::adapter_name: alloc::sync::Arc<str>
pub vyre_driver_wgpu::WgpuBackendStats::persistent_pool: vyre_driver_wgpu::buffer::BufferPoolStats
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_byte_capacity: usize
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_bytes: usize
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_capacity: usize
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_entries: usize
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_evictions: u64
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_hit_rate: f64
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_hits: u64
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_insertions: u64
pub vyre_driver_wgpu::WgpuBackendStats::pipeline_cache_misses: u64
impl core::clone::Clone for vyre_driver_wgpu::WgpuBackendStats
pub fn vyre_driver_wgpu::WgpuBackendStats::clone(&self) -> vyre_driver_wgpu::WgpuBackendStats
impl core::fmt::Debug for vyre_driver_wgpu::WgpuBackendStats
pub fn vyre_driver_wgpu::WgpuBackendStats::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl core::marker::Freeze for vyre_driver_wgpu::WgpuBackendStats
impl core::marker::Send for vyre_driver_wgpu::WgpuBackendStats
impl core::marker::Sync for vyre_driver_wgpu::WgpuBackendStats
impl core::marker::Unpin for vyre_driver_wgpu::WgpuBackendStats
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::WgpuBackendStats
impl core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::WgpuBackendStats
impl core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::WgpuBackendStats
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::WgpuBackendStats where U: core::convert::From<T>
pub fn vyre_driver_wgpu::WgpuBackendStats::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::WgpuBackendStats where U: core::convert::Into<T>
pub type vyre_driver_wgpu::WgpuBackendStats::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::WgpuBackendStats::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::WgpuBackendStats where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::WgpuBackendStats::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::WgpuBackendStats::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> alloc::borrow::ToOwned for vyre_driver_wgpu::WgpuBackendStats where T: core::clone::Clone
pub type vyre_driver_wgpu::WgpuBackendStats::Owned = T
pub fn vyre_driver_wgpu::WgpuBackendStats::clone_into(&self, target: &mut T)
pub fn vyre_driver_wgpu::WgpuBackendStats::to_owned(&self) -> T
impl<T> core::any::Any for vyre_driver_wgpu::WgpuBackendStats where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuBackendStats::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::WgpuBackendStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuBackendStats::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::WgpuBackendStats where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuBackendStats::borrow_mut(&mut self) -> &mut T
impl<T> core::clone::CloneToUninit for vyre_driver_wgpu::WgpuBackendStats where T: core::clone::Clone
pub unsafe fn vyre_driver_wgpu::WgpuBackendStats::clone_to_uninit(&self, dest: *mut u8)
impl<T> core::convert::From<T> for vyre_driver_wgpu::WgpuBackendStats
pub fn vyre_driver_wgpu::WgpuBackendStats::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::WgpuBackendStats
pub fn vyre_driver_wgpu::WgpuBackendStats::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::WgpuBackendStats
pub fn vyre_driver_wgpu::WgpuBackendStats::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::WgpuBackendStats
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::WgpuBackendStats
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::WgpuBackendStats
pub type vyre_driver_wgpu::WgpuBackendStats::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::WgpuBackendStats where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::WgpuBackendStats where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::WgpuBackendStats where T: core::marker::Sync
pub struct vyre_driver_wgpu::WgpuDeviceBuffer
impl vyre_driver_wgpu::WgpuDeviceBuffer
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::handle(&self) -> &vyre_driver_wgpu::buffer::GpuBufferHandle
impl core::fmt::Debug for vyre_driver_wgpu::WgpuDeviceBuffer
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result
impl vyre_driver::backend::device_buffer::DeviceBuffer for vyre_driver_wgpu::WgpuDeviceBuffer
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::as_any(&self) -> &dyn core::any::Any
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::as_any_mut(&mut self) -> &mut dyn core::any::Any
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::backend_id(&self) -> &'static str
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::byte_len(&self) -> usize
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::debug_label(&self) -> core::option::Option<&str>
impl core::marker::Freeze for vyre_driver_wgpu::WgpuDeviceBuffer
impl core::marker::Send for vyre_driver_wgpu::WgpuDeviceBuffer
impl core::marker::Sync for vyre_driver_wgpu::WgpuDeviceBuffer
impl core::marker::Unpin for vyre_driver_wgpu::WgpuDeviceBuffer
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::WgpuDeviceBuffer
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::WgpuDeviceBuffer
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::WgpuDeviceBuffer
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::WgpuDeviceBuffer where U: core::convert::From<T>
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::WgpuDeviceBuffer where U: core::convert::Into<T>
pub type vyre_driver_wgpu::WgpuDeviceBuffer::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::WgpuDeviceBuffer where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::WgpuDeviceBuffer::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::WgpuDeviceBuffer where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::WgpuDeviceBuffer where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::WgpuDeviceBuffer where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::WgpuDeviceBuffer
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::WgpuDeviceBuffer
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::WgpuDeviceBuffer
pub fn vyre_driver_wgpu::WgpuDeviceBuffer::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::WgpuDeviceBuffer
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::WgpuDeviceBuffer
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::WgpuDeviceBuffer
pub type vyre_driver_wgpu::WgpuDeviceBuffer::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::WgpuDeviceBuffer where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::WgpuDeviceBuffer where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::WgpuDeviceBuffer where T: core::marker::Sync
pub struct vyre_driver_wgpu::WgpuIR
pub vyre_driver_wgpu::WgpuIR::pipeline: vyre_driver_wgpu::pipeline::WgpuPipeline
impl core::marker::Freeze for vyre_driver_wgpu::WgpuIR
impl core::marker::Send for vyre_driver_wgpu::WgpuIR
impl core::marker::Sync for vyre_driver_wgpu::WgpuIR
impl core::marker::Unpin for vyre_driver_wgpu::WgpuIR
impl core::marker::UnsafeUnpin for vyre_driver_wgpu::WgpuIR
impl !core::panic::unwind_safe::RefUnwindSafe for vyre_driver_wgpu::WgpuIR
impl !core::panic::unwind_safe::UnwindSafe for vyre_driver_wgpu::WgpuIR
impl<T, U> core::convert::Into<U> for vyre_driver_wgpu::WgpuIR where U: core::convert::From<T>
pub fn vyre_driver_wgpu::WgpuIR::into(self) -> U
impl<T, U> core::convert::TryFrom<U> for vyre_driver_wgpu::WgpuIR where U: core::convert::Into<T>
pub type vyre_driver_wgpu::WgpuIR::Error = core::convert::Infallible
pub fn vyre_driver_wgpu::WgpuIR::try_from(value: U) -> core::result::Result<T, <T as core::convert::TryFrom<U>>::Error>
impl<T, U> core::convert::TryInto<U> for vyre_driver_wgpu::WgpuIR where U: core::convert::TryFrom<T>
pub type vyre_driver_wgpu::WgpuIR::Error = <U as core::convert::TryFrom<T>>::Error
pub fn vyre_driver_wgpu::WgpuIR::try_into(self) -> core::result::Result<U, <U as core::convert::TryFrom<T>>::Error>
impl<T> core::any::Any for vyre_driver_wgpu::WgpuIR where T: 'static + ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuIR::type_id(&self) -> core::any::TypeId
impl<T> core::borrow::Borrow<T> for vyre_driver_wgpu::WgpuIR where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuIR::borrow(&self) -> &T
impl<T> core::borrow::BorrowMut<T> for vyre_driver_wgpu::WgpuIR where T: ?core::marker::Sized
pub fn vyre_driver_wgpu::WgpuIR::borrow_mut(&mut self) -> &mut T
impl<T> core::convert::From<T> for vyre_driver_wgpu::WgpuIR
pub fn vyre_driver_wgpu::WgpuIR::from(t: T) -> T
impl<T> khronos_egl::Downcast<T> for vyre_driver_wgpu::WgpuIR
pub fn vyre_driver_wgpu::WgpuIR::downcast(&self) -> &T
impl<T> khronos_egl::Upcast<T> for vyre_driver_wgpu::WgpuIR
pub fn vyre_driver_wgpu::WgpuIR::upcast(&self) -> core::option::Option<&T>
impl<T> tracing::instrument::Instrument for vyre_driver_wgpu::WgpuIR
impl<T> tracing::instrument::WithSubscriber for vyre_driver_wgpu::WgpuIR
impl<T> typenum::type_operators::Same for vyre_driver_wgpu::WgpuIR
pub type vyre_driver_wgpu::WgpuIR::Output = T
impl<T> wgpu_types::send_sync::WasmNotSend for vyre_driver_wgpu::WgpuIR where T: core::marker::Send
impl<T> wgpu_types::send_sync::WasmNotSendSync for vyre_driver_wgpu::WgpuIR where T: wgpu_types::send_sync::WasmNotSend + wgpu_types::send_sync::WasmNotSync
impl<T> wgpu_types::send_sync::WasmNotSync for vyre_driver_wgpu::WgpuIR where T: core::marker::Sync
pub const vyre_driver_wgpu::WGPU_BACKEND_ID: &str

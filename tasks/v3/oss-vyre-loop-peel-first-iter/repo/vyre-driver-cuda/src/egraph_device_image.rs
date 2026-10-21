//! CUDA upload planning for GPU e-graph device images.
//!
//! The e-graph substrate stays in `vyre-foundation`; this module only
//! translates its validated u32 device image into CUDA byte spans. That keeps
//! equality-saturation semantics out of the backend while giving the CUDA
//! path a single-copy upload contract.

use std::fmt;

use vyre_driver::BackendError;
use vyre_foundation::optimizer::eqsat_gpu::{
    GpuEGraphDeviceImage, GpuEGraphDeviceImageError, GpuEGraphDeviceLayout, GpuEGraphDeviceSpan,
    GpuEGraphSnapshot,
};

use crate::backend::{CudaBackend, CudaResidentBuffer};
use crate::numeric::CUDA_NUMERIC;

/// Error returned when a CUDA e-graph upload plan cannot be built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaEGraphDeviceUploadError {
    /// Foundation image packing rejected the snapshot.
    Image(GpuEGraphDeviceImageError),
    /// A word span could not be represented as byte offsets.
    ByteSizeOverflow {
        /// Segment being translated.
        context: &'static str,
        /// Word count or word offset that overflowed when scaled by four.
        words: usize,
    },
}

impl fmt::Display for CudaEGraphDeviceUploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Image(error) => error.fmt(f),
            Self::ByteSizeOverflow { context, words } => write!(
                f,
                "CUDA e-graph upload {context} word value {words} overflows byte addressing. Fix: shard the e-graph upload before staging."
            ),
        }
    }
}

impl std::error::Error for CudaEGraphDeviceUploadError {}

impl From<GpuEGraphDeviceImageError> for CudaEGraphDeviceUploadError {
    fn from(error: GpuEGraphDeviceImageError) -> Self {
        Self::Image(error)
    }
}

/// Byte span inside the single CUDA e-graph upload slab.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CudaEGraphDeviceByteSpan {
    offset: usize,
    byte_len: usize,
}

impl CudaEGraphDeviceByteSpan {
    const fn new(offset: usize, byte_len: usize) -> Self {
        Self { offset, byte_len }
    }

    /// Byte offset from the start of the CUDA upload slab.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Number of bytes in the span.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// `true` iff this span contains no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.byte_len == 0
    }
}

/// CUDA byte layout for a packed e-graph device image.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CudaEGraphDeviceByteLayout {
    row_count: usize,
    child_count: usize,
    eclass_group_count: usize,
    row_eclass_ids: CudaEGraphDeviceByteSpan,
    row_language_op_ids: CudaEGraphDeviceByteSpan,
    row_children_offsets: CudaEGraphDeviceByteSpan,
    row_children_lens: CudaEGraphDeviceByteSpan,
    row_signatures: CudaEGraphDeviceByteSpan,
    children: CudaEGraphDeviceByteSpan,
    group_eclass_ids: CudaEGraphDeviceByteSpan,
    group_offsets: CudaEGraphDeviceByteSpan,
    group_rows: CudaEGraphDeviceByteSpan,
}

impl CudaEGraphDeviceByteLayout {
    /// Number of snapshot rows in the upload image.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Number of child references in the upload image.
    #[must_use]
    pub const fn child_count(&self) -> usize {
        self.child_count
    }

    /// Number of e-class row groups in the upload image.
    #[must_use]
    pub const fn eclass_group_count(&self) -> usize {
        self.eclass_group_count
    }

    /// Byte span containing one e-class id per row.
    #[must_use]
    pub const fn row_eclass_ids(&self) -> CudaEGraphDeviceByteSpan {
        self.row_eclass_ids
    }

    /// Byte span containing one language op id per row.
    #[must_use]
    pub const fn row_language_op_ids(&self) -> CudaEGraphDeviceByteSpan {
        self.row_language_op_ids
    }

    /// Byte span containing one child-column offset per row.
    #[must_use]
    pub const fn row_children_offsets(&self) -> CudaEGraphDeviceByteSpan {
        self.row_children_offsets
    }

    /// Byte span containing one child count per row.
    #[must_use]
    pub const fn row_children_lens(&self) -> CudaEGraphDeviceByteSpan {
        self.row_children_lens
    }

    /// Byte span containing one structural signature per row.
    #[must_use]
    pub const fn row_signatures(&self) -> CudaEGraphDeviceByteSpan {
        self.row_signatures
    }

    /// Byte span containing the flat child e-class column.
    #[must_use]
    pub const fn children(&self) -> CudaEGraphDeviceByteSpan {
        self.children
    }

    /// Byte span containing sorted grouped e-class ids.
    #[must_use]
    pub const fn group_eclass_ids(&self) -> CudaEGraphDeviceByteSpan {
        self.group_eclass_ids
    }

    /// Byte span containing prefix offsets into [`Self::group_rows`].
    #[must_use]
    pub const fn group_offsets(&self) -> CudaEGraphDeviceByteSpan {
        self.group_offsets
    }

    /// Byte span containing row indices grouped by e-class.
    #[must_use]
    pub const fn group_rows(&self) -> CudaEGraphDeviceByteSpan {
        self.group_rows
    }
}

/// CUDA upload plan for a validated foundation e-graph device image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphDeviceUploadPlan {
    image: GpuEGraphDeviceImage,
    byte_layout: CudaEGraphDeviceByteLayout,
    byte_len: usize,
}

impl CudaEGraphDeviceUploadPlan {
    /// Packed u32 words to copy into CUDA-pinned staging memory.
    #[must_use]
    pub fn words(&self) -> &[u32] {
        self.image.words()
    }

    /// Foundation-owned logical image.
    #[must_use]
    pub const fn image(&self) -> &GpuEGraphDeviceImage {
        &self.image
    }

    /// CUDA byte layout for kernel parameters.
    #[must_use]
    pub const fn byte_layout(&self) -> CudaEGraphDeviceByteLayout {
        self.byte_layout
    }

    /// Total number of bytes required for the CUDA upload slab.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }
}

/// Borrowed CUDA upload plan for an already-packed foundation e-graph image.
///
/// This is the release hot path when the caller still needs to inspect the
/// packed image for launch planning after upload. It avoids cloning the full
/// packed slab just to satisfy the owned upload-plan API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphDeviceBorrowedUploadPlan<'a> {
    words: &'a [u32],
    byte_layout: CudaEGraphDeviceByteLayout,
    byte_len: usize,
}

impl<'a> CudaEGraphDeviceBorrowedUploadPlan<'a> {
    /// Packed u32 words to copy into CUDA-pinned staging memory.
    #[must_use]
    pub const fn words(&self) -> &'a [u32] {
        self.words
    }

    /// CUDA byte layout for kernel parameters.
    #[must_use]
    pub const fn byte_layout(&self) -> CudaEGraphDeviceByteLayout {
        self.byte_layout
    }

    /// Total number of bytes required for the CUDA upload slab.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }
}

/// CUDA-resident e-graph device image plus the byte layout kernels need.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaResidentEGraphDeviceImage {
    handle: CudaResidentBuffer,
    byte_layout: CudaEGraphDeviceByteLayout,
    byte_len: usize,
    word_count: usize,
}

impl CudaResidentEGraphDeviceImage {
    /// Resident CUDA buffer containing the packed u32 e-graph image.
    #[must_use]
    pub const fn handle(&self) -> CudaResidentBuffer {
        self.handle
    }

    /// CUDA byte layout for kernel parameters.
    #[must_use]
    pub const fn byte_layout(&self) -> CudaEGraphDeviceByteLayout {
        self.byte_layout
    }

    /// Total bytes uploaded to the resident image buffer.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Total u32 words uploaded to the resident image buffer.
    #[must_use]
    pub const fn word_count(&self) -> usize {
        self.word_count
    }
}

/// Checked kernel-facing pointer view of a CUDA-resident e-graph image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaEGraphDeviceKernelView {
    base_ptr: u64,
    byte_len: usize,
    row_count: usize,
    child_count: usize,
    eclass_group_count: usize,
    row_eclass_ids_ptr: u64,
    row_language_op_ids_ptr: u64,
    row_children_offsets_ptr: u64,
    row_children_lens_ptr: u64,
    row_signatures_ptr: u64,
    children_ptr: u64,
    group_eclass_ids_ptr: u64,
    group_offsets_ptr: u64,
    group_rows_ptr: u64,
}

impl CudaEGraphDeviceKernelView {
    /// Build a kernel view from a base pointer, byte length, and byte layout.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if any layout span points outside the image or
    /// if pointer arithmetic overflows.
    pub fn from_checked_parts(
        base_ptr: u64,
        byte_len: usize,
        layout: CudaEGraphDeviceByteLayout,
    ) -> Result<Self, BackendError> {
        Ok(Self {
            base_ptr,
            byte_len,
            row_count: layout.row_count(),
            child_count: layout.child_count(),
            eclass_group_count: layout.eclass_group_count(),
            row_eclass_ids_ptr: device_span_ptr(
                base_ptr,
                layout.row_eclass_ids(),
                byte_len,
                "row eclass ids",
            )?,
            row_language_op_ids_ptr: device_span_ptr(
                base_ptr,
                layout.row_language_op_ids(),
                byte_len,
                "row language op ids",
            )?,
            row_children_offsets_ptr: device_span_ptr(
                base_ptr,
                layout.row_children_offsets(),
                byte_len,
                "row child offsets",
            )?,
            row_children_lens_ptr: device_span_ptr(
                base_ptr,
                layout.row_children_lens(),
                byte_len,
                "row child lengths",
            )?,
            row_signatures_ptr: device_span_ptr(
                base_ptr,
                layout.row_signatures(),
                byte_len,
                "row signatures",
            )?,
            children_ptr: device_span_ptr(base_ptr, layout.children(), byte_len, "children")?,
            group_eclass_ids_ptr: device_span_ptr(
                base_ptr,
                layout.group_eclass_ids(),
                byte_len,
                "group eclass ids",
            )?,
            group_offsets_ptr: device_span_ptr(
                base_ptr,
                layout.group_offsets(),
                byte_len,
                "group offsets",
            )?,
            group_rows_ptr: device_span_ptr(base_ptr, layout.group_rows(), byte_len, "group rows")?,
        })
    }

    /// Base device pointer of the packed e-graph image.
    #[must_use]
    pub const fn base_ptr(&self) -> u64 {
        self.base_ptr
    }

    /// Total byte length of the resident image.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Number of e-graph rows.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.row_count
    }

    /// Number of child e-class references.
    #[must_use]
    pub const fn child_count(&self) -> usize {
        self.child_count
    }

    /// Number of grouped e-class row spans.
    #[must_use]
    pub const fn eclass_group_count(&self) -> usize {
        self.eclass_group_count
    }

    /// Device pointer to the row e-class id column.
    #[must_use]
    pub const fn row_eclass_ids_ptr(&self) -> u64 {
        self.row_eclass_ids_ptr
    }

    /// Device pointer to the row language-op id column.
    #[must_use]
    pub const fn row_language_op_ids_ptr(&self) -> u64 {
        self.row_language_op_ids_ptr
    }

    /// Device pointer to the row child-offset column.
    #[must_use]
    pub const fn row_children_offsets_ptr(&self) -> u64 {
        self.row_children_offsets_ptr
    }

    /// Device pointer to the row child-length column.
    #[must_use]
    pub const fn row_children_lens_ptr(&self) -> u64 {
        self.row_children_lens_ptr
    }

    /// Device pointer to the row structural-signature column.
    #[must_use]
    pub const fn row_signatures_ptr(&self) -> u64 {
        self.row_signatures_ptr
    }

    /// Device pointer to the flat child e-class column.
    #[must_use]
    pub const fn children_ptr(&self) -> u64 {
        self.children_ptr
    }

    /// Device pointer to sorted grouped e-class ids.
    #[must_use]
    pub const fn group_eclass_ids_ptr(&self) -> u64 {
        self.group_eclass_ids_ptr
    }

    /// Device pointer to group prefix offsets.
    #[must_use]
    pub const fn group_offsets_ptr(&self) -> u64 {
        self.group_offsets_ptr
    }

    /// Device pointer to row indices grouped by e-class.
    #[must_use]
    pub const fn group_rows_ptr(&self) -> u64 {
        self.group_rows_ptr
    }
}

impl CudaBackend {
    /// Pack and upload an e-graph snapshot into one CUDA-resident buffer.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the snapshot is malformed, the image cannot
    /// be represented as CUDA byte spans, or resident allocation/upload fails.
    pub fn upload_egraph_device_image(
        &self,
        snapshot: &GpuEGraphSnapshot,
    ) -> Result<CudaResidentEGraphDeviceImage, BackendError> {
        let plan = plan_cuda_egraph_device_upload(snapshot)
            .map_err(cuda_egraph_upload_plan_to_backend_error)?;
        self.upload_egraph_device_image_plan(plan)
    }

    /// Upload an already-planned e-graph image into one CUDA-resident buffer.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if host-byte staging, resident allocation, or
    /// resident upload fails.
    pub fn upload_egraph_device_image_plan(
        &self,
        plan: CudaEGraphDeviceUploadPlan,
    ) -> Result<CudaResidentEGraphDeviceImage, BackendError> {
        self.upload_egraph_device_image_words(plan.words(), plan.byte_layout(), plan.byte_len())
    }

    /// Upload a borrowed e-graph image plan into one CUDA-resident buffer.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if host-byte staging, resident allocation, or
    /// resident upload fails.
    pub fn upload_egraph_device_image_borrowed_plan(
        &self,
        plan: CudaEGraphDeviceBorrowedUploadPlan<'_>,
    ) -> Result<CudaResidentEGraphDeviceImage, BackendError> {
        self.upload_egraph_device_image_words(plan.words(), plan.byte_layout(), plan.byte_len())
    }

    fn upload_egraph_device_image_words(
        &self,
        words: &[u32],
        byte_layout: CudaEGraphDeviceByteLayout,
        byte_len: usize,
    ) -> Result<CudaResidentEGraphDeviceImage, BackendError> {
        let word_count = words.len();
        let handle = self.allocate_resident(byte_len)?;
        if let Err(error) = upload_egraph_words_to_resident(self, handle, words) {
            let _ = self.free_resident(handle);
            return Err(error);
        }
        Ok(CudaResidentEGraphDeviceImage {
            handle,
            byte_layout,
            byte_len,
            word_count,
        })
    }

    /// Resolve a resident e-graph image into checked kernel pointer metadata.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if the resident handle is not owned by this
    /// backend or if any byte span would point outside the resident image.
    pub fn egraph_device_kernel_view(
        &self,
        image: CudaResidentEGraphDeviceImage,
    ) -> Result<CudaEGraphDeviceKernelView, BackendError> {
        let base_ptr = self.resident_device_ptr(image.handle())?;
        CudaEGraphDeviceKernelView::from_checked_parts(
            base_ptr,
            image.byte_len(),
            image.byte_layout(),
        )
    }
}

/// Build a CUDA upload plan directly from a foundation e-graph snapshot.
///
/// # Errors
///
/// Returns [`CudaEGraphDeviceUploadError`] if the snapshot cannot be packed or
/// if the packed word spans overflow host byte addressing.

pub fn plan_cuda_egraph_device_upload(
    snapshot: &GpuEGraphSnapshot,
) -> Result<CudaEGraphDeviceUploadPlan, CudaEGraphDeviceUploadError> {
    plan_cuda_egraph_device_upload_from_image(snapshot.try_pack_device_image()?)
}

/// Build a CUDA upload plan from an already-packed foundation image.
///
/// # Errors
///
/// Returns [`CudaEGraphDeviceUploadError`] if a packed word span overflows host
/// byte addressing.
pub fn plan_cuda_egraph_device_upload_from_image(
    image: GpuEGraphDeviceImage,
) -> Result<CudaEGraphDeviceUploadPlan, CudaEGraphDeviceUploadError> {
    let borrowed = plan_cuda_egraph_device_upload_from_image_ref(&image)?;
    let byte_layout = borrowed.byte_layout();
    let byte_len = borrowed.byte_len();
    Ok(CudaEGraphDeviceUploadPlan {
        image,
        byte_layout,
        byte_len,
    })
}

/// Build a borrowed CUDA upload plan from an already-packed foundation image.
///
/// # Errors
///
/// Returns [`CudaEGraphDeviceUploadError`] if a packed word span overflows host
/// byte addressing.
pub fn plan_cuda_egraph_device_upload_from_image_ref(
    image: &GpuEGraphDeviceImage,
) -> Result<CudaEGraphDeviceBorrowedUploadPlan<'_>, CudaEGraphDeviceUploadError> {
    let layout = image.layout();
    let byte_layout = cuda_byte_layout(layout)?;
    let byte_len = checked_words_to_bytes(image.words().len(), "total upload length")?;
    Ok(CudaEGraphDeviceBorrowedUploadPlan {
        words: image.words(),
        byte_layout,
        byte_len,
    })
}

fn cuda_byte_layout(
    layout: GpuEGraphDeviceLayout,
) -> Result<CudaEGraphDeviceByteLayout, CudaEGraphDeviceUploadError> {
    Ok(CudaEGraphDeviceByteLayout {
        row_count: layout.row_count(),
        child_count: layout.child_count(),
        eclass_group_count: layout.eclass_group_count(),
        row_eclass_ids: byte_span(layout.row_eclass_ids(), "row eclass ids")?,
        row_language_op_ids: byte_span(layout.row_language_op_ids(), "row language op ids")?,
        row_children_offsets: byte_span(layout.row_children_offsets(), "row child offsets")?,
        row_children_lens: byte_span(layout.row_children_lens(), "row child lengths")?,
        row_signatures: byte_span(layout.row_signatures(), "row signatures")?,
        children: byte_span(layout.children(), "children")?,
        group_eclass_ids: byte_span(layout.group_eclass_ids(), "group eclass ids")?,
        group_offsets: byte_span(layout.group_offsets(), "group offsets")?,
        group_rows: byte_span(layout.group_rows(), "group rows")?,
    })
}

fn byte_span(
    span: GpuEGraphDeviceSpan,
    context: &'static str,
) -> Result<CudaEGraphDeviceByteSpan, CudaEGraphDeviceUploadError> {
    Ok(CudaEGraphDeviceByteSpan::new(
        checked_words_to_bytes(span.offset(), context)?,
        checked_words_to_bytes(span.len(), context)?,
    ))
}

fn checked_words_to_bytes(
    words: usize,
    context: &'static str,
) -> Result<usize, CudaEGraphDeviceUploadError> {
    words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or(CudaEGraphDeviceUploadError::ByteSizeOverflow { context, words })
}

fn upload_egraph_words_to_resident(
    backend: &CudaBackend,
    handle: CudaResidentBuffer,
    words: &[u32],
) -> Result<(), BackendError> {
    #[cfg(target_endian = "little")]
    {
        backend.upload_resident(handle, bytemuck::cast_slice(words))
    }
    #[cfg(not(target_endian = "little"))]
    {
        let bytes = egraph_words_to_le_bytes(words)?;
        backend.upload_resident(handle, &bytes)
    }
}

#[cfg(not(target_endian = "little"))]
fn egraph_words_to_le_bytes(words: &[u32]) -> Result<Vec<u8>, BackendError> {
    let byte_len = checked_words_to_bytes(words.len(), "resident egraph upload words")
        .map_err(cuda_egraph_upload_plan_to_backend_error)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(byte_len)
        .map_err(|error| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph resident upload could not reserve {byte_len} host byte(s): {error}. Shard the e-graph image before upload."
            ),
        })?;
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    Ok(bytes)
}

fn cuda_egraph_upload_plan_to_backend_error(error: CudaEGraphDeviceUploadError) -> BackendError {
    BackendError::InvalidProgram {
        fix: error.to_string(),
    }
}

fn device_span_ptr(
    base_ptr: u64,
    span: CudaEGraphDeviceByteSpan,
    image_byte_len: usize,
    context: &'static str,
) -> Result<u64, BackendError> {
    let end = span
        .offset()
        .checked_add(span.byte_len())
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph kernel view span `{context}` overflows usize. Rebuild or shard the image before launch."
            ),
        })?;
    if end > image_byte_len {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph kernel view span `{context}` points to bytes [{}..{end}) but resident image has {image_byte_len} bytes.",
                span.offset()
            ),
        });
    }
    base_ptr
        .checked_add(CUDA_NUMERIC.usize_to_u64(
            span.offset(),
            "CUDA e-graph kernel view byte offset",
        )?)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA e-graph kernel view pointer arithmetic overflowed for span `{context}` at byte offset {}.",
                span.offset()
            ),
        })
}

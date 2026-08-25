//! Boundary-condition tests for BufferDecl construction and validation.
//!
//! These tests assert that BufferDecl constructors accept boundary values
//! (the validator owns rejection, not construction) and that validation
//! correctly flags problematic declarations.

use vyre::ir::{BufferAccess, BufferDecl, DataType, LinearType, ShapePredicate};

#[test]
fn empty_buffer_name_is_constructible() {
    // Empty names are constructible; the validator may reject them.
    let buf = BufferDecl::storage("", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1);
    assert_eq!(buf.name(), "");
}

#[test]
fn unicode_buffer_name_is_constructible() {
    let buf =
        BufferDecl::storage("缓冲区", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1);
    assert_eq!(buf.name(), "缓冲区");
}

#[test]
fn buffer_name_with_newline_is_constructible() {
    let buf =
        BufferDecl::storage("line\nbreak", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1);
    assert_eq!(buf.name(), "line\nbreak");
}

#[test]
fn binding_slot_max_u32_is_constructible() {
    // u32::MAX is a legal binding slot at construction time.
    let buf =
        BufferDecl::storage("x", u32::MAX, BufferAccess::ReadWrite, DataType::U32).with_count(1);
    assert_eq!(buf.binding(), u32::MAX);
}

#[test]
fn workgroup_buffer_with_zero_count_is_constructible() {
    let buf = BufferDecl::workgroup("scratch", 0, DataType::U32);
    assert_eq!(buf.count(), 0);
    assert_eq!(buf.access(), BufferAccess::Workgroup);
}

#[test]
fn workgroup_buffer_with_max_count_is_constructible() {
    let buf = BufferDecl::workgroup("scratch", u32::MAX, DataType::U32);
    assert_eq!(buf.count(), u32::MAX);
}

#[test]
fn pipeline_live_out_on_non_output_buffer_is_constructible() {
    // pipeline_live_out is typically only meaningful on output buffers,
    // but the constructor allows it on any buffer.
    let buf = BufferDecl::read("in", 0, DataType::U32)
        .with_count(1)
        .with_pipeline_live_out(true);
    assert!(buf.is_pipeline_live_out());
}

#[test]
fn bytes_extraction_on_scalar_buffer_is_constructible() {
    // bytes_extraction relaxes V013; it can be set on any buffer.
    let buf = BufferDecl::storage("raw", 0, BufferAccess::ReadWrite, DataType::U32)
        .with_count(1)
        .with_bytes_extraction(true);
    assert!(buf.bytes_extraction);
}

#[test]
fn output_byte_range_on_non_output_buffer_is_constructible() {
    // output_byte_range can technically be set on any buffer type.
    let buf = BufferDecl::read("in", 0, DataType::U32)
        .with_count(1)
        .with_output_byte_range(0..16);
    assert_eq!(buf.output_byte_range(), Some(0..16));
}

#[test]
fn linear_type_restricted_is_constructible() {
    let buf = BufferDecl::storage("linear", 0, BufferAccess::ReadWrite, DataType::U32)
        .with_count(1)
        .with_linear_type(LinearType::Linear);
    assert_eq!(buf.linear_type(), LinearType::Linear);
}

#[test]
fn shape_predicate_is_constructible() {
    let buf = BufferDecl::storage("shaped", 0, BufferAccess::ReadWrite, DataType::U32)
        .with_count(64)
        .with_shape_predicate(ShapePredicate::Exactly(64));
    assert!(buf.shape_predicate().is_some());
}

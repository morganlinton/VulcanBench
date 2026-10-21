//! Generated coverage for fixed-width vector cast byte semantics.

use vyre::ir::{DataType, Expr, Program};
use vyre_reference::{
    execution::expr as eval_expr,
    value::Value,
    workgroup::{Invocation, InvocationIds, Memory},
};

fn eval_cast(target: DataType, source: Expr) -> Value {
    let program = Program::wrapped(Vec::new(), [1, 1, 1], Vec::new());
    let expr = Expr::Cast {
        target,
        value: Box::new(source),
    };
    eval_expr::eval(
        &expr,
        &mut Invocation::new(InvocationIds::ZERO, program.entry()),
        &mut Memory::empty(),
        &program,
    )
    .expect("Fix: generated vector cast expression must evaluate in the reference oracle.")
}

#[test]
fn generated_vector_casts_match_fixed_width_encoding_for_32768_cases() {
    let targets = [(DataType::Vec2U32, 8usize), (DataType::Vec4U32, 16usize)];
    let mut checked = 0usize;

    for seed in 0u32..4096 {
        for (target, width) in targets.iter().cloned() {
            for source_kind in 0..4 {
                let (source_expr, source_value, label) = generated_source(seed, source_kind);
                let actual = eval_cast(target.clone(), source_expr).to_bytes();
                let expected = source_value.to_bytes_width(width);
                assert_eq!(
                    actual, expected,
                    "Fix: {target:?} cast from {label} must match fixed-width Value encoding at seed {seed}."
                );
                checked += 1;
            }
        }
    }

    assert_eq!(checked, 32_768);
}

fn generated_source(seed: u32, source_kind: u32) -> (Expr, Value, &'static str) {
    match source_kind {
        0 => {
            let value = mix32(seed);
            (Expr::u32(value), Value::U32(value), "u32")
        }
        1 => {
            let value = mix32(seed ^ 0xA5A5_5A5A) as i32;
            (Expr::i32(value), Value::I32(value), "i32")
        }
        2 => {
            let value = (mix32(seed ^ 0xC001_CAFE) & 1) != 0;
            (Expr::bool(value), Value::Bool(value), "bool")
        }
        _ => {
            let value = generated_f32(seed);
            (Expr::f32(value), Value::Float(f64::from(value)), "f32")
        }
    }
}

fn generated_f32(seed: u32) -> f32 {
    let bits = mix32(seed ^ 0x9E37_79B9);
    let magnitude = ((bits & 0x7FF) as f32) / 16.0;
    if (bits & 0x800) == 0 {
        magnitude
    } else {
        -magnitude
    }
}

fn mix32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

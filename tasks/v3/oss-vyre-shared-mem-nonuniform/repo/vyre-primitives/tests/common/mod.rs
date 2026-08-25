#![allow(unused_imports, unused_macros)]

use vyre_foundation::ir::Program;
use vyre_reference::value::Value;

pub(crate) use vyre_primitives::wire::pack_u32_slice as u32_bytes;

pub(crate) fn reference_eval_idoms(
    program: &Program,
    node_count: u32,
    edge_offsets: &[u32],
    edge_targets: &[u32],
    pred_offsets: &[u32],
    pred_targets: &[u32],
) -> Vec<u32> {
    let to_bytes = |w: &[u32]| w.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();

    let values: Vec<Value> = vec![
        Value::from(to_bytes(edge_offsets)),
        Value::from(to_bytes(edge_targets)),
        Value::from(to_bytes(pred_offsets)),
        Value::from(to_bytes(pred_targets)),
        Value::from(to_bytes(&vec![0u32; node_count as usize])),
        Value::from(to_bytes(&vec![0u32; node_count as usize])),
    ];

    let outputs = vyre_reference::reference_eval(program, &values)
        .expect("dominator-tree reference program must evaluate");
    let bytes = outputs[0].to_bytes();
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().expect("u32 output chunk has four bytes")))
        .collect()
}

macro_rules! adversarial_unary_vec_cases {
    ($($name:ident: $input:expr => $expected:expr, $message:expr;)+) => {
        $(
            #[test]
            fn $name() {
                let input = $input;
                let expected = $expected;
                let actual = cpu_ref(&input);
                assert_eq!(actual, expected, "{}", $message);
            }
        )+
    };
}

macro_rules! adversarial_binary_vec_cases {
    ($($name:ident: $lhs:expr, $rhs:expr => $expected:expr, $message:expr;)+) => {
        $(
            #[test]
            fn $name() {
                let lhs = $lhs;
                let rhs = $rhs;
                let expected = $expected;
                let actual = cpu_ref(&lhs, &rhs);
                assert_eq!(actual, expected, "{}", $message);
            }
        )+
    };
}

macro_rules! adversarial_binary_vec_usize_cases {
    ($($name:ident: $lhs:expr, $rhs:expr, $len:expr => $expected:expr, $message:expr;)+) => {
        $(
            #[test]
            fn $name() {
                let lhs = $lhs;
                let rhs = $rhs;
                let len = $len;
                let expected = $expected;
                let actual = cpu_ref(&lhs, &rhs, len);
                assert_eq!(actual, expected, "{}", $message);
            }
        )+
    };
}

macro_rules! adversarial_vec_u32_cases {
    ($($name:ident: $input:expr, $param:expr => $expected:expr, $message:expr;)+) => {
        $(
            #[test]
            fn $name() {
                let input = $input;
                let param = $param;
                let expected = $expected;
                let actual = cpu_ref(&input, param);
                assert_eq!(actual, expected, "{}", $message);
            }
        )+
    };
}

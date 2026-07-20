use vyre::ir::{DataType, Program};
use vyre_libs::parsing::c::preprocess::gpu_pipeline::{
    gpu_filter_source_bytes, FilteredBytes, GpuDispatcher,
};
use vyre_reference::value::Value;

/// Reference-eval dispatcher. Each input `Vec<u8>` becomes a `Value`,
/// `reference_eval` runs the Program through the pure-Rust interpreter,
/// each output `Value` is converted back to `Vec<u8>`.
struct RefDispatcher;

impl GpuDispatcher for RefDispatcher {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        let values: Vec<Value> = inputs.iter().cloned().map(Value::from).collect();
        let outputs = vyre_reference::reference_eval(program, &values)
            .map_err(|e| format!("reference_eval: {e}"))?;
        Ok(outputs.into_iter().map(|v| v.to_bytes().to_vec()).collect())
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}

pub(crate) struct CountingDispatcher {
    pub(crate) calls: std::cell::Cell<usize>,
    pub(crate) op_ids: std::cell::RefCell<Vec<String>>,
    bytes_in_elements: std::cell::RefCell<Vec<DataType>>,
    bytes_in_input_lens: std::cell::RefCell<Vec<usize>>,
    preflight_flags: std::cell::RefCell<Vec<(String, u32, usize)>>,
}

impl CountingDispatcher {
    pub(crate) fn new() -> Self {
        Self {
            calls: std::cell::Cell::new(0),
            op_ids: std::cell::RefCell::new(Vec::new()),
            bytes_in_elements: std::cell::RefCell::new(Vec::new()),
            bytes_in_input_lens: std::cell::RefCell::new(Vec::new()),
            preflight_flags: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl GpuDispatcher for CountingDispatcher {
    fn dispatch(&self, program: &Program, inputs: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        self.calls.set(self.calls.get() + 1);
        self.op_ids.borrow_mut().push(
            program
                .entry_op_id
                .clone()
                .unwrap_or_else(|| "<anonymous>".to_string()),
        );
        self.bytes_in_elements.borrow_mut().extend(
            program
                .buffers()
                .iter()
                .filter_map(|buffer| (buffer.name() == "bytes_in").then_some(buffer.element())),
        );
        self.bytes_in_input_lens.borrow_mut().extend(
            program
                .buffers()
                .iter()
                .position(|buffer| buffer.name() == "bytes_in")
                .and_then(|index| inputs.get(index))
                .map(Vec::len),
        );
        for (index, buffer) in program.buffers().iter().enumerate() {
            let name = buffer.name();
            if name == "transform_flag" || name == "spliced_comment_flag" {
                self.preflight_flags.borrow_mut().push((
                    name.to_string(),
                    buffer.count(),
                    inputs.get(index).map_or(0, Vec::len),
                ));
            }
        }
        RefDispatcher.dispatch(program, inputs)
    }

    fn requires_output_inputs(&self) -> bool {
        true
    }
}

pub(crate) fn assert_byte_source_dispatches_use_supported_layouts(dispatcher: &CountingDispatcher) {
    let elements = dispatcher.bytes_in_elements.borrow();
    assert!(
        !elements.is_empty(),
        "filter path must dispatch at least one byte-source program"
    );
    assert!(
        elements
            .iter()
            .all(|element| matches!(element, DataType::U8 | DataType::U32)),
        "filter byte-source programs must consume raw U8 or packed U32 source buffers, got {elements:?}"
    );
}

pub(crate) fn assert_byte_source_inputs_are_unpadded(
    dispatcher: &CountingDispatcher,
    raw_len: usize,
) {
    let input_lens = dispatcher.bytes_in_input_lens.borrow();
    assert!(
        !input_lens.is_empty(),
        "filter path must dispatch at least one byte-source program"
    );
    assert!(
        input_lens
            .iter()
            .all(|len| *len == raw_len || (*len >= raw_len && *len % 4 == 0)),
        "filter byte-source programs must consume raw input length {raw_len} or padded packed-word input, got {input_lens:?}"
    );
}

pub(crate) fn assert_preflight_flags_match_declared_extent(dispatcher: &CountingDispatcher) {
    let flags = dispatcher.preflight_flags.borrow();
    assert!(
        flags.iter().any(|(name, _, _)| name == "transform_flag"),
        "filter path must dispatch the transform preflight"
    );
    for (name, count, input_len) in flags.iter() {
        let expected_len = (*count as usize) * std::mem::size_of::<u32>();
        assert!(
            *count >= 1 && *input_len == expected_len,
            "{name} must stage zeroed u32 storage matching its declared count, got count={count} input_len={input_len}"
        );
    }
}

pub(crate) fn reference_filter_source_bytes(raw: &[u8]) -> Vec<u8> {
    use vyre_libs::parsing::c::preprocess::gpu_comment_strip_mask::reference_gpu_comment_strip_mask;
    use vyre_primitives::parsing::line_splice_classify::reference_line_splice_classify;
    let splice_keep = reference_line_splice_classify(raw);
    let comment_mask = reference_gpu_comment_strip_mask(raw);
    raw.iter()
        .enumerate()
        .filter(|(i, _)| splice_keep[*i] == 1 && comment_mask[*i] != 1)
        .map(|(i, b)| if comment_mask[i] == 2 { b' ' } else { *b })
        .collect()
}

pub(crate) fn run(raw: &[u8]) -> FilteredBytes {
    gpu_filter_source_bytes(&RefDispatcher, raw).expect("gpu_filter_source_bytes")
}

pub(crate) fn generated_route_source(case: u32) -> Vec<u8> {
    match case % 8 {
        0 => format!("int keep_{case} = a / b; char c_{case} = '/';\n").into_bytes(),
        1 => format!("int x_{case} = 1; // strip line {case}\nint y_{case} = 2;\n").into_bytes(),
        2 => format!("int x_{case} = /* strip block {case} */ {case};\n").into_bytes(),
        3 => format!("int joined_{case} = {case} + \\\n{};\n", case + 1).into_bytes(),
        4 => format!("int x_{case} = 1; /\\\n/ hidden {case}\nint y_{case} = 2;\n").into_bytes(),
        5 => format!("int x_{case} = 1; /\\\n* hidden {case} */ int y_{case} = 2;\n").into_bytes(),
        6 => format!("int a_{case} = 1; // line\nint b_{case} = /* block */ 2;\n").into_bytes(),
        _ => format!("int x_{case} = 1; /* outer /* inner {case} */ int y_{case} = 2;\n")
            .into_bytes(),
    }
}

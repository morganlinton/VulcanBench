//! Live WGPU resident-output parity contracts.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};
use vyre_driver::Resource;
use vyre_driver_wgpu::WgpuBackend;

#[test]
fn resident_output_counter_matches_borrowed_dispatch() {
    let backend = WgpuBackend::acquire().expect(
        "Fix: live WGPU backend required for resident output parity; missing GPU is a configuration bug.",
    );
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("values", 1, BufferAccess::ReadOnly, DataType::U32).with_count(4),
        ],
        [4, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::and(
                    Expr::lt(Expr::var("idx"), Expr::u32(4)),
                    Expr::gt(Expr::load("values", Expr::var("idx")), Expr::u32(0)),
                ),
                vec![Node::let_bind(
                    "_slot",
                    Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                )],
            ),
        ],
    );
    let values = [1u32, 0, 7, 9]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let borrowed = backend
        .dispatch_borrowed(&program, &[values.as_slice()], &DispatchConfig::default())
        .expect("Fix: borrowed WGPU dispatch must run resident-output parity fixture.");
    assert_eq!(
        borrowed,
        vec![3u32.to_le_bytes().to_vec()],
        "Fix: WGPU borrowed dispatch must count every positive fixture value before resident parity can trust it."
    );

    let out = backend
        .allocate_resident(4)
        .expect("Fix: WGPU must allocate resident output buffer.");
    let input = backend
        .allocate_resident(values.len())
        .expect("Fix: WGPU must allocate resident input buffer.");
    backend
        .upload_resident(&out, &0u32.to_le_bytes())
        .expect("Fix: WGPU must upload resident output reset bytes.");
    backend
        .upload_resident(&input, &values)
        .expect("Fix: WGPU must upload resident input bytes.");

    let timed = backend
        .dispatch_resident_timed(
            &program,
            &[out.clone(), input.clone()],
            &DispatchConfig::default(),
        )
        .expect("Fix: WGPU resident dispatch must run resident-output parity fixture.");

    assert_eq!(
        timed.outputs, borrowed,
        "Fix: WGPU resident dispatch must bind, execute, and read back resident output buffers exactly like borrowed dispatch."
    );

    for resource in [out, input] {
        if let Resource::Resident(_) = resource {
            backend
                .free_resident(resource)
                .expect("Fix: WGPU resident parity test cleanup must free resources.");
        }
    }
}

#[test]
fn resident_metadata_counter_matches_borrowed_dispatch_at_release_scale() {
    const RECORDS: u32 = 1_048_576;

    let backend = WgpuBackend::acquire().expect(
        "Fix: live WGPU backend required for release-scale resident metadata parity; missing GPU is a configuration bug.",
    );
    let program = Program::wrapped(
        vec![
            BufferDecl::output("out_count", 0, DataType::U32).with_count(1),
            BufferDecl::storage("filesize", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(RECORDS),
            BufferDecl::storage("header", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(RECORDS),
            BufferDecl::storage("entropy_x1000", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(RECORDS),
        ],
        [256, 1, 1],
        vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(
                Expr::and(
                    Expr::lt(Expr::var("idx"), Expr::u32(RECORDS)),
                    Expr::and(
                        Expr::gt(Expr::load("filesize", Expr::var("idx")), Expr::u32(4096)),
                        Expr::and(
                            Expr::eq(Expr::load("header", Expr::var("idx")), Expr::u32(0x4550)),
                            Expr::gt(
                                Expr::load("entropy_x1000", Expr::var("idx")),
                                Expr::u32(7200),
                            ),
                        ),
                    ),
                ),
                vec![Node::let_bind(
                    "_slot",
                    Expr::atomic_add("out_count", Expr::u32(0), Expr::u32(1)),
                )],
            ),
        ],
    );
    let mut filesize = Vec::with_capacity(RECORDS as usize);
    let mut header = Vec::with_capacity(RECORDS as usize);
    let mut entropy = Vec::with_capacity(RECORDS as usize);
    let mut expected_count = 0u32;
    for index in 0..RECORDS {
        let size = 1024 + (index.wrapping_mul(13) % 131_072);
        let hdr = if index % 5 == 0 { 0x4550 } else { 0x464c_457f };
        let ent = 5000 + (index.wrapping_mul(17) % 4500);
        expected_count += u32::from(size > 4096 && hdr == 0x4550 && ent > 7200);
        filesize.push(size);
        header.push(hdr);
        entropy.push(ent);
    }
    let inputs = [pack_u32(&filesize), pack_u32(&header), pack_u32(&entropy)];
    let borrowed_refs = inputs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let mut config = DispatchConfig::default();
    config.grid_override = Some([RECORDS.div_ceil(256), 1, 1]);
    let borrowed = backend
        .dispatch_borrowed(&program, &borrowed_refs, &config)
        .expect("Fix: borrowed WGPU metadata dispatch must run.");
    assert_eq!(
        borrowed,
        vec![expected_count.to_le_bytes().to_vec()],
        "Fix: WGPU borrowed metadata dispatch must count the full release-scale fixture before resident parity can trust it."
    );

    let out = backend
        .allocate_resident(4)
        .expect("Fix: WGPU must allocate resident output buffer.");
    let filesize_res = backend
        .allocate_resident(inputs[0].len())
        .expect("Fix: WGPU must allocate resident filesize buffer.");
    let header_res = backend
        .allocate_resident(inputs[1].len())
        .expect("Fix: WGPU must allocate resident header buffer.");
    let entropy_res = backend
        .allocate_resident(inputs[2].len())
        .expect("Fix: WGPU must allocate resident entropy buffer.");
    let resources = [
        out.clone(),
        filesize_res.clone(),
        header_res.clone(),
        entropy_res.clone(),
    ];
    backend
        .upload_resident(&out, &0u32.to_le_bytes())
        .expect("Fix: WGPU must upload resident metadata output reset bytes.");
    backend
        .upload_resident(&filesize_res, &inputs[0])
        .expect("Fix: WGPU must upload resident filesize bytes.");
    backend
        .upload_resident(&header_res, &inputs[1])
        .expect("Fix: WGPU must upload resident header bytes.");
    backend
        .upload_resident(&entropy_res, &inputs[2])
        .expect("Fix: WGPU must upload resident entropy bytes.");

    let timed = backend
        .dispatch_resident_timed(&program, &resources, &config)
        .expect("Fix: WGPU resident metadata dispatch must run.");

    assert_eq!(
        timed.outputs, borrowed,
        "Fix: WGPU resident metadata dispatch must match borrowed dispatch at release scale."
    );

    for resource in resources {
        backend
            .free_resident(resource)
            .expect("Fix: WGPU resident metadata cleanup must free resources.");
    }
}

fn pack_u32(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

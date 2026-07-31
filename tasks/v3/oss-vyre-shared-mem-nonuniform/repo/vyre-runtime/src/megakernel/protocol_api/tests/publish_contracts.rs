use super::*;

#[test]
fn encode_work_items_ring_into_publishes_contiguous_slots() {
    let items = [
        MegakernelWorkItem {
            op_handle: protocol::opcode::STORE_U32,
            input_handle: 10,
            output_handle: 20,
            param: 30,
        },
        MegakernelWorkItem {
            op_handle: protocol::opcode::ATOMIC_ADD,
            input_handle: 40,
            output_handle: 50,
            param: 60,
        },
    ];
    let mut ring = vec![0xAA; 4096];

    Megakernel::encode_work_items_ring_into(4, 7, &items, &mut ring).unwrap();

    assert_eq!(read_word(&ring, 0, STATUS_WORD as usize), slot::PUBLISHED);
    assert_eq!(
        read_word(&ring, 0, OPCODE_WORD as usize),
        protocol::opcode::STORE_U32
    );
    assert_eq!(read_word(&ring, 0, TENANT_WORD as usize), 7);
    assert_eq!(
        read_word(&ring, 0, PRIORITY_WORD as usize),
        scheduler::priority::NORMAL
    );
    assert_eq!(read_word(&ring, 0, ARG0_WORD as usize), 10);
    assert_eq!(read_word(&ring, 0, ARG0_WORD as usize + 1), 20);
    assert_eq!(read_word(&ring, 0, ARG0_WORD as usize + 2), 30);
    assert_eq!(read_word(&ring, 1, STATUS_WORD as usize), slot::PUBLISHED);
    assert_eq!(
        read_word(&ring, 1, OPCODE_WORD as usize),
        protocol::opcode::ATOMIC_ADD
    );
    assert_eq!(read_word(&ring, 1, ARG0_WORD as usize), 40);
    assert_eq!(read_word(&ring, 1, ARG0_WORD as usize + 1), 50);
    assert_eq!(read_word(&ring, 1, ARG0_WORD as usize + 2), 60);
    assert_eq!(read_word(&ring, 2, STATUS_WORD as usize), slot::EMPTY);
}

#[test]
fn encode_work_items_ring_words_into_matches_byte_encoder() {
    let items = [
        MegakernelWorkItem {
            op_handle: protocol::opcode::STORE_U32,
            input_handle: 10,
            output_handle: 20,
            param: 30,
        },
        MegakernelWorkItem {
            op_handle: protocol::opcode::ATOMIC_ADD,
            input_handle: 40,
            output_handle: 50,
            param: 60,
        },
    ];
    let mut bytes = Vec::new();
    let mut words = Vec::new();

    Megakernel::encode_work_items_ring_into(4, 7, &items, &mut bytes).unwrap();
    Megakernel::encode_work_items_ring_words_into(4, 7, &items, &mut words).unwrap();

    assert_eq!(bytemuck::cast_slice::<u32, u8>(&words), bytes.as_slice());
}

#[test]
fn encode_work_items_ring_words_into_reuses_buffer_by_clearing_status_words() {
    let first = [
        MegakernelWorkItem {
            op_handle: protocol::opcode::STORE_U32,
            input_handle: 10,
            output_handle: 20,
            param: 30,
        },
        MegakernelWorkItem {
            op_handle: protocol::opcode::ATOMIC_ADD,
            input_handle: 40,
            output_handle: 50,
            param: 60,
        },
    ];
    let second = [MegakernelWorkItem {
        op_handle: protocol::opcode::STORE_U32,
        input_handle: 70,
        output_handle: 80,
        param: 90,
    }];
    let mut words = Vec::new();

    Megakernel::encode_work_items_ring_words_into(4, 7, &first, &mut words).unwrap();
    Megakernel::encode_work_items_ring_words_into(4, 7, &second, &mut words).unwrap();

    assert_eq!(
        read_word_words(&words, 0, STATUS_WORD as usize),
        slot::PUBLISHED
    );
    assert_eq!(read_word_words(&words, 0, ARG0_WORD as usize), 70);
    assert_eq!(read_word_words(&words, 0, ARG0_WORD as usize + 1), 80);
    assert_eq!(read_word_words(&words, 0, ARG0_WORD as usize + 2), 90);
    assert_eq!(
        read_word_words(&words, 1, STATUS_WORD as usize),
        slot::EMPTY
    );
    assert_eq!(
        read_word_words(&words, 2, STATUS_WORD as usize),
        slot::EMPTY
    );
    assert_eq!(
        read_word_words(&words, 3, STATUS_WORD as usize),
        slot::EMPTY
    );
}

#[test]
fn publish_work_items_updates_window_without_resetting_unrelated_slots() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    write_word(&mut ring, 0, ARG0_WORD as usize, 0xDEAD_BEEF);
    write_word(&mut ring, 3, ARG0_WORD as usize, 0xABCD_EF01);
    let items = [
        MegakernelWorkItem {
            op_handle: protocol::opcode::STORE_U32,
            input_handle: 10,
            output_handle: 20,
            param: 30,
        },
        MegakernelWorkItem {
            op_handle: protocol::opcode::ATOMIC_ADD,
            input_handle: 40,
            output_handle: 50,
            param: 60,
        },
    ];

    let published = Megakernel::publish_work_items(&mut ring, 1, 7, &items).unwrap();

    assert_eq!(published, 2);
    assert_eq!(read_word(&ring, 0, ARG0_WORD as usize), 0xDEAD_BEEF);
    assert_eq!(read_word(&ring, 3, ARG0_WORD as usize), 0xABCD_EF01);
    assert_eq!(read_word(&ring, 1, STATUS_WORD as usize), slot::PUBLISHED);
    assert_eq!(
        read_word(&ring, 1, OPCODE_WORD as usize),
        protocol::opcode::STORE_U32
    );
    assert_eq!(read_word(&ring, 1, TENANT_WORD as usize), 7);
    assert_eq!(read_word(&ring, 1, ARG0_WORD as usize), 10);
    assert_eq!(read_word(&ring, 1, ARG0_WORD as usize + 1), 20);
    assert_eq!(read_word(&ring, 1, ARG0_WORD as usize + 2), 30);
    assert_eq!(read_word(&ring, 2, STATUS_WORD as usize), slot::PUBLISHED);
    assert_eq!(
        read_word(&ring, 2, OPCODE_WORD as usize),
        protocol::opcode::ATOMIC_ADD
    );
    assert_eq!(read_word(&ring, 2, ARG0_WORD as usize), 40);
    assert_eq!(read_word(&ring, 2, ARG0_WORD as usize + 1), 50);
    assert_eq!(read_word(&ring, 2, ARG0_WORD as usize + 2), 60);
}

#[test]
fn publish_work_items_rejects_inflight_window_without_mutating() {
    let mut ring = Megakernel::encode_empty_ring(4).unwrap();
    write_word(&mut ring, 1, STATUS_WORD as usize, slot::CLAIMED);
    let before = ring.clone();
    let items = [MegakernelWorkItem {
        op_handle: protocol::opcode::STORE_U32,
        input_handle: 10,
        output_handle: 20,
        param: 30,
    }];

    let error = Megakernel::publish_work_items(&mut ring, 1, 7, &items)
        .expect_err("in-flight target slots must be rejected before mutation");

    assert!(error.to_string().contains("not publishable"));
    assert_eq!(ring, before);
}

#[test]
fn encode_work_items_ring_into_rejects_oversized_queue_without_mutating() {
    let items = [
        MegakernelWorkItem {
            op_handle: protocol::opcode::STORE_U32,
            input_handle: 1,
            output_handle: 2,
            param: 3,
        },
        MegakernelWorkItem {
            op_handle: protocol::opcode::STORE_U32,
            input_handle: 4,
            output_handle: 5,
            param: 6,
        },
    ];
    let mut ring = vec![0xAA; 8];

    let result = Megakernel::encode_work_items_ring_into(1, 0, &items, &mut ring);

    assert!(result.is_err(), "oversized queue must be rejected");
    assert_eq!(ring, vec![0xAA; 8], "rejection must not mutate ring");
}

#[test]
fn encode_work_items_ring_into_rejects_bad_opcode_without_mutating() {
    let items = [MegakernelWorkItem {
        op_handle: protocol::opcode::RESERVED_MAX_RANGE_MIN,
        input_handle: 1,
        output_handle: 2,
        param: 3,
    }];
    let mut ring = vec![0xAA; 8];

    let result = Megakernel::encode_work_items_ring_into(1, 0, &items, &mut ring);

    assert!(result.is_err(), "invalid opcode must be rejected");
    assert_eq!(ring, vec![0xAA; 8], "rejection must not mutate ring");
}

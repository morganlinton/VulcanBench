use super::*;

#[test]
fn generated_split3_preserves_tuple_columns_after_reuse() {
    for len in 0usize..4096 {
        let triples: Vec<(u32, u32, u32)> = (0..len)
            .map(|index| {
                let value = index as u32;
                (value, value.wrapping_mul(3), value.wrapping_mul(7))
            })
            .collect();
        let mut first = vec![0xAAAA_AAAAu32; 5];
        let mut second = vec![0xBBBB_BBBBu32; 7];
        let mut third = vec![0xCCCC_CCCCu32; 11];
        split_ifds_rule_triples_into(
            &triples,
            &mut first,
            &mut second,
            &mut third,
            "generated split3",
        )
        .unwrap();
        assert_eq!(first.len(), len);
        assert_eq!(second.len(), len);
        assert_eq!(third.len(), len);
        for (index, &(a, b, c)) in triples.iter().enumerate() {
            assert_eq!(first[index], a);
            assert_eq!(second[index], b);
            assert_eq!(third[index], c);
        }
    }
}

#[test]
fn generated_split4_preserves_tuple_columns_after_reuse() {
    for len in 0usize..4096 {
        let quads: Vec<(u32, u32, u32, u32)> = (0..len)
            .map(|index| {
                let value = index as u32;
                (
                    value,
                    value.wrapping_mul(5),
                    value.wrapping_mul(11),
                    value.wrapping_mul(13),
                )
            })
            .collect();
        let mut first = vec![0xAAAA_AAAAu32; 5];
        let mut second = vec![0xBBBB_BBBBu32; 7];
        let mut third = vec![0xCCCC_CCCCu32; 11];
        let mut fourth = vec![0xDDDD_DDDDu32; 13];
        split_ifds_rule_quads_into(
            &quads,
            &mut first,
            &mut second,
            &mut third,
            &mut fourth,
            "generated split4",
        )
        .unwrap();
        assert_eq!(first.len(), len);
        assert_eq!(second.len(), len);
        assert_eq!(third.len(), len);
        assert_eq!(fourth.len(), len);
        for (index, &(a, b, c, d)) in quads.iter().enumerate() {
            assert_eq!(first[index], a);
            assert_eq!(second[index], b);
            assert_eq!(third[index], c);
            assert_eq!(fourth[index], d);
        }
    }
}

#[test]
fn rule_columns_prepare_splits_all_domains_and_reuses_storage() {
    let mut columns = IfdsCsrRuleColumns::default();
    columns
        .prepare(
            &[(1, 2, 3), (4, 5, 6)],
            &[(7, 8, 9, 10)],
            &[(11, 12, 13)],
            &[(14, 15, 16), (17, 18, 19)],
        )
        .expect("Fix: IFDS rule columns should prepare");
    let capacities = [
        columns.intra_proc.capacity(),
        columns.intra_src_block.capacity(),
        columns.intra_dst_block.capacity(),
        columns.inter_src_proc.capacity(),
        columns.inter_src_block.capacity(),
        columns.inter_dst_proc.capacity(),
        columns.inter_dst_block.capacity(),
        columns.gen_proc.capacity(),
        columns.gen_block.capacity(),
        columns.gen_fact.capacity(),
        columns.kill_proc.capacity(),
        columns.kill_block.capacity(),
        columns.kill_fact.capacity(),
    ];

    assert_eq!(columns.intra_proc, [1, 4]);
    assert_eq!(columns.intra_src_block, [2, 5]);
    assert_eq!(columns.intra_dst_block, [3, 6]);
    assert_eq!(columns.inter_src_proc, [7]);
    assert_eq!(columns.inter_src_block, [8]);
    assert_eq!(columns.inter_dst_proc, [9]);
    assert_eq!(columns.inter_dst_block, [10]);
    assert_eq!(columns.gen_proc, [11]);
    assert_eq!(columns.gen_block, [12]);
    assert_eq!(columns.gen_fact, [13]);
    assert_eq!(columns.kill_proc, [14, 17]);
    assert_eq!(columns.kill_block, [15, 18]);
    assert_eq!(columns.kill_fact, [16, 19]);

    columns
        .prepare(&[(20, 21, 22)], &[], &[], &[])
        .expect("Fix: IFDS rule columns should reuse storage for smaller batches");
    assert_eq!(columns.intra_proc, [20]);
    assert_eq!(columns.intra_src_block, [21]);
    assert_eq!(columns.intra_dst_block, [22]);
    assert!(columns.inter_src_proc.is_empty());
    assert!(columns.gen_proc.is_empty());
    assert!(columns.kill_proc.is_empty());
    assert_eq!(columns.intra_proc.capacity(), capacities[0]);
    assert_eq!(columns.intra_src_block.capacity(), capacities[1]);
    assert_eq!(columns.intra_dst_block.capacity(), capacities[2]);
    assert_eq!(columns.inter_src_proc.capacity(), capacities[3]);
    assert_eq!(columns.inter_src_block.capacity(), capacities[4]);
    assert_eq!(columns.inter_dst_proc.capacity(), capacities[5]);
    assert_eq!(columns.inter_dst_block.capacity(), capacities[6]);
    assert_eq!(columns.gen_proc.capacity(), capacities[7]);
    assert_eq!(columns.gen_block.capacity(), capacities[8]);
    assert_eq!(columns.gen_fact.capacity(), capacities[9]);
    assert_eq!(columns.kill_proc.capacity(), capacities[10]);
    assert_eq!(columns.kill_block.capacity(), capacities[11]);
    assert_eq!(columns.kill_fact.capacity(), capacities[12]);
}

// pass_to_pass regression guard: pre-existing Itertools methods still work.
// Does not reference strip_prefix / StripPrefixError, so it compiles and passes
// at the base commit too.

use itertools::Itertools;

#[test]
fn vb_existing_itertools_unaffected() {
    assert_eq!((1..4).interleave(vec![10, 20, 30]).collect_vec(), vec![1, 10, 2, 20, 3, 30]);
    assert_eq!((0..5).chunks(2).into_iter().map(|c| c.collect_vec()).collect_vec().len(), 3);
    assert_eq!(vec![1, 1, 2, 3, 3].into_iter().dedup().collect_vec(), vec![1, 2, 3]);
    assert_eq!((1..=3).cartesian_product(vec!['a']).collect_vec(), vec![(1, 'a'), (2, 'a'), (3, 'a')]);
}

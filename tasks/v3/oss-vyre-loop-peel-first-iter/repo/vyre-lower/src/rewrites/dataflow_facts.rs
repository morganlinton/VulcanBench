//! Shared dataflow-fact helpers for memory and loop rewrites.

use crate::analyses::reaching_def_facts::ReachingDefFactSet;
use rustc_hash::FxHashMap;

pub(super) fn resolve_reaching_def_id(id: u32, reaching_defs: Option<&ReachingDefFactSet>) -> u32 {
    let Some(facts) = reaching_defs else {
        return id;
    };
    let mut current = id;
    let mut hops = 0usize;
    loop {
        let reaching = facts.reaching_defs(current);
        if reaching.len() != 1 || reaching[0] == current {
            return current;
        }
        current = reaching[0];
        hops += 1;
        if hops > facts.len() + 1 {
            return current;
        }
    }
}

pub(super) fn resolve_remapped_reaching_def_id(
    id: u32,
    remap: &FxHashMap<u32, u32>,
    reaching_defs: Option<&ReachingDefFactSet>,
) -> u32 {
    let mut current = id;
    let mut hops = 0usize;
    while let Some(&next) = remap.get(&current) {
        if next == current {
            break;
        }
        current = next;
        hops += 1;
        if hops > remap.len() + 1 {
            break;
        }
    }
    if let Some(facts) = reaching_defs {
        let reaching = facts.reaching_defs(current);
        if reaching.len() == 1 {
            return reaching[0];
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_reaching_def_id_follows_single_definition_chain() {
        let mut facts = ReachingDefFactSet::default();
        facts.set_reaching_defs(9, vec![7]);
        facts.set_reaching_defs(7, vec![3]);
        facts.set_reaching_defs(3, vec![3]);

        assert_eq!(resolve_reaching_def_id(9, Some(&facts)), 3);
    }

    #[test]
    fn resolve_reaching_def_id_stops_on_ambiguous_or_missing_facts() {
        let mut facts = ReachingDefFactSet::default();
        facts.set_reaching_defs(9, vec![7, 8]);

        assert_eq!(resolve_reaching_def_id(9, Some(&facts)), 9);
        assert_eq!(resolve_reaching_def_id(11, Some(&facts)), 11);
        assert_eq!(resolve_reaching_def_id(11, None), 11);
    }

    #[test]
    fn resolve_reaching_def_id_terminates_on_cycles() {
        let mut facts = ReachingDefFactSet::default();
        facts.set_reaching_defs(1, vec![2]);
        facts.set_reaching_defs(2, vec![1]);

        let resolved = resolve_reaching_def_id(1, Some(&facts));
        assert!(
            resolved == 1 || resolved == 2,
            "Fix: cyclic reaching-def chains must terminate on an observed id."
        );
    }

    #[test]
    fn resolve_remapped_reaching_def_id_applies_copy_remap_before_facts() {
        let mut remap = FxHashMap::default();
        remap.insert(10, 20);
        let mut facts = ReachingDefFactSet::default();
        facts.set_reaching_defs(20, vec![5]);

        assert_eq!(
            resolve_remapped_reaching_def_id(10, &remap, Some(&facts)),
            5
        );
    }

    #[test]
    fn resolve_remapped_reaching_def_id_tolerates_cycles_and_ambiguous_facts() {
        let mut remap = FxHashMap::default();
        remap.insert(1, 2);
        remap.insert(2, 1);
        let mut facts = ReachingDefFactSet::default();
        facts.set_reaching_defs(1, vec![7, 8]);

        let resolved = resolve_remapped_reaching_def_id(1, &remap, Some(&facts));
        assert!(
            resolved == 1 || resolved == 2,
            "Fix: cyclic copy remaps must terminate on an observed id."
        );
    }
}

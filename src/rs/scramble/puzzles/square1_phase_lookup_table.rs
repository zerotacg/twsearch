use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    time::Instant,
};

use cubing::kpuzzle::{KPattern, KPuzzle};

use crate::{
    _internal::{
        options::{Generators, MetricEnum},
        FlatMoveIndex, IndexedVec, PatternValidityChecker, SearchGenerators,
    },
    index_type,
    scramble::randomize::BasicParity,
};

use super::{mask_pattern::mask, square1::wedge_parity};

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct LookupPattern {
    masked_pattern: KPattern,
    parity: BasicParity,
}

impl Debug for LookupPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LookupPattern")
            .field("masked_pattern", &self.masked_pattern.to_data())
            .field("parity", &self.parity)
            .finish()
    }
}

impl LookupPattern {
    pub fn try_new<TPatternValidityChecker: PatternValidityChecker<KPuzzle>>(
        full_pattern: &KPattern,
        phase_mask: &KPattern,
    ) -> Option<Self> {
        let Ok(masked_pattern) = mask(full_pattern, phase_mask) else {
            panic!("Mask application failed");
        };

        if !TPatternValidityChecker::is_valid(&masked_pattern) {
            return None;
        }

        let parity = wedge_parity(full_pattern);
        Some(Self {
            masked_pattern,
            parity,
        })
    }
}

index_type!(PhasePatternIndex);

pub struct PhaseLookupTable {
    pub index_to_lookup_pattern: IndexedVec<PhasePatternIndex, LookupPattern>, // TODO: support optimizations when the size is known ahead of time
    pub lookup_pattern_to_index: HashMap<LookupPattern, PhasePatternIndex>,
    pub move_application_table:
        IndexedVec<PhasePatternIndex, IndexedVec<FlatMoveIndex, Option<PhasePatternIndex>>>,
}

impl PhaseLookupTable {
    pub fn apply_move(
        &self,
        phase_pattern_index: PhasePatternIndex,
        flat_move_index: FlatMoveIndex,
    ) -> Option<PhasePatternIndex> {
        *self
            .move_application_table
            .at(phase_pattern_index)
            .at(flat_move_index)
    }
}

pub fn build_phase_lookup_table<C: PatternValidityChecker<KPuzzle>>(
    kpuzzle: KPuzzle,
    generators: &Generators,
    phase_mask: &KPattern,
) -> (PhaseLookupTable, SearchGenerators<KPuzzle>) {
    let start_time = Instant::now();
    let random_start = false; // TODO: for scrambles, we may want this to be true
    let search_generators = SearchGenerators::try_new(
        &kpuzzle,
        generators.enumerate_moves_for_kpuzzle(&kpuzzle),
        &MetricEnum::Hand,
        random_start,
    )
    .expect("Couldn't build SearchGenerators while building PhaseLookupTable");

    // (lookup pattern, depth)
    let mut fringe = VecDeque::<(KPattern, usize)>::new();
    fringe.push_back((kpuzzle.default_pattern(), 0));

    let mut index_to_lookup_pattern = IndexedVec::<PhasePatternIndex, LookupPattern>::default();
    let mut lookup_pattern_to_index = HashMap::<LookupPattern, PhasePatternIndex>::default();
    let mut exact_prune_table = IndexedVec::<PhasePatternIndex, usize>::default();

    let mut index_to_representative_full_pattern =
        IndexedVec::<PhasePatternIndex, KPattern>::default();

    while let Some((full_pattern, depth)) = fringe.pop_front() {
        let Some(lookup_pattern) = LookupPattern::try_new::<C>(&full_pattern, phase_mask) else {
            continue;
        };

        if lookup_pattern_to_index.contains_key(&lookup_pattern) {
            // TODO: consider avoiding putting things in the fringe that are already in the fringe
            // or lookup table.
            continue;
        }

        let index = index_to_lookup_pattern.len();
        index_to_lookup_pattern.push(lookup_pattern.clone());
        lookup_pattern_to_index.insert(lookup_pattern, PhasePatternIndex(index));
        exact_prune_table.push(depth);

        for move_transformation_info in &search_generators.flat {
            fringe.push_back((
                full_pattern.apply_transformation(&move_transformation_info.transformation),
                depth + 1,
            ));
        }

        // Note that this is safe to do at the end of this loop because we use BFS rather than DFS.
        index_to_representative_full_pattern.push(full_pattern);
    }
    println!(
        "PhaseLookupTable has size {}",
        index_to_lookup_pattern.len()
    );

    let mut move_application_table: IndexedVec<
        PhasePatternIndex,
        IndexedVec<FlatMoveIndex, Option<PhasePatternIndex>>,
    > = IndexedVec::default();
    for (phase_pattern_index, _) in index_to_lookup_pattern.iter() {
        let representative = index_to_representative_full_pattern.at(phase_pattern_index);
        let mut table_row = IndexedVec::<FlatMoveIndex, Option<PhasePatternIndex>>::default();
        for move_transformation_info in &search_generators.flat {
            let new_representative =
                representative.apply_transformation(&move_transformation_info.transformation);
            let new_lookup_pattern = LookupPattern::try_new::<C>(&new_representative, phase_mask)
                .map(|new_lookup_pattern| {
                    lookup_pattern_to_index
                        .get(&new_lookup_pattern)
                        .expect("Inconsistent pattern enumeration")
                });
            table_row.push(new_lookup_pattern.copied());
        }
        move_application_table.push(table_row);
    }

    println!(
        "Built phase lookup table in: {:?}",
        Instant::now() - start_time
    );

    // dbg!(exact_prune_table);

    (
        PhaseLookupTable {
            index_to_lookup_pattern,
            lookup_pattern_to_index,
            move_application_table,
        },
        search_generators,
    )
}

#[cfg(test)]
mod tests {
    use cubing::alg::{parse_alg, parse_move};

    use super::build_phase_lookup_table;
    use crate::{
        _internal::FlatMoveIndex,
        scramble::{
            puzzles::{
                definitions::{square1_square_square_shape_kpattern, square1_unbandaged_kpuzzle},
                square1::{wedge_parity, Phase1Checker},
                square1_phase_lookup_table::{LookupPattern, PhasePatternIndex},
            },
            scramble_search::generators_from_vec_str,
        },
    };

    #[test]
    fn phase_lookup_table_test() {
        let kpuzzle = square1_unbandaged_kpuzzle();
        let generators = generators_from_vec_str(vec!["U_SQ_", "D_SQ_", "_SLASH_"]);

        let (phase_lookup_table, _search_generators) = build_phase_lookup_table::<Phase1Checker>(
            kpuzzle.clone(),
            &generators,
            &square1_square_square_shape_kpattern().to_owned(),
        );
        let cube_pattern_index = PhasePatternIndex(0);

        #[allow(non_snake_case)]
        let U_SQ_move_index = FlatMoveIndex(0);

        #[allow(non_snake_case)]
        let U_SQ_pattern_index = phase_lookup_table
            .apply_move(cube_pattern_index, U_SQ_move_index)
            .unwrap();
        dbg!(U_SQ_pattern_index);

        let lookup_pattern = phase_lookup_table
            .index_to_lookup_pattern
            .at(U_SQ_pattern_index);

        assert_eq!(
            &U_SQ_pattern_index,
            phase_lookup_table
                .lookup_pattern_to_index
                .get(lookup_pattern)
                .unwrap(),
        );

        let other_pattern = kpuzzle.default_pattern()
            .apply_move(&parse_move!("U_SQ_1"))
            .unwrap();
        let other_lookup_pattern =
            &LookupPattern::try_new::<Phase1Checker>(&other_pattern, square1_square_square_shape_kpattern())
                .unwrap();

        assert_eq!(
            &U_SQ_pattern_index,
            phase_lookup_table
                .lookup_pattern_to_index
                .get(other_lookup_pattern)
                .unwrap()
        );

        // <<< dbg!(phase_lookup_table
        // <<<     .index_to_lookup_pattern
        // <<<     .at(U_SQ_.unwrap()));
        // <<< dbg!(phase_lookup_table.apply_move(U_SQ_.unwrap(), FlatMoveIndex(10)));
        // <<< #[allow(non_snake_case)]
        // <<< let U_SQ_SLICE = phase_lookup_table
        // <<<     .index_to_lookup_pattern
        // <<<     .at(phase_lookup_table
        // <<<         .apply_move(U_SQ_.unwrap(), FlatMoveIndex(22))
        // <<<         .unwrap());
        // <<< dbg!(U_SQ_SLICE);
        // <<< dbg!(
        // <<<     U_SQ_,
        // <<<     phase_lookup_table
        // <<<         .move_application_table
        // <<<         .at(U_SQ_.unwrap())
        // <<<         .at(FlatMoveIndex(22))
        // <<< );
        // <<< dbg!(
        // <<<     U_SQ_,
        // <<<     phase_lookup_table
        // <<<         .index_to_lookup_pattern
        // <<<         .at(PhasePatternIndex(1))
        // <<< );
        // <<<
        // <<< dbg!(&search_generators.flat[10]);
        // <<< dbg!(phase_lookup_table.lookup_pattern_to_index.get(
        // <<<     &LookupPattern::try_new::<Phase1Checker>(
        // <<<         &kpuzzle.default_pattern(),
        // <<<         &square1_square_square_shape_kpattern()
        // <<<             .apply_alg(&parse_alg!("(1, 0)"))
        // <<<             .unwrap(),
        // <<<     )
        // <<<     .unwrap(),
        // <<< ));
        // <<< dbg!(phase_lookup_table.lookup_pattern_to_index.get(
        // <<<     &LookupPattern::try_new::<Phase1Checker>(
        // <<<         &kpuzzle.default_pattern(),
        // <<<         &square1_square_square_shape_kpattern()
        // <<<             .apply_alg(&parse_alg!("(4, 0)"))
        // <<<             .unwrap(),
        // <<<     )
        // <<<     .unwrap(),
        // <<< ));
        // <<< dbg!();
    }
}

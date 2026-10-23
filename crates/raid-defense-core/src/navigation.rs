//! Retained grid navigation and deterministic work counters.
//!
//! Navigation is a pure function of the grid topology (building occupancy plus
//! an optional hypothetical blocked cell) and a goal-cell set. Distance fields
//! are therefore retained per goal set and only rebuilt after a topology change
//! (any building insertion) or for a goal set that has not been requested since
//! the last change. Agents follow the retained fields with constant-time
//! next-step lookups instead of running one grid search per agent.
//!
//! The cache and the counters are not authoritative state: they never feed the
//! checksum, snapshots or equality, and every answer is identical to the
//! from-scratch breadth-first search the simulation used before (same neighbor
//! order, same tie-breaking).

use std::{
    cell::{Cell as CounterCell, RefCell},
    rc::Rc,
};

use super::*;

const UNREACHABLE_DISTANCE: u16 = u16::MAX;

/// Deterministic operation counts accumulated since the state was created.
///
/// These describe how much navigation, targeting and logistics work the
/// simulation performed. They are a pure consequence of the same inputs as the
/// game itself, but they are not part of the checksum, snapshots or gameplay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkCounters {
    /// Grid-wide navigation computations (distance/flow-field builds).
    pub flow_field_rebuilds: u64,
    /// Grid cells expanded by those computations.
    pub navigation_cells_visited: u64,
    /// Per-agent next-step lookups against retained navigation.
    pub path_follow_steps: u64,
    /// Raider candidates examined by tower targeting.
    pub target_candidate_evaluations: u64,
    /// Worker/resource/storage/site candidates examined by logistics selection.
    pub logistics_candidate_searches: u64,
}

type FieldMap = BTreeMap<Vec<Cell>, Option<DistanceField>>;

#[derive(Clone, Debug, Default)]
struct NavigationCache {
    /// Fields for the real topology, keyed by sorted goal-cell set.
    fields: FieldMap,
    /// Fields for the real topology plus one hypothetical blocked cell (used by
    /// placement validation), keyed by that cell and the goal-cell set.
    probe: Option<(Cell, FieldMap)>,
    /// Construction site -> settlement storage nodes that can reach the site,
    /// ordered by (storage-to-site distance, entity id). Settlement storage
    /// only appears through building insertion, so the ranking is a pure
    /// function of the topology and is retained with the fields.
    site_sources: BTreeMap<EntityId, Rc<[EntityId]>>,
}

/// Non-authoritative per-state instrumentation and retained navigation.
///
/// Equality deliberately ignores it so that two states with identical
/// authoritative content compare equal regardless of which queries warmed the
/// cache or how much work they performed.
#[derive(Clone, Debug, Default)]
pub(crate) struct Instrumentation {
    counters: CounterCell<WorkCounters>,
    navigation: RefCell<NavigationCache>,
}

impl PartialEq for Instrumentation {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for Instrumentation {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DistanceField {
    distances: Vec<u16>,
}

impl DistanceField {
    /// Multi-source breadth-first distances from `goals` over unblocked cells.
    fn build(
        state: &GameState,
        goals: &[Cell],
        extra_block: Option<Cell>,
        counters: &mut WorkCounters,
    ) -> Option<Self> {
        if goals.is_empty() {
            return None;
        }

        let cell_count = usize::try_from(GRID_WIDTH).ok()? * usize::try_from(GRID_HEIGHT).ok()?;
        let mut distances = vec![UNREACHABLE_DISTANCE; cell_count];
        let mut queue = VecDeque::new();

        for goal in goals {
            if state.path_cell_blocked(*goal, extra_block) {
                continue;
            }
            let index = cell_index(*goal)?;
            if distances[index] == 0 {
                continue;
            }
            distances[index] = 0;
            queue.push_back(*goal);
        }

        if queue.is_empty() {
            return None;
        }

        counters.flow_field_rebuilds += 1;
        while let Some(cell) = queue.pop_front() {
            counters.navigation_cells_visited += 1;
            let next_distance = distances[cell_index(cell)?].saturating_add(1);
            for neighbor in neighbors(cell) {
                if state.path_cell_blocked(neighbor, extra_block) {
                    continue;
                }
                let index = cell_index(neighbor)?;
                if distances[index] <= next_distance {
                    continue;
                }
                distances[index] = next_distance;
                queue.push_back(neighbor);
            }
        }

        Some(Self { distances })
    }

    pub(crate) fn distance(&self, cell: Cell) -> Option<u16> {
        let distance = *self.distances.get(cell_index(cell)?)?;
        (distance != UNREACHABLE_DISTANCE).then_some(distance)
    }

    /// Neighbor (north, east, south, west order) with the smallest distance
    /// strictly below `bound`. The first such neighbor wins ties, which matches
    /// the parent chain of a forward breadth-first search in the same order.
    fn best_neighbor_below(&self, start: Cell, bound: Option<u16>) -> Option<(u16, Cell)> {
        let mut best: Option<(u16, Cell)> = None;
        for neighbor in neighbors(start) {
            let Some(distance) = self.distance(neighbor) else {
                continue;
            };
            if bound.is_some_and(|bound| distance >= bound) {
                continue;
            }
            if best.is_none_or(|(best_distance, _)| distance < best_distance) {
                best = Some((distance, neighbor));
            }
        }
        best
    }

    /// Next step from an unblocked cell that lies on the field.
    pub(crate) fn next_step(&self, start: Cell) -> Option<Cell> {
        let current_distance = self.distance(start)?;
        if current_distance == 0 {
            return Some(start);
        }
        self.best_neighbor_below(start, Some(current_distance))
            .map(|(_, cell)| cell)
    }
}

fn goal_key(goals: &[Cell]) -> Vec<Cell> {
    let mut key = goals.to_vec();
    key.sort_unstable();
    key.dedup();
    key
}

impl GameState {
    /// Deterministic work performed by this state since it was created.
    #[must_use]
    pub fn work_counters(&self) -> WorkCounters {
        self.instrumentation.counters.get()
    }

    pub(super) fn count_work(&self, update: impl FnOnce(&mut WorkCounters)) {
        let mut counters = self.instrumentation.counters.get();
        update(&mut counters);
        self.instrumentation.counters.set(counters);
    }

    /// Discard retained navigation after the grid topology changed.
    pub(super) fn invalidate_navigation(&mut self) {
        let navigation = self.instrumentation.navigation.get_mut();
        navigation.fields.clear();
        navigation.probe = None;
        navigation.site_sources.clear();
    }

    /// Settlement storage nodes ranked as construction sources for `site`
    /// under the current topology, building the ranking on first use. Building
    /// it examines every settlement storage node once.
    pub(super) fn construction_source_ranking(&self, site: EntityId) -> Rc<[EntityId]> {
        if let Some(ranking) = self
            .instrumentation
            .navigation
            .borrow()
            .site_sources
            .get(&site)
        {
            return Rc::clone(ranking);
        }
        let ranking: Rc<[EntityId]> = self.buildings.get(entity_key(site)).map_or_else(
            || Rc::from([]),
            |building| {
                let site_goals = self.adjacent_walkable_cells(building.cell, None);
                let mut ranked = self
                    .settlement_storage_ids()
                    .into_iter()
                    .filter_map(|entity| {
                        self.count_work(|work| work.logistics_candidate_searches += 1);
                        let storage_goals = self.storage_goal_cells(entity, None);
                        let distance =
                            self.distance_between_goal_sets(&storage_goals, &site_goals)?;
                        Some((distance, entity))
                    })
                    .collect::<Vec<_>>();
                ranked.sort_unstable();
                ranked.into_iter().map(|(_, entity)| entity).collect()
            },
        );
        self.instrumentation
            .navigation
            .borrow_mut()
            .site_sources
            .insert(site, Rc::clone(&ranking));
        ranking
    }

    /// Run `query` against the retained distance field for `goals` under the
    /// current topology plus `extra_block`, building the field if needed.
    pub(super) fn with_distance_field<R>(
        &self,
        goals: &[Cell],
        extra_block: Option<Cell>,
        query: impl FnOnce(Option<&DistanceField>) -> R,
    ) -> R {
        let key = goal_key(goals);
        let cached = {
            let navigation = self.instrumentation.navigation.borrow();
            match extra_block {
                None => navigation.fields.contains_key(&key),
                Some(block) => navigation
                    .probe
                    .as_ref()
                    .is_some_and(|(cell, fields)| *cell == block && fields.contains_key(&key)),
            }
        };
        if !cached {
            let mut counters = self.instrumentation.counters.get();
            let field = DistanceField::build(self, &key, extra_block, &mut counters);
            self.instrumentation.counters.set(counters);
            let mut navigation = self.instrumentation.navigation.borrow_mut();
            let fields = match extra_block {
                None => &mut navigation.fields,
                Some(block) => {
                    if navigation
                        .probe
                        .as_ref()
                        .is_none_or(|(cell, _)| *cell != block)
                    {
                        navigation.probe = Some((block, FieldMap::new()));
                    }
                    &mut navigation
                        .probe
                        .as_mut()
                        .expect("probe fields were just ensured")
                        .1
                }
            };
            fields.insert(key.clone(), field);
        }

        let navigation = self.instrumentation.navigation.borrow();
        let field = match extra_block {
            None => navigation.fields.get(&key),
            Some(_) => navigation
                .probe
                .as_ref()
                .and_then(|(_, fields)| fields.get(&key)),
        };
        query(field.and_then(Option::as_ref))
    }

    /// First step of a shortest path from `start` to any of `goals`.
    ///
    /// Equivalent to a forward breadth-first search from `start` that expands
    /// neighbors in north, east, south, west order and follows the parent chain
    /// of the first goal reached, including when `start` itself is blocked.
    pub(super) fn next_path_step_to_any(
        &self,
        start: Cell,
        goals: &[Cell],
        extra_block: Option<Cell>,
    ) -> Option<Cell> {
        self.count_work(|work| work.path_follow_steps += 1);
        if goals.contains(&start) {
            return Some(start);
        }
        if !in_bounds(start) || goals.is_empty() {
            return None;
        }
        let start_blocked = self.path_cell_blocked(start, extra_block);
        self.with_distance_field(goals, extra_block, |field| {
            let field = field?;
            let bound = if start_blocked {
                None
            } else {
                Some(field.distance(start)?)
            };
            field
                .best_neighbor_below(start, bound)
                .map(|(_, cell)| cell)
        })
    }

    /// Shortest path length from `start` to any of `goals`.
    pub(super) fn path_distance_to_any(
        &self,
        start: Cell,
        goals: &[Cell],
        extra_block: Option<Cell>,
    ) -> Option<u16> {
        if goals.contains(&start) {
            return Some(0);
        }
        if !in_bounds(start) || goals.is_empty() {
            return None;
        }
        let start_blocked = self.path_cell_blocked(start, extra_block);
        self.with_distance_field(goals, extra_block, |field| {
            let field = field?;
            if start_blocked {
                field
                    .best_neighbor_below(start, None)
                    .map(|(distance, _)| distance.saturating_add(1))
            } else {
                field.distance(start)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The original from-scratch forward search, kept as an oracle.
    fn reference_next_step(
        state: &GameState,
        start: Cell,
        goals: &[Cell],
        extra_block: Option<Cell>,
    ) -> Option<Cell> {
        if goals.contains(&start) {
            return Some(start);
        }
        if !in_bounds(start) || goals.is_empty() {
            return None;
        }
        let cell_count = usize::try_from(GRID_WIDTH).ok()? * usize::try_from(GRID_HEIGHT).ok()?;
        let mut visited = vec![false; cell_count];
        let mut parent = vec![None; cell_count];
        let mut queue = VecDeque::new();
        visited[cell_index(start)?] = true;
        queue.push_back(start);
        let mut goal = None;
        while let Some(cell) = queue.pop_front() {
            if goals.contains(&cell) {
                goal = Some(cell);
                break;
            }
            for neighbor in neighbors(cell) {
                let index = cell_index(neighbor)?;
                if visited[index] || state.path_cell_blocked(neighbor, extra_block) {
                    continue;
                }
                visited[index] = true;
                parent[index] = Some(cell);
                queue.push_back(neighbor);
            }
        }
        let mut cursor = goal?;
        loop {
            let previous = parent[cell_index(cursor)?]?;
            if previous == start {
                return Some(cursor);
            }
            cursor = previous;
        }
    }

    fn reference_distance(
        state: &GameState,
        start: Cell,
        goals: &[Cell],
        extra_block: Option<Cell>,
    ) -> Option<u16> {
        if goals.contains(&start) {
            return Some(0);
        }
        if !in_bounds(start) || goals.is_empty() {
            return None;
        }
        let cell_count = usize::try_from(GRID_WIDTH).ok()? * usize::try_from(GRID_HEIGHT).ok()?;
        let mut visited = vec![false; cell_count];
        let mut queue = VecDeque::new();
        visited[cell_index(start)?] = true;
        queue.push_back((start, 0_u16));
        while let Some((cell, distance)) = queue.pop_front() {
            for neighbor in neighbors(cell) {
                let index = cell_index(neighbor)?;
                if visited[index] || state.path_cell_blocked(neighbor, extra_block) {
                    continue;
                }
                if goals.contains(&neighbor) {
                    return Some(distance + 1);
                }
                visited[index] = true;
                queue.push_back((neighbor, distance + 1));
            }
        }
        None
    }

    fn all_cells() -> Vec<Cell> {
        (-1..=GRID_HEIGHT)
            .flat_map(|z| (-1..=GRID_WIDTH).map(move |x| Cell::new(x, z)))
            .collect()
    }

    #[test]
    fn retained_fields_match_forward_search_for_every_start_goal_and_block() {
        for seed in [0x5eed_u64, 41] {
            let state = GameState::new(seed);
            let mut goal_sets = vec![town_goal_cells(), Vec::new()];
            for (_, building) in state.buildings.iter().take(6) {
                goal_sets.push(state.adjacent_walkable_cells(building.cell, None));
            }
            // Goal sets that include blocked cells and a lone corner.
            goal_sets.push(
                state
                    .buildings
                    .iter()
                    .map(|(_, building)| building.cell)
                    .collect(),
            );
            goal_sets.push(vec![Cell::new(0, 0)]);

            let blocks = [
                None,
                Some(Cell::new(GRID_WIDTH / 2, 3)),
                Some(Cell::new(4, 10)),
            ];
            for goals in &goal_sets {
                for extra_block in blocks {
                    for start in all_cells() {
                        assert_eq!(
                            state.next_path_step_to_any(start, goals, extra_block),
                            reference_next_step(&state, start, goals, extra_block),
                            "seed {seed} step from {start:?} to {goals:?} block {extra_block:?}"
                        );
                        assert_eq!(
                            state.path_distance_to_any(start, goals, extra_block),
                            reference_distance(&state, start, goals, extra_block),
                            "seed {seed} distance from {start:?} to {goals:?} block {extra_block:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn repeated_queries_reuse_fields_until_topology_changes() {
        let mut state = GameState::new(17);
        state.invalidate_navigation();
        let goals = town_goal_cells();
        let before = state.work_counters();
        for edge in Edge::ALL {
            state.next_path_step_to_any(edge.spawn_cell(), &goals, None);
            state.next_path_step_to_any(edge.spawn_cell(), &goals, None);
        }
        let warmed = state.work_counters();
        assert_eq!(warmed.flow_field_rebuilds - before.flow_field_rebuilds, 1);
        assert!(
            warmed.navigation_cells_visited - before.navigation_cells_visited
                <= u64::try_from(GRID_WIDTH * GRID_HEIGHT).unwrap()
        );
        assert_eq!(warmed.path_follow_steps - before.path_follow_steps, 8);

        state.invalidate_navigation();
        state.next_path_step_to_any(Edge::North.spawn_cell(), &goals, None);
        assert_eq!(
            state.work_counters().flow_field_rebuilds - warmed.flow_field_rebuilds,
            1
        );
    }
}

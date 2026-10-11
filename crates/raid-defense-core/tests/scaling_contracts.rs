//! Acceptance: deterministic action-level scaling contracts (#38).
//!
//! These tests bind targeting, navigation and command-validation work to the
//! dimension they are allowed to scale with. They use deterministic work
//! counters exposed by the authoritative Rust core (`GameState::work_counters`)
//! instead of wall-clock time. Each scaling assertion measures the same
//! operation at two sizes of one controlled dimension while every other
//! dimension is held fixed.
//!
//! Counter semantics (cumulative since the state was created, never part of
//! the authoritative checksum or gameplay outcome):
//! - `flow_field_rebuilds`: grid-wide navigation computations. Every flow
//!   field / distance field build and every from-scratch grid search (for
//!   example a BFS run for a single agent, a placement route check or a
//!   path-distance probe) counts as one rebuild, whatever it is called.
//! - `navigation_cells_visited`: grid cells expanded by those computations.
//! - `path_follow_steps`: per-agent next-step lookups against an existing
//!   navigation structure (the per-agent follow/movement work).
//! - `target_candidate_evaluations`: raider entities considered (distance
//!   tested or otherwise examined) by tower targeting.
//! - `logistics_candidate_searches`: worker/resource/storage/site candidate
//!   entities examined by logistics assignment and storage/sawmill selection.

use raid_defense_core::{
    Cell, Command, EntityKind, GRID_HEIGHT, GRID_WIDTH, GameError, GameRules, GameState,
    RaiderArchetype, STANDARD_RULES, TowerArchetype, WaveGroupRules, WaveRules, WorkCounters,
};

const GRID_CELLS: u64 = (GRID_WIDTH as u64) * (GRID_HEIGHT as u64);
const SEED: u64 = 0x38;

fn delta(after: WorkCounters, before: WorkCounters) -> WorkCounters {
    WorkCounters {
        flow_field_rebuilds: after.flow_field_rebuilds - before.flow_field_rebuilds,
        navigation_cells_visited: after.navigation_cells_visited - before.navigation_cells_visited,
        path_follow_steps: after.path_follow_steps - before.path_follow_steps,
        target_candidate_evaluations: after.target_candidate_evaluations
            - before.target_candidate_evaluations,
        logistics_candidate_searches: after.logistics_candidate_searches
            - before.logistics_candidate_searches,
    }
}

/// A raid-only profile: a single explicit wave of `raiders` basic raiders that
/// spawn one per tick and walk slowly, against a Town Hall that cannot fall.
/// The economy is paused during the raid, so workers do not move.
fn raid_rules(raiders: u16) -> GameRules {
    let mut rules = STANDARD_RULES;
    rules.economy.starting_wood = rules.economy.town_wood_capacity;
    rules.cycle.automatic_raids = false;
    rules.cycle.raid_rally_ticks = 1;
    rules.cycle.pause_economy_during_raids = true;
    rules.buildings.town_hall.max_health = u16::MAX;
    rules.raids.spawn_interval_ticks = 1;
    rules.raids.basic.speed_milli = 50;
    rules.raids.basic.damage = 1;
    rules.raids.basic.health = u16::MAX;
    rules.raids.wave_plan.wave_count = 1;
    let mut wave = WaveRules::EMPTY;
    wave.group_count = 1;
    wave.groups[0] = WaveGroupRules {
        archetype: RaiderArchetype::Basic,
        count: raiders,
    };
    rules.raids.wave_plan.waves[0] = wave;
    rules.validate().expect("raid fixture rules must validate");
    rules
}

/// Start the configured wave and advance until every raider has spawned and
/// taken its first step. Returns the state ready for a measurement window.
fn raid_in_progress(raiders: u16) -> GameState {
    let mut state = GameState::with_rules(SEED, raid_rules(raiders));
    state.apply(Command::StartWave).expect("wave should start");
    for _ in 0..(u32::from(raiders) + 4) {
        state.advance_tick();
    }
    assert_eq!(
        state.raider_count(),
        usize::from(raiders),
        "fixture: all raiders must be alive and spawned before measuring"
    );
    state
}

fn raider_cells(state: &GameState) -> Vec<Cell> {
    state
        .snapshot()
        .entities
        .into_iter()
        .filter(|entity| entity.kind == EntityKind::Raider)
        .map(|entity| entity.cell)
        .collect()
}

fn building_goal_entities(state: &GameState) -> u64 {
    state
        .snapshot()
        .entities
        .into_iter()
        .filter(|entity| {
            matches!(
                entity.kind,
                EntityKind::TownHall
                    | EntityKind::Tower
                    | EntityKind::Sawmill
                    | EntityKind::StorageHouse
                    | EntityKind::House
                    | EntityKind::Forest
            )
        })
        .count() as u64
}

// ---------------------------------------------------------------------------
// Repeated simulation tick with unchanged navigation topology (raiders)
// ---------------------------------------------------------------------------

fn raider_window(raiders: u16, ticks: u32) -> (WorkCounters, GameState) {
    let mut state = raid_in_progress(raiders);
    let before = state.work_counters();
    for _ in 0..ticks {
        state.advance_tick();
    }
    assert_eq!(
        state.raider_count(),
        usize::from(raiders),
        "fixture: no raider may leave during the measurement window"
    );
    (delta(state.work_counters(), before), state)
}

#[test]
fn unchanged_topology_raid_ticks_retain_navigation_and_rebuild_nothing() {
    // Two raider populations, identical map, goals and tick count.
    let (small, _) = raider_window(8, 80);
    let (large, _) = raider_window(32, 80);

    assert_eq!(
        small.flow_field_rebuilds, 0,
        "prepared navigation must be retained across unchanged-topology ticks (8 raiders): {small:?}"
    );
    assert_eq!(
        large.flow_field_rebuilds, 0,
        "prepared navigation must be retained across unchanged-topology ticks (32 raiders): {large:?}"
    );
    assert_eq!(small.navigation_cells_visited, 0, "{small:?}");
    assert_eq!(large.navigation_cells_visited, 0, "{large:?}");

    // Per-agent follow work exists and may grow with the population.
    assert!(small.path_follow_steps > 0, "{small:?}");
    assert!(
        large.path_follow_steps > small.path_follow_steps,
        "follow work is the per-agent part of the operation: {small:?} vs {large:?}"
    );
}

#[test]
fn unchanged_topology_worker_ticks_rebuild_at_most_once_per_navigation_goal() {
    fn worker_window(people: u16) -> (WorkCounters, u64) {
        let mut rules = STANDARD_RULES;
        rules.economy.starting_wood = rules.economy.town_wood_capacity;
        rules.cycle.automatic_raids = false;
        rules.population.base_capacity = people;
        rules.population.starting_people = people;
        rules
            .validate()
            .expect("worker fixture rules must validate");
        let mut state = GameState::with_rules(SEED, rules);

        // Topology preparation: one sawmill (instantly built) near the town.
        let sawmill = sawmill_cell(&state);
        state
            .apply(Command::PlaceSawmill {
                x: sawmill.x,
                z: sawmill.z,
            })
            .expect("fixture sawmill must be accepted");
        let goals = building_goal_entities(&state);

        let before = state.work_counters();
        for _ in 0..600 {
            state.advance_tick();
        }
        assert_eq!(
            building_goal_entities(&state),
            goals,
            "fixture: topology must stay unchanged during the window"
        );
        (delta(state.work_counters(), before), goals)
    }

    let (small, goals) = worker_window(2);
    let (large, large_goals) = worker_window(12);
    assert_eq!(goals, large_goals);

    // Navigation structures are keyed by destination, not by agent: without a
    // topology change, at most one build per navigation goal entity may occur
    // in the whole window, independent of worker count and tick count.
    for (people, work) in [(2, small), (12, large)] {
        assert!(
            work.flow_field_rebuilds <= goals,
            "{people} workers rebuilt navigation {} times for {goals} goal entities: {work:?}",
            work.flow_field_rebuilds
        );
        assert!(
            work.navigation_cells_visited <= goals * GRID_CELLS,
            "{people} workers: navigation cells exceed one grid sweep per goal: {work:?}"
        );
    }
    assert!(small.path_follow_steps > 0, "{small:?}");
    assert!(
        large.path_follow_steps > small.path_follow_steps,
        "worker follow work is reported separately and grows with active workers: {small:?} vs {large:?}"
    );
}

fn sawmill_cell(state: &GameState) -> Cell {
    let center = raid_defense_core::town_center();
    let mut cells = (1..GRID_HEIGHT - 1)
        .flat_map(|z| (1..GRID_WIDTH - 1).map(move |x| Cell::new(x, z)))
        .collect::<Vec<_>>();
    cells.sort_by_key(|cell| ((cell.x - center.x).abs() + (cell.z - center.z).abs(), *cell));
    cells
        .into_iter()
        .find(|cell| {
            let mut probe = state.clone();
            probe
                .apply(Command::PlaceSawmill {
                    x: cell.x,
                    z: cell.z,
                })
                .is_ok()
        })
        .expect("seeded map must accept a sawmill")
}

// ---------------------------------------------------------------------------
// Topology change: rebuild is legitimate, bounded by grid size, agent-independent
// ---------------------------------------------------------------------------

/// Find a free cell on the northern raider approach that currently accepts a
/// tower construction site in every given state.
fn route_blocking_cell(states: &[&GameState]) -> Cell {
    let north_x = GRID_WIDTH / 2;
    (2..GRID_HEIGHT / 2 - 2)
        .flat_map(|z| [north_x, north_x - 1, north_x + 1].map(|x| Cell::new(x, z)))
        .find(|cell| {
            states.iter().all(|state| {
                let mut probe = (*state).clone();
                !raider_cells(state).contains(cell)
                    && probe
                        .apply(Command::PlaceTower {
                            x: cell.x,
                            z: cell.z,
                            archetype: TowerArchetype::Arrow,
                        })
                        .is_ok()
            })
        })
        .expect("fixture: a route cell must accept a construction site")
}

struct TopologyChange {
    validation: WorkCounters,
    rebuild_window: WorkCounters,
}

fn topology_change(mut state: GameState, cell: Cell) -> TopologyChange {
    let before = state.work_counters();
    state
        .apply(Command::PlaceTower {
            x: cell.x,
            z: cell.z,
            archetype: TowerArchetype::Arrow,
        })
        .expect("fixture placement must be accepted");
    let after_apply = state.work_counters();

    // Behaviour: no raider ever walks into the newly blocked cell.
    for _ in 0..200 {
        state.advance_tick();
        assert!(
            !raider_cells(&state).contains(&cell),
            "raiders must route around the new construction site at {cell:?}"
        );
    }

    TopologyChange {
        validation: delta(after_apply, before),
        rebuild_window: delta(state.work_counters(), after_apply),
    }
}

#[test]
fn topology_change_rebuild_work_is_bounded_by_grid_size_and_independent_of_agents() {
    let small_state = raid_in_progress(8);
    let large_state = raid_in_progress(32);
    let cell = route_blocking_cell(&[&small_state, &large_state]);
    let small = topology_change(small_state, cell);
    let large = topology_change(large_state, cell);

    for (raiders, change) in [(8, &small), (32, &large)] {
        let rebuild = change.rebuild_window;
        assert!(
            rebuild.flow_field_rebuilds >= 1,
            "{raiders} raiders: a topology change legitimately invalidates navigation: {rebuild:?}"
        );
        assert!(
            rebuild.navigation_cells_visited <= rebuild.flow_field_rebuilds * GRID_CELLS,
            "{raiders} raiders: each declared rebuild may visit at most the grid: {rebuild:?}"
        );
        let validation = change.validation;
        assert!(
            validation.navigation_cells_visited <= validation.flow_field_rebuilds * GRID_CELLS,
            "{raiders} raiders: placement validation searches are bounded by the grid: {validation:?}"
        );
    }

    // Rebuilding after the change and validating the placement are topology
    // operations: their navigation work must not grow with raider count.
    assert_eq!(
        small.rebuild_window.flow_field_rebuilds, large.rebuild_window.flow_field_rebuilds,
        "post-change rebuilds must be per topology/goal, not per raider or per tick: \
         8 raiders {:?} vs 32 raiders {:?}",
        small.rebuild_window, large.rebuild_window
    );
    assert_eq!(
        small.validation.flow_field_rebuilds, large.validation.flow_field_rebuilds,
        "placement route validation must not run one grid search per raider: \
         8 raiders {:?} vs 32 raiders {:?}",
        small.validation, large.validation
    );
}

// ---------------------------------------------------------------------------
// Tower targeting: fixed local candidates, scaled distant raiders
// ---------------------------------------------------------------------------

/// A corner tower far from every raider approach (its local candidate set is
/// empty for the whole raid), then a raid with `raiders` distant raiders.
fn corner_tower_raid(raiders: u16, ticks: u32) -> (WorkCounters, u64) {
    let mut state = GameState::with_rules(SEED, raid_rules(raiders));
    let cell = corner_tower_cell(&state);
    state
        .apply(Command::PlaceTower {
            x: cell.x,
            z: cell.z,
            archetype: TowerArchetype::Arrow,
        })
        .expect("corner tower must be accepted");
    for _ in 0..3_000 {
        if state.tower_count() == 1 {
            break;
        }
        state.advance_tick();
    }
    assert_eq!(
        state.tower_count(),
        1,
        "fixture: corner tower must complete"
    );

    state.apply(Command::StartWave).expect("wave should start");
    let before = state.work_counters();
    let mut shots = 0_u64;
    for _ in 0..ticks {
        if let raid_defense_core::Event::TickAdvanced { shots: fired, .. } = state.advance_tick() {
            shots += u64::from(fired);
        }
    }
    assert_eq!(
        state.raider_count(),
        usize::from(raiders),
        "fixture: every raider stays alive and distant"
    );
    (delta(state.work_counters(), before), shots)
}

fn corner_tower_cell(state: &GameState) -> Cell {
    [1, 2, 3]
        .into_iter()
        .flat_map(|z| [1, 2, 3].map(move |x| Cell::new(x, z)))
        .find(|cell| {
            let mut probe = state.clone();
            probe
                .apply(Command::PlaceTower {
                    x: cell.x,
                    z: cell.z,
                    archetype: TowerArchetype::Arrow,
                })
                .is_ok()
        })
        .expect("seeded map must accept a corner tower")
}

#[test]
fn tower_targeting_work_follows_local_candidates_not_total_raiders() {
    let (small, small_shots) = corner_tower_raid(8, 120);
    let (large, large_shots) = corner_tower_raid(32, 120);
    assert_eq!(
        (small_shots, large_shots),
        (0, 0),
        "fixture: the corner tower has no local candidates in either run"
    );

    assert_eq!(
        small.target_candidate_evaluations, large.target_candidate_evaluations,
        "targeting with a fixed (empty) local candidate set must not grow with distant raiders: \
         8 raiders {small:?} vs 32 raiders {large:?}"
    );
}

// ---------------------------------------------------------------------------
// Player command validation without materialising a world copy
// ---------------------------------------------------------------------------

fn mid_game_state() -> GameState {
    let mut state = GameState::new(SEED);
    let sawmill = sawmill_cell(&state);
    state
        .apply(Command::PlaceSawmill {
            x: sawmill.x,
            z: sawmill.z,
        })
        .unwrap();
    for _ in 0..300 {
        state.advance_tick();
    }
    state
}

fn candidate_commands() -> Vec<Command> {
    let mut commands = vec![Command::StartWave, Command::AdvanceTick];
    for z in 0..GRID_HEIGHT {
        for x in 0..GRID_WIDTH {
            commands.push(Command::PlaceTower {
                x,
                z,
                archetype: TowerArchetype::Arrow,
            });
            commands.push(Command::PlaceTower {
                x,
                z,
                archetype: TowerArchetype::Cannon,
            });
            commands.push(Command::PlaceSawmill { x, z });
            commands.push(Command::PlaceStorageHouse { x, z });
            commands.push(Command::PlaceHouse { x, z });
            commands.push(Command::UpgradeTower { x, z });
        }
    }
    commands.push(Command::PlaceSawmill { x: -1, z: 0 });
    commands
}

#[test]
fn validate_command_matches_apply_without_mutating_state() {
    let state = mid_game_state();
    let snapshot = state.snapshot();
    let checksum = state.checksum();

    for command in candidate_commands() {
        let expected: Result<(), GameError> = {
            let mut probe = state.clone();
            probe.apply(command).map(|_| ())
        };
        assert_eq!(
            state.validate_command(command),
            expected,
            "validate_command must agree with apply for {command:?}"
        );
    }

    assert_eq!(
        state.snapshot(),
        snapshot,
        "validation must not mutate state"
    );
    assert_eq!(
        state.checksum(),
        checksum,
        "validation must not mutate state"
    );
}

#[test]
fn validation_probes_and_work_counters_do_not_change_authoritative_outcomes() {
    let mut probed = mid_game_state();
    let mut plain = mid_game_state();
    for command in candidate_commands() {
        let _ = probed.validate_command(command);
    }
    for _ in 0..400 {
        probed.advance_tick();
        plain.advance_tick();
    }
    assert_eq!(probed.checksum(), plain.checksum());
    assert_eq!(probed.snapshot(), plain.snapshot());
}

// ---------------------------------------------------------------------------
// Determinism and gameplay parity
// ---------------------------------------------------------------------------

#[test]
fn work_counters_are_identical_across_reruns_with_the_same_seed() {
    let run = || {
        let mut state = mid_game_state();
        state.apply(Command::StartWave).unwrap();
        for _ in 0..600 {
            state.advance_tick();
        }
        (state.work_counters(), state.checksum())
    };
    let (first, first_checksum) = run();
    let (second, second_checksum) = run();
    assert_eq!(first, second);
    assert_eq!(first_checksum, second_checksum);
    assert!(first.flow_field_rebuilds > 0, "{first:?}");
    assert!(first.path_follow_steps > 0, "{first:?}");
    assert!(first.logistics_candidate_searches > 0, "{first:?}");
}

#[test]
fn instrumentation_preserves_the_baseline_gameplay_checksum() {
    // Pinned on the pre-instrumentation baseline (origin/main 153ab0a).
    // Counters and navigation retention must not change authoritative outcomes.
    let mut state = mid_game_state();
    state.apply(Command::StartWave).unwrap();
    for _ in 0..1_500 {
        state.advance_tick();
    }
    assert_eq!(state.checksum(), BASELINE_CHECKSUM);
}

const BASELINE_CHECKSUM: u64 = 0x1dbc_58e4_a68b_0eea;

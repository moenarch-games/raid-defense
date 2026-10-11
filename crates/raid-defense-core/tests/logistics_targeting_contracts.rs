//! Acceptance: logistics population scaling bound and in-range targeting
//! contract (#42, remainder of #38).
//!
//! Like `scaling_contracts.rs`, these tests measure deterministic work counters
//! (`GameState::work_counters`) at two sizes of one controlled dimension while
//! every other dimension is held fixed, and they pin gameplay checksums so a
//! work optimisation cannot change authoritative outcomes.
//!
//! Logistics bound (declared here, enforced at two worker populations):
//!
//! ```text
//! logistics_candidate_searches <= LOGISTICS_SEARCHES_PER_ASSIGNMENT * C * assignments
//!                                 + C * ticks
//! ```
//!
//! - `C` is the relevant candidate count: forests + sawmills + settlement
//!   storage nodes (Town Hall + Storage Houses) + construction sites. People,
//!   raiders, towers and houses are not logistics candidates.
//! - `assignments` is the number of observed changes of a person's
//!   `(person_state, person_target_entity)` between consecutive ticks: every
//!   job, next-leg or return-home decision a worker actually takes.
//! - Each assignment may examine every relevant candidate a constant number of
//!   times (`LOGISTICS_SEARCHES_PER_ASSIGNMENT`), because one idle-worker
//!   decision legitimately consults sites, storage, sawmills and forests.
//! - `C * ticks` allows one population-independent sweep of the candidates per
//!   tick (a per-tick candidate index). A per-change index is also accepted.
//!
//! What the bound forbids is the current shape: every idle worker re-examining
//! every forest, sawmill and storage on every tick, i.e. work proportional to
//! idle-worker-ticks x all candidates.

use std::collections::BTreeMap;

use raid_defense_core::{
    Cell, Command, EntityId, EntityKind, Event, GRID_HEIGHT, GRID_WIDTH, GameRules, GameState,
    PersonState, RaiderArchetype, STANDARD_RULES, TowerArchetype, WaveGroupRules, WaveRules,
    WorkCounters,
};

const SEED: u64 = 0x42;

/// Each assignment may examine every relevant logistics candidate at most this
/// many times.
const LOGISTICS_SEARCHES_PER_ASSIGNMENT: u64 = 2;

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

/// Fold a value into an FNV-1a style digest (used to pin per-tick traces).
fn fold(digest: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *digest ^= u64::from(byte);
        *digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

const DIGEST_SEED: u64 = 0xcbf2_9ce4_8422_2325;

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
// Logistics: fixed resources and storage, two worker populations
// ---------------------------------------------------------------------------

const LOGISTICS_TICKS: u64 = 1_200;
const SMALL_POPULATION: u16 = 2;
const LARGE_POPULATION: u16 = 12;

/// Fixed, finite resources: every seeded forest holds two harvest batches and
/// effectively never regrows inside the window; one sawmill; Town Hall storage
/// only. The population is the only varied dimension.
fn logistics_rules(people: u16) -> GameRules {
    let mut rules = STANDARD_RULES;
    rules.cycle.automatic_raids = false;
    rules.economy.forest_tile_wood = 2 * u32::from(rules.economy.sawmill_output);
    rules.economy.forest_regrowth_interval_ticks = u16::MAX;
    rules.population.base_capacity = people;
    rules.population.starting_people = people;
    rules
        .validate()
        .expect("logistics fixture rules must validate");
    rules
}

fn logistics_candidates(state: &GameState) -> u64 {
    // The Town Hall is always a settlement storage node.
    (state.forest_count()
        + state.sawmill_count()
        + 1
        + state.storage_house_count()
        + state.construction_site_count()) as u64
}

fn person_jobs(state: &GameState) -> BTreeMap<EntityId, (PersonState, Option<EntityId>)> {
    state
        .snapshot()
        .entities
        .into_iter()
        .filter_map(|entity| {
            entity
                .person_state
                .map(|person| (entity.id, (person, entity.person_target_entity)))
        })
        .collect()
}

fn forest_stock(state: &GameState) -> u32 {
    state
        .snapshot()
        .entities
        .into_iter()
        .filter(|entity| entity.kind == EntityKind::Forest)
        .map(|entity| entity.stored_wood)
        .sum()
}

#[derive(Debug)]
struct LogisticsRun {
    people: u16,
    candidates: u64,
    assignments: u64,
    idle_worker_ticks: u64,
    work: WorkCounters,
    final_wood: u32,
    final_forest_stock: u32,
    trace_digest: u64,
    checksum: u64,
}

impl LogisticsRun {
    fn bound(&self) -> u64 {
        LOGISTICS_SEARCHES_PER_ASSIGNMENT * self.candidates * self.assignments
            + self.candidates * LOGISTICS_TICKS
    }
}

fn logistics_run(people: u16) -> LogisticsRun {
    let mut state = GameState::with_rules(SEED, logistics_rules(people));
    let sawmill = sawmill_cell(&state);
    state
        .apply(Command::PlaceSawmill {
            x: sawmill.x,
            z: sawmill.z,
        })
        .expect("fixture sawmill must be accepted");
    let candidates = logistics_candidates(&state);

    let before = state.work_counters();
    let mut previous = person_jobs(&state);
    let mut assignments = 0_u64;
    let mut idle_worker_ticks = 0_u64;
    let mut trace_digest = DIGEST_SEED;
    for _ in 0..LOGISTICS_TICKS {
        idle_worker_ticks += previous
            .values()
            .filter(|(person, _)| *person == PersonState::IdleAtTownHall)
            .count() as u64;
        state.advance_tick();
        let current = person_jobs(&state);
        assignments += current
            .iter()
            .filter(|(entity, job)| previous.get(entity) != Some(job))
            .count() as u64;
        previous = current;
        fold(&mut trace_digest, state.checksum());

        assert_eq!(
            logistics_candidates(&state),
            candidates,
            "fixture: resources and storage nodes stay fixed during the window"
        );
    }
    assert_eq!(state.people_count(), usize::from(people));

    LogisticsRun {
        people,
        candidates,
        assignments,
        idle_worker_ticks,
        work: delta(state.work_counters(), before),
        final_wood: state.wood(),
        final_forest_stock: forest_stock(&state),
        trace_digest,
        checksum: state.checksum(),
    }
}

#[test]
fn logistics_fixture_holds_resources_fixed_and_discriminates_idle_rescans() {
    let small = logistics_run(SMALL_POPULATION);
    let large = logistics_run(LARGE_POPULATION);

    assert_eq!(small.candidates, large.candidates, "{small:?} vs {large:?}");
    // Both populations work the same fixed stock to the same end state.
    assert_eq!(small.final_wood, large.final_wood, "{small:?} vs {large:?}");
    assert_eq!(
        small.final_forest_stock, large.final_forest_stock,
        "{small:?} vs {large:?}"
    );
    assert!(small.assignments > 0 && large.assignments > 0);

    // The larger population spends far more ticks idle: an implementation that
    // rescans every candidate for every idle worker on every tick cannot fit
    // inside the declared bound at that population.
    assert!(
        large.idle_worker_ticks >= 5 * small.idle_worker_ticks,
        "fixture: the large population must accumulate many idle-worker-ticks: {small:?} vs {large:?}"
    );
    assert!(
        large.idle_worker_ticks * large.candidates > 4 * large.bound(),
        "fixture: idle-worker-ticks x candidates must dwarf the declared bound: {large:?} (bound {})",
        large.bound()
    );
}

#[test]
fn logistics_searches_follow_assignments_not_idle_worker_ticks() {
    for run in [
        logistics_run(SMALL_POPULATION),
        logistics_run(LARGE_POPULATION),
    ] {
        let searches = run.work.logistics_candidate_searches;
        assert!(
            searches > 0,
            "{} workers: logistics work must still be counted: {run:?}",
            run.people
        );
        assert!(
            searches <= run.bound(),
            "{} workers: logistics_candidate_searches {searches} exceed the declared bound \
             {LOGISTICS_SEARCHES_PER_ASSIGNMENT} x {} candidates x {} assignments + {} candidates x \
             {LOGISTICS_TICKS} ticks = {} ({} idle-worker-ticks): {run:?}",
            run.people,
            run.candidates,
            run.assignments,
            run.candidates,
            run.bound(),
            run.idle_worker_ticks,
        );
    }
}

#[test]
fn logistics_fixture_preserves_baseline_gameplay() {
    // Pinned on the pre-#42 baseline (origin/main 0317fbc). A candidate index
    // must keep the same deterministic tie-breaking: every tick's checksum is
    // folded into the trace digest, so a different forest, sawmill or storage
    // choice on any tick changes it.
    let small = logistics_run(SMALL_POPULATION);
    let large = logistics_run(LARGE_POPULATION);
    assert_eq!(
        (small.checksum, small.trace_digest),
        LOGISTICS_SMALL_BASELINE,
        "{small:?}"
    );
    assert_eq!(
        (large.checksum, large.trace_digest),
        LOGISTICS_LARGE_BASELINE,
        "{large:?}"
    );
    assert_eq!(
        (small.assignments, large.assignments),
        LOGISTICS_BASELINE_ASSIGNMENTS
    );
}

const LOGISTICS_SMALL_BASELINE: (u64, u64) = (0xc833_6a55_addb_075e, 0xa646_6aec_0a7d_451b);
const LOGISTICS_LARGE_BASELINE: (u64, u64) = (0x6672_c496_476d_0ad9, 0x4893_e274_9f77_6585);
const LOGISTICS_BASELINE_ASSIGNMENTS: (u64, u64) = (320, 288);

#[test]
fn standard_profile_worker_trace_preserves_baseline_gameplay() {
    // Ordinary standard-profile economy with a larger crew, one sawmill and a
    // raid, so assignment tie-breaking under contention is pinned too.
    let mut rules = STANDARD_RULES;
    rules.population.base_capacity = LARGE_POPULATION;
    rules.population.starting_people = LARGE_POPULATION;
    rules.validate().expect("standard crew rules must validate");
    let mut state = GameState::with_rules(SEED, rules);
    let sawmill = sawmill_cell(&state);
    state
        .apply(Command::PlaceSawmill {
            x: sawmill.x,
            z: sawmill.z,
        })
        .expect("fixture sawmill must be accepted");
    let mut digest = DIGEST_SEED;
    for _ in 0..1_500 {
        state.advance_tick();
        fold(&mut digest, state.checksum());
    }
    assert_eq!((state.checksum(), digest), STANDARD_CREW_BASELINE);
}

const STANDARD_CREW_BASELINE: (u64, u64) = (0x37bf_b950_5f46_c8ff, 0xa70e_7855_261c_b8a5);

// ---------------------------------------------------------------------------
// Tower targeting: fixed NON-EMPTY local candidate set, growing distant raiders
// ---------------------------------------------------------------------------

/// The tower guards the north spawn gate.
const GUARD_TOWER: Cell = Cell::new(GRID_WIDTH / 2 + 1, 0);
const GUARD_RANGE_MILLI: i32 = 1_200;
/// Raiders spawned before the measurement window starts.
const ANCHOR_RAIDERS: u16 = 4;
const DISTANT_RAIDERS_SMALL: u16 = 0;
const DISTANT_RAIDERS_LARGE: u16 = 12;
/// Ticks after `StartWave` before measuring: every raider has spawned and every
/// fast raider that entered at the guarded gate has left the tower's cells.
const TARGETING_WARMUP_TICKS: u32 = 26;
/// Measured ticks: every fast raider is still alive and away from the tower.
const TARGETING_WINDOW_TICKS: u32 = 22;

/// One explicit wave: first `ANCHOR_RAIDERS` near-stationary basic raiders (one
/// per gate, so exactly one sits in the guard tower's range at the north gate),
/// then `distant` fast advanced raiders that walk away from the gate towards the
/// Town Hall. Raiders cannot die and the Town Hall cannot fall.
fn targeting_rules(distant: u16) -> GameRules {
    let mut rules = STANDARD_RULES;
    rules.economy.starting_wood = rules.economy.town_wood_capacity;
    rules.cycle.automatic_raids = false;
    rules.cycle.raid_rally_ticks = 1;
    rules.cycle.pause_economy_during_raids = true;
    rules.buildings.town_hall.max_health = u16::MAX;
    rules.raids.spawn_interval_ticks = 1;
    rules.raids.basic.speed_milli = 1;
    rules.raids.basic.damage = 1;
    rules.raids.basic.health = u16::MAX;
    rules.raids.advanced.speed_milli = 200;
    rules.raids.advanced.damage = 1;
    rules.raids.advanced.health = u16::MAX;
    rules.towers.arrow.levels[0].range_milli = GUARD_RANGE_MILLI;
    rules.raids.wave_plan.wave_count = 1;
    let mut wave = WaveRules::EMPTY;
    wave.groups[0] = WaveGroupRules {
        archetype: RaiderArchetype::Basic,
        count: ANCHOR_RAIDERS,
    };
    wave.group_count = 1;
    if distant > 0 {
        wave.groups[1] = WaveGroupRules {
            archetype: RaiderArchetype::Advanced,
            count: distant,
        };
        wave.group_count = 2;
    }
    rules.raids.wave_plan.waves[0] = wave;
    rules
        .validate()
        .expect("targeting fixture rules must validate");
    rules
}

fn axis_cell(milli: i64, last: i16) -> i16 {
    i16::try_from(milli.div_euclid(1_000).clamp(0, i64::from(last))).unwrap_or(last)
}

#[derive(Debug, PartialEq, Eq)]
struct TargetingTick {
    /// Raiders whose cell overlaps the tower's range square (local candidates).
    local: Vec<EntityId>,
    /// Raiders strictly within the tower's range.
    in_range: Vec<EntityId>,
    shots: u16,
    /// Targets of projectiles fired this tick.
    new_targets: Vec<EntityId>,
}

#[derive(Debug)]
struct TargetingRun {
    raiders: usize,
    work: WorkCounters,
    ticks: Vec<TargetingTick>,
    checksum: u64,
}

fn targeting_run(distant: u16) -> TargetingRun {
    let mut state = GameState::with_rules(SEED, targeting_rules(distant));
    state
        .apply(Command::PlaceTower {
            x: GUARD_TOWER.x,
            z: GUARD_TOWER.z,
            archetype: TowerArchetype::Arrow,
        })
        .expect("guard tower site must be accepted");
    for _ in 0..3_000 {
        if state.tower_count() == 1 {
            break;
        }
        state.advance_tick();
    }
    assert_eq!(state.tower_count(), 1, "fixture: guard tower must complete");
    let tower = state
        .snapshot()
        .entities
        .into_iter()
        .find(|entity| entity.kind == EntityKind::Tower)
        .expect("guard tower snapshot");
    assert_eq!(tower.cell, GUARD_TOWER);
    let range = i64::from(tower.attack_range_milli);
    assert_eq!(tower.attack_range_milli, GUARD_RANGE_MILLI);
    let (tower_x, tower_z) = (i64::from(tower.x_milli), i64::from(tower.z_milli));
    let square_x =
        axis_cell(tower_x - range, GRID_WIDTH - 1)..=axis_cell(tower_x + range, GRID_WIDTH - 1);
    let square_z =
        axis_cell(tower_z - range, GRID_HEIGHT - 1)..=axis_cell(tower_z + range, GRID_HEIGHT - 1);

    state.apply(Command::StartWave).expect("wave should start");
    for _ in 0..TARGETING_WARMUP_TICKS {
        state.advance_tick();
    }
    let total = usize::from(ANCHOR_RAIDERS + distant);
    assert_eq!(state.raider_count(), total, "fixture: all raiders spawned");

    let before = state.work_counters();
    let mut ticks = Vec::new();
    for _ in 0..TARGETING_WINDOW_TICKS {
        let previous_projectiles = projectiles(&state);
        let event = state.advance_tick();
        let Event::TickAdvanced { shots, .. } = event else {
            panic!("advance_tick must report a tick: {event:?}");
        };
        assert_eq!(
            state.raider_count(),
            total,
            "fixture: every raider stays alive during the window"
        );
        let snapshot = state.snapshot();
        let mut local = Vec::new();
        let mut in_range = Vec::new();
        for raider in snapshot
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Raider)
        {
            if square_x.contains(&raider.cell.x) && square_z.contains(&raider.cell.z) {
                local.push(raider.id);
            }
            let dx = i64::from(raider.x_milli) - tower_x;
            let dz = i64::from(raider.z_milli) - tower_z;
            if dx * dx + dz * dz <= range * range {
                in_range.push(raider.id);
            }
        }
        let new_targets = snapshot
            .entities
            .iter()
            .filter(|entity| {
                entity.kind == EntityKind::Projectile && !previous_projectiles.contains(&entity.id)
            })
            .filter_map(|entity| entity.projectile_target)
            .collect();
        ticks.push(TargetingTick {
            local,
            in_range,
            shots,
            new_targets,
        });
    }

    TargetingRun {
        raiders: total,
        work: delta(state.work_counters(), before),
        ticks,
        checksum: state.checksum(),
    }
}

fn projectiles(state: &GameState) -> Vec<EntityId> {
    state
        .snapshot()
        .entities
        .into_iter()
        .filter(|entity| entity.kind == EntityKind::Projectile)
        .map(|entity| entity.id)
        .collect()
}

#[test]
fn in_range_targeting_work_and_outcomes_ignore_distant_raiders() {
    let small = targeting_run(DISTANT_RAIDERS_SMALL);
    let large = targeting_run(DISTANT_RAIDERS_LARGE);
    assert!(large.raiders > small.raiders, "{small:?} vs {large:?}");

    // Fixture: the local candidate set is fixed, NOT empty, and identical in
    // both populations; some raider is genuinely within range.
    let anchor = &small.ticks[0].local;
    assert!(!anchor.is_empty(), "fixture: local candidates must exist");
    for run in [&small, &large] {
        for (index, tick) in run.ticks.iter().enumerate() {
            assert_eq!(
                &tick.local, anchor,
                "fixture: local candidate set must stay fixed (tick {index}, {} raiders)",
                run.raiders
            );
            assert!(
                !tick.in_range.is_empty(),
                "fixture: a raider must be within range (tick {index}, {} raiders)",
                run.raiders
            );
        }
    }

    // Outcomes: the same shots at the same targets on every tick.
    let shots = small
        .ticks
        .iter()
        .map(|tick| u64::from(tick.shots))
        .sum::<u64>();
    assert!(shots > 0, "fixture: the guard tower must fire: {small:?}");
    for (index, (a, b)) in small.ticks.iter().zip(&large.ticks).enumerate() {
        assert_eq!(
            (a.shots, &a.new_targets),
            (b.shots, &b.new_targets),
            "tick {index}: chosen targets and shots must not depend on distant raiders"
        );
        assert!(
            a.new_targets.iter().all(|target| anchor.contains(target)),
            "tick {index}: targets must come from the local candidate set"
        );
    }

    // Work: evaluations follow the fixed non-empty local set only.
    assert!(
        small.work.target_candidate_evaluations > 0,
        "in-range targeting must evaluate candidates: {:?}",
        small.work
    );
    assert_eq!(
        small.work.target_candidate_evaluations, large.work.target_candidate_evaluations,
        "target_candidate_evaluations must stay constant while distant raiders grow: \
         {} raiders {:?} vs {} raiders {:?}",
        small.raiders, small.work, large.raiders, large.work
    );
}

#[test]
fn in_range_targeting_fixture_preserves_baseline_gameplay() {
    // Pinned on the pre-#42 baseline (origin/main 0317fbc).
    assert_eq!(
        targeting_run(DISTANT_RAIDERS_SMALL).checksum,
        TARGETING_SMALL_BASELINE_CHECKSUM
    );
    assert_eq!(
        targeting_run(DISTANT_RAIDERS_LARGE).checksum,
        TARGETING_LARGE_BASELINE_CHECKSUM
    );
}

const TARGETING_SMALL_BASELINE_CHECKSUM: u64 = 0xb0ba_bb83_5370_d58d;
const TARGETING_LARGE_BASELINE_CHECKSUM: u64 = 0x9262_80e4_ab9c_d0bd;

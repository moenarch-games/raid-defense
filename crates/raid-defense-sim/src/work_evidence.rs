//! `coding-tooling/work-evidence/v1` collector for the operations declared in
//! `.performance/contract.json` (schemaVersion 2).
//!
//! Each operation measures one product operation at the contract's scale
//! points, varying a single dimension while every other input stays fixed,
//! and reports the deterministic work counters the contract bounds. The
//! fixtures mirror the repository's Rust scaling contracts
//! (`crates/raid-defense-core/tests/scaling_contracts.rs` and
//! `logistics_targeting_contracts.rs`). Wall-clock time is never reported here.
//!
//! coding-tooling checks that the emitted operations, scale points and metrics
//! match the declarations exactly; `tests/work_evidence_collector.rs` guards
//! the same match locally.

use std::fmt::Write as _;

use raid_defense_core::{
    Cell, Command, EntityKind, GRID_HEIGHT, GRID_WIDTH, GameRules, GameState, PersonState,
    RaiderArchetype, STANDARD_RULES, TowerArchetype, WaveGroupRules, WaveRules, WorkCounters,
};

use super::{Policy, simulate};

pub(crate) const SUITE: &str = "raid-defense/headless-simulation-v1";
const SEED: u64 = 0x42;
/// Ticks measured by `construction_hauling`, after the assignment tick.
const HAUL_WINDOW_TICKS: u32 = 8;

/// One reported counter value of a sample.
type Metric = (&'static str, u64);

struct Operation {
    id: &'static str,
    /// Dimension names; each scale point lists one value per dimension.
    dimensions: &'static [&'static str],
    points: &'static [&'static [u64]],
    measure: fn(&[u64]) -> Vec<Metric>,
}

/// The declared operations and their scale points. Keep in sync with the
/// `operations` block of `.performance/contract.json`.
const OPERATIONS: [Operation; 6] = [
    Operation {
        id: "raid-tick-navigation",
        dimensions: &["raiders"],
        points: &[&[8], &[16], &[32]],
        measure: |point| raid_tick_navigation(point[0]),
    },
    Operation {
        id: "tower-targeting",
        dimensions: &["raiders"],
        points: &[&[4], &[10], &[16]],
        measure: |point| tower_targeting(point[0]),
    },
    Operation {
        id: "worker-logistics",
        dimensions: &["workers"],
        points: &[&[2], &[12]],
        measure: |point| worker_logistics(point[0]),
    },
    Operation {
        id: "idle-workers-exhausted-stock",
        dimensions: &["workers"],
        points: &[&[2], &[4], &[8], &[12]],
        measure: |point| idle_workers_exhausted_stock(point[0]),
    },
    Operation {
        id: "construction-hauling",
        dimensions: &["haulers", "storages"],
        points: &[&[2, 1], &[8, 1], &[2, 4], &[8, 4]],
        measure: |point| construction_hauling(point[0], point[1]),
    },
    Operation {
        id: "policy-journey",
        dimensions: &["ticks"],
        points: &[&[1_000], &[2_000], &[4_000]],
        measure: |point| policy_journey(point[0]),
    },
];

/// Render the work evidence for `contract_sha256` (the sha256 of the exact
/// contract bytes, computed by the caller).
pub(crate) fn render(contract_sha256: &str) -> Result<String, String> {
    if contract_sha256.len() != 64
        || !contract_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "--contract-sha256 must be 64 lowercase hex digits, got {contract_sha256:?}"
        ));
    }
    let mut out = String::new();
    write!(
        out,
        "{{\"schemaVersion\":\"coding-tooling/work-evidence/v1\",\"suite\":\"{SUITE}\",\"contractSha256\":\"{contract_sha256}\",\"operations\":["
    )
    .expect("writing to a String cannot fail");
    for (index, operation) in OPERATIONS.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write!(out, "{{\"id\":\"{}\",\"samples\":[", operation.id)
            .expect("writing to a String cannot fail");
        for (point_index, point) in operation.points.iter().enumerate() {
            if point_index > 0 {
                out.push(',');
            }
            out.push_str("{\"dimensions\":{");
            for (dimension_index, (name, value)) in
                operation.dimensions.iter().zip(point.iter()).enumerate()
            {
                if dimension_index > 0 {
                    out.push(',');
                }
                write!(out, "\"{name}\":{value}").expect("writing to a String cannot fail");
            }
            out.push_str("},\"metrics\":{");
            for (metric_index, (name, value)) in (operation.measure)(point).iter().enumerate() {
                if metric_index > 0 {
                    out.push(',');
                }
                write!(out, "\"{name}\":{value}").expect("writing to a String cannot fail");
            }
            out.push_str("}}");
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    Ok(out)
}

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

fn count(value: u64) -> u16 {
    u16::try_from(value).expect("scale point fits the fixture population")
}

fn single_wave(groups: &[(RaiderArchetype, u16)]) -> WaveRules {
    let mut wave = WaveRules::EMPTY;
    for (index, (archetype, count)) in groups.iter().enumerate() {
        wave.groups[index] = WaveGroupRules {
            archetype: *archetype,
            count: *count,
        };
    }
    wave.group_count = u8::try_from(groups.len()).expect("wave group count fits u8");
    wave
}

/// A raid-only profile in which raiders cannot die, the Town Hall cannot fall
/// and the economy pauses, so the measured window only advances raiders.
fn raid_only_rules() -> GameRules {
    let mut rules = STANDARD_RULES;
    rules.economy.starting_wood = rules.economy.town_wood_capacity;
    rules.cycle.automatic_raids = false;
    rules.cycle.raid_rally_ticks = 1;
    rules.cycle.pause_economy_during_raids = true;
    rules.buildings.town_hall.max_health = u16::MAX;
    rules.raids.spawn_interval_ticks = 1;
    rules.raids.basic.damage = 1;
    rules.raids.basic.health = u16::MAX;
    rules.raids.wave_plan.wave_count = 1;
    rules
}

/// Advance 80 raid ticks under unchanged navigation topology with `raiders`
/// slow basic raiders on the field.
fn raid_tick_navigation(raiders: u64) -> Vec<Metric> {
    let raiders = count(raiders);
    let mut rules = raid_only_rules();
    rules.raids.basic.speed_milli = 50;
    rules.raids.wave_plan.waves[0] = single_wave(&[(RaiderArchetype::Basic, raiders)]);
    rules.validate().expect("raid fixture rules must validate");
    let mut state = GameState::with_rules(SEED, rules);
    state.apply(Command::StartWave).expect("wave should start");
    for _ in 0..(u32::from(raiders) + 4) {
        state.advance_tick();
    }
    assert_eq!(state.raider_count(), usize::from(raiders));
    let before = state.work_counters();
    for _ in 0..80 {
        state.advance_tick();
    }
    assert_eq!(state.raider_count(), usize::from(raiders));
    let work = delta(state.work_counters(), before);
    vec![
        ("flow_field_rebuilds", work.flow_field_rebuilds),
        ("navigation_cells_visited", work.navigation_cells_visited),
        ("path_follow_steps", work.path_follow_steps),
    ]
}

/// Four near-stationary gate raiders (one inside the guard tower's range at
/// the north gate) plus `raiders - 4` fast raiders walking away from it; the
/// tower's targeting work over a 22-tick window.
fn tower_targeting(raiders: u64) -> Vec<Metric> {
    const ANCHORS: u16 = 4;
    let distant = count(raiders)
        .checked_sub(ANCHORS)
        .expect("the targeting fixture always has its four gate raiders");
    let mut rules = raid_only_rules();
    rules.raids.basic.speed_milli = 1;
    rules.raids.advanced.speed_milli = 200;
    rules.raids.advanced.damage = 1;
    rules.raids.advanced.health = u16::MAX;
    rules.towers.arrow.levels[0].range_milli = 1_200;
    let mut groups = vec![(RaiderArchetype::Basic, ANCHORS)];
    if distant > 0 {
        groups.push((RaiderArchetype::Advanced, distant));
    }
    rules.raids.wave_plan.waves[0] = single_wave(&groups);
    rules
        .validate()
        .expect("targeting fixture rules must validate");
    let mut state = GameState::with_rules(SEED, rules);
    state
        .apply(Command::PlaceTower {
            x: GRID_WIDTH / 2 + 1,
            z: 0,
            archetype: TowerArchetype::Arrow,
        })
        .expect("guard tower site must be accepted");
    for _ in 0..3_000 {
        if state.tower_count() == 1 {
            break;
        }
        state.advance_tick();
    }
    assert_eq!(state.tower_count(), 1, "guard tower must complete");
    state.apply(Command::StartWave).expect("wave should start");
    for _ in 0..26 {
        state.advance_tick();
    }
    assert_eq!(state.raider_count(), usize::from(ANCHORS + distant));
    let before = state.work_counters();
    for _ in 0..22 {
        state.advance_tick();
    }
    assert_eq!(state.raider_count(), usize::from(ANCHORS + distant));
    let work = delta(state.work_counters(), before);
    vec![(
        "target_candidate_evaluations",
        work.target_candidate_evaluations,
    )]
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
            state
                .validate_command(Command::PlaceSawmill {
                    x: cell.x,
                    z: cell.z,
                })
                .is_ok()
        })
        .expect("seeded map must accept a sawmill")
}

/// Fixed, finite resources: every seeded forest holds two harvest batches and
/// effectively never regrows, one sawmill, Town Hall storage only.
fn fixed_stock_settlement(workers: u64) -> GameState {
    let workers = count(workers);
    let mut rules = STANDARD_RULES;
    rules.cycle.automatic_raids = false;
    rules.economy.forest_tile_wood = 2 * u32::from(rules.economy.sawmill_output);
    rules.economy.forest_regrowth_interval_ticks = u16::MAX;
    rules.population.base_capacity = workers;
    rules.population.starting_people = workers;
    rules
        .validate()
        .expect("logistics fixture rules must validate");
    let mut state = GameState::with_rules(SEED, rules);
    let sawmill = sawmill_cell(&state);
    state
        .apply(Command::PlaceSawmill {
            x: sawmill.x,
            z: sawmill.z,
        })
        .expect("fixture sawmill must be accepted");
    state
}

/// `workers` people work the fixed stock for 1200 ticks. Assignments are
/// bounded by the stock, so per-worker search work must not grow with the
/// crew (the two-point `linear` ratio check).
fn worker_logistics(workers: u64) -> Vec<Metric> {
    let mut state = fixed_stock_settlement(workers);
    let before = state.work_counters();
    for _ in 0..1_200 {
        state.advance_tick();
    }
    let work = delta(state.work_counters(), before);
    vec![(
        "logistics_candidate_searches",
        work.logistics_candidate_searches,
    )]
}

/// After the fixed stock is worked out, every worker idles at the Town Hall
/// with nothing assignable; 600 such ticks. Re-scanning every candidate for
/// every idle worker would grow with the crew; the per-tick candidate index
/// costs one sweep per tick whatever the crew size.
fn idle_workers_exhausted_stock(workers: u64) -> Vec<Metric> {
    let mut state = fixed_stock_settlement(workers);
    for _ in 0..2_400 {
        state.advance_tick();
    }
    let snapshot = state.snapshot();
    assert!(
        snapshot.entities.iter().all(|entity| match entity.kind {
            EntityKind::Forest => entity.stored_wood == 0,
            _ => entity
                .person_state
                .is_none_or(|person| person == PersonState::IdleAtTownHall),
        }),
        "fixture: the forest stock is exhausted and every worker is idle"
    );
    let before = state.work_counters();
    for _ in 0..600 {
        state.advance_tick();
    }
    let work = delta(state.work_counters(), before);
    vec![(
        "logistics_candidate_searches",
        work.logistics_candidate_searches,
    )]
}

/// Candidate interior cells in a stable order: rows from `rows`, columns left
/// to right. Placement validation decides which are actually buildable.
fn interior_cells(rows: impl Iterator<Item = i16>) -> Vec<Cell> {
    rows.flat_map(|z| (1..GRID_WIDTH - 1).map(move |x| Cell::new(x, z)))
        .collect()
}

/// `haulers` people haul for open tower construction sites in the north
/// while `storages` settlement storage nodes exist (the Town Hall plus empty
/// Storage Houses in the south, farther from every site than the Town Hall).
/// Measured over the window after every hauler was assigned and before any
/// returns idle, so the only logistics work is construction-source selection.
fn construction_hauling(haulers: u64, storages: u64) -> Vec<Metric> {
    const SITES: usize = 6;
    let haulers = count(haulers);
    let mut rules = STANDARD_RULES;
    rules.cycle.automatic_raids = false;
    rules.economy.starting_wood = rules.economy.town_wood_capacity;
    rules.population.base_capacity = haulers;
    rules.population.starting_people = haulers;
    rules
        .validate()
        .expect("construction fixture rules must validate");
    let mut state = GameState::with_rules(SEED, rules);

    let houses = usize::try_from(storages - 1).expect("storage count fits usize");
    let south = interior_cells((GRID_HEIGHT * 3 / 4..GRID_HEIGHT - 1).rev());
    let mut placed = 0;
    for cell in south {
        if placed == houses {
            break;
        }
        if state
            .apply(Command::PlaceStorageHouse {
                x: cell.x,
                z: cell.z,
            })
            .is_ok()
        {
            placed += 1;
        }
    }
    assert_eq!(placed, houses, "fixture storage houses must be accepted");

    let north = interior_cells(2..GRID_HEIGHT / 4);
    let mut sites = 0;
    for cell in north.into_iter().step_by(3) {
        if sites == SITES {
            break;
        }
        if state
            .apply(Command::PlaceTower {
                x: cell.x,
                z: cell.z,
                archetype: TowerArchetype::Arrow,
            })
            .is_ok()
        {
            sites += 1;
        }
    }
    assert_eq!(sites, SITES, "fixture construction sites must be accepted");

    let hauling = |state: &GameState| {
        state.snapshot().entities.iter().all(|entity| {
            entity.person_state.is_none_or(|person| {
                matches!(
                    person,
                    PersonState::ToConstructionStorage | PersonState::ToConstructionSite
                )
            })
        })
    };
    state.advance_tick();
    assert!(hauling(&state), "fixture: every person hauls for a site");
    let before = state.work_counters();
    for _ in 0..HAUL_WINDOW_TICKS {
        state.advance_tick();
    }
    assert!(
        hauling(&state),
        "fixture: no hauler returns idle in the window"
    );
    let work = delta(state.work_counters(), before);
    vec![(
        "logistics_candidate_searches",
        work.logistics_candidate_searches,
    )]
}

/// The balanced headless policy journey (seed 41) for `ticks` ticks.
fn policy_journey(ticks: u64) -> Vec<Metric> {
    let result = simulate(41, ticks, Policy::Balanced);
    vec![("world_clones", result.policy_work.world_clones)]
}

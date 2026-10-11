use std::{env, fmt, process};

use raid_defense_core::{
    Cell, Command, EntityKind, Event, GRID_HEIGHT, GRID_WIDTH, GameState, TowerArchetype,
    WorkCounters,
};

mod work_evidence;

const DEFAULT_SEED: u64 = 1;
const DEFAULT_RUNS: u32 = 1;
const DEFAULT_TICKS: u64 = 4_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Policy {
    Passive,
    Economy,
    Balanced,
    Defense,
}

impl Policy {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "passive" => Ok(Self::Passive),
            "economy" => Ok(Self::Economy),
            "balanced" => Ok(Self::Balanced),
            "defense" => Ok(Self::Defense),
            _ => Err(format!(
                "unknown policy {value:?}; expected passive, economy, balanced, or defense"
            )),
        }
    }

    fn choose_action(self, state: &GameState, work: &mut PolicyWork) -> Option<Command> {
        if self == Self::Passive
            || state.is_night()
            || state.town_health() == 0
            || state.construction_site_count() != 0
        {
            return None;
        }

        let rules = state.rules();
        let targets = PlanTargets::for_policy(
            self,
            state.completed_waves(),
            state.houses_unlocked(),
            rules.buildings.house.unlock_completed_waves,
        );

        match self {
            Self::Passive => None,
            Self::Economy => choose_economy_action(state, targets, work),
            Self::Balanced => choose_balanced_action(state, targets, work),
            Self::Defense => choose_defense_action(state, targets, work),
        }
    }
}

impl fmt::Display for Policy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Passive => "passive",
            Self::Economy => "economy",
            Self::Balanced => "balanced",
            Self::Defense => "defense",
        };
        formatter.write_str(name)
    }
}

/// Deterministic work performed by the policy layer while choosing commands.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PolicyWork {
    /// Candidate commands checked through `GameState::validate_command`.
    validation_probes: u64,
    /// Full game-state clones made while probing candidates. Probing validates
    /// against the authoritative state in place, so this stays zero unless an
    /// explicit, reviewed O(N) clone contract is introduced.
    world_clones: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlanTargets {
    sawmills: usize,
    storage_houses: usize,
    towers: usize,
    houses: usize,
}

impl PlanTargets {
    fn for_policy(
        policy: Policy,
        completed_waves: u32,
        houses_unlocked: bool,
        house_unlock_completed_waves: u32,
    ) -> Self {
        let wave_step = usize::try_from(completed_waves.min(12)).expect("wave step is bounded");
        let waves_since_house_unlock = completed_waves.saturating_sub(house_unlock_completed_waves);
        let houses = if houses_unlocked {
            1 + usize::try_from(waves_since_house_unlock / 5)
                .expect("house step fits usize")
                .min(2)
        } else {
            0
        };

        match policy {
            Policy::Passive => Self {
                sawmills: 0,
                storage_houses: 0,
                towers: 0,
                houses: 0,
            },
            Policy::Economy => Self {
                sawmills: 2 + wave_step / 5,
                storage_houses: 2 + wave_step / 6,
                towers: 1 + wave_step / 4,
                houses,
            },
            Policy::Balanced => Self {
                sawmills: 1 + wave_step / 6,
                storage_houses: 1 + wave_step / 7,
                towers: 2 + wave_step / 2,
                houses,
            },
            Policy::Defense => Self {
                sawmills: 1 + wave_step / 8,
                storage_houses: 1 + wave_step / 10,
                towers: 3 + wave_step,
                houses,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Config {
    seed: u64,
    runs: u32,
    ticks: u64,
    policy: Policy,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEED,
            runs: DEFAULT_RUNS,
            ticks: DEFAULT_TICKS,
            policy: Policy::Balanced,
        }
    }
}

impl Config {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut config = Self::default();
        let mut index = 0;

        while index < args.len() {
            let flag = &args[index];
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("missing value for {flag}"))?;

            match flag.as_str() {
                "--seed" => {
                    config.seed = value
                        .parse()
                        .map_err(|_| format!("invalid --seed value {value:?}"))?;
                }
                "--runs" => {
                    config.runs = value
                        .parse()
                        .map_err(|_| format!("invalid --runs value {value:?}"))?;
                }
                "--ticks" => {
                    config.ticks = value
                        .parse()
                        .map_err(|_| format!("invalid --ticks value {value:?}"))?;
                }
                "--policy" => config.policy = Policy::parse(value)?,
                _ => return Err(format!("unknown argument {flag:?}")),
            }

            index += 2;
        }

        if config.runs == 0 {
            return Err("--runs must be greater than zero".to_owned());
        }
        if config.ticks == 0 {
            return Err("--ticks must be greater than zero".to_owned());
        }

        Ok(config)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TickMetrics {
    peak_raiders: usize,
    shots: u64,
    impacts: u64,
    kills: u64,
    wood_produced: u64,
    wood_picked_up: u64,
    wood_delivered: u64,
    towers_completed: u64,
    wood_stolen: u64,
    town_damage: u64,
}

impl TickMetrics {
    fn record(&mut self, event: Event, state: &GameState) {
        if let Event::TickAdvanced {
            shots,
            impacts,
            kills,
            wood_produced,
            wood_picked_up,
            wood_delivered,
            towers_completed,
            wood_stolen,
            town_damage,
            ..
        } = event
        {
            self.shots += u64::from(shots);
            self.impacts += u64::from(impacts);
            self.kills += u64::from(kills);
            self.wood_produced += u64::from(wood_produced);
            self.wood_picked_up += u64::from(wood_picked_up);
            self.wood_delivered += u64::from(wood_delivered);
            self.towers_completed += u64::from(towers_completed);
            self.wood_stolen += u64::from(wood_stolen);
            self.town_damage += u64::from(town_damage);
        }
        self.peak_raiders = self.peak_raiders.max(state.raider_count());
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SimulationResult {
    seed: u64,
    policy: Policy,
    ticks: u64,
    wave: u32,
    completed_waves: u32,
    town_health: u16,
    town_max_health: u16,
    wood: u32,
    towers: usize,
    construction_sites: usize,
    sawmills: usize,
    storage_houses: usize,
    houses: usize,
    people: usize,
    forests: usize,
    commands_applied: u64,
    metrics: TickMetrics,
    checksum: u64,
    work: WorkCounters,
    policy_work: PolicyWork,
}

impl SimulationResult {
    fn survived_horizon(&self, requested_ticks: u64) -> bool {
        self.town_health != 0 && self.ticks == requested_ticks
    }
}

fn main() {
    if let Err(error) = run_cli() {
        eprintln!("raid-defense-sim: {error}");
        eprintln!("Run with --help for usage.");
        process::exit(2);
    }
}

fn run_cli() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_help();
        return Ok(());
    }

    if args
        .first()
        .is_some_and(|argument| argument == "--work-evidence")
    {
        let [_, flag, sha] = args.as_slice() else {
            return Err("usage: --work-evidence --contract-sha256 <sha256>".to_owned());
        };
        if flag != "--contract-sha256" {
            return Err(format!("unknown argument {flag:?} after --work-evidence"));
        }
        println!("{}", work_evidence::render(sha)?);
        return Ok(());
    }

    let config = Config::parse(&args)?;
    let capacity = usize::try_from(config.runs).expect("run count fits usize");
    let mut results = Vec::with_capacity(capacity);
    for offset in 0..config.runs {
        let seed = config.seed.wrapping_add(u64::from(offset));
        results.push(simulate(seed, config.ticks, config.policy));
    }
    print_report(&config, &results);
    Ok(())
}

fn print_help() {
    println!(
        "Raid Defense headless balance simulator\n\n\
Usage:\n  cargo run -p raid-defense-sim -- [options]\n\n\
Options:\n  --seed <u64>       First deterministic world seed (default: {DEFAULT_SEED})\n  --runs <u32>       Number of consecutive seeds to simulate (default: {DEFAULT_RUNS})\n  --ticks <u64>      Maximum ticks per run (default: {DEFAULT_TICKS})\n  --policy <name>    passive | economy | balanced | defense (default: balanced)\n  --work-evidence --contract-sha256 <sha256>\n                     Print coding-tooling/work-evidence/v1 for the\n                     operations in .performance/contract.json\n  -h, --help         Show this help\n\n\
Examples:\n  cargo run -p raid-defense-sim -- --seed 42 --ticks 6000 --policy balanced\n  cargo run -p raid-defense-sim -- --seed 100 --runs 32 --ticks 10000 --policy economy"
    );
}

fn simulate(seed: u64, tick_limit: u64, policy: Policy) -> SimulationResult {
    let mut state = GameState::new(seed);
    let mut metrics = TickMetrics::default();
    let mut commands_applied = 0_u64;
    let mut policy_work = PolicyWork::default();

    while state.tick() < tick_limit && state.town_health() != 0 {
        if let Some(command) = policy.choose_action(&state, &mut policy_work) {
            state
                .apply(command)
                .expect("a command accepted by the deterministic policy probe must remain valid");
            commands_applied += 1;
        }

        let event = state
            .apply(Command::AdvanceTick)
            .expect("advancing a live simulation tick must succeed");
        metrics.record(event, &state);
    }

    let snapshot = state.snapshot();
    SimulationResult {
        seed,
        policy,
        ticks: snapshot.tick,
        wave: snapshot.wave,
        completed_waves: snapshot.completed_waves,
        town_health: snapshot.town_health,
        town_max_health: snapshot.town_max_health,
        wood: snapshot.wood,
        towers: state.tower_count(),
        construction_sites: state.construction_site_count(),
        sawmills: state.sawmill_count(),
        storage_houses: state.storage_house_count(),
        houses: state.house_count(),
        people: state.people_count(),
        forests: state.forest_count(),
        commands_applied,
        metrics,
        checksum: state.checksum(),
        work: state.work_counters(),
        policy_work,
    }
}

fn choose_economy_action(
    state: &GameState,
    targets: PlanTargets,
    work: &mut PolicyWork,
) -> Option<Command> {
    if state.sawmill_count() < targets.sawmills
        && let Some(command) = try_place_sawmill(state, work)
    {
        return Some(command);
    }
    if state.storage_house_count() < targets.storage_houses
        && let Some(command) = try_place_storage_house(state, work)
    {
        return Some(command);
    }
    if state.house_count() < targets.houses
        && let Some(command) = try_place_house(state, work)
    {
        return Some(command);
    }
    if state.tower_count() < targets.towers
        && let Some(command) = try_place_tower(state, work)
    {
        return Some(command);
    }
    try_upgrade_tower(state, work)
}

fn choose_balanced_action(
    state: &GameState,
    targets: PlanTargets,
    work: &mut PolicyWork,
) -> Option<Command> {
    if state.sawmill_count() == 0
        && let Some(command) = try_place_sawmill(state, work)
    {
        return Some(command);
    }
    if state.storage_house_count() == 0
        && let Some(command) = try_place_storage_house(state, work)
    {
        return Some(command);
    }
    if state.tower_count() < targets.towers
        && let Some(command) = try_place_tower(state, work)
    {
        return Some(command);
    }
    if state.house_count() < targets.houses
        && let Some(command) = try_place_house(state, work)
    {
        return Some(command);
    }
    if state.sawmill_count() < targets.sawmills
        && let Some(command) = try_place_sawmill(state, work)
    {
        return Some(command);
    }
    if state.storage_house_count() < targets.storage_houses
        && let Some(command) = try_place_storage_house(state, work)
    {
        return Some(command);
    }
    try_upgrade_tower(state, work)
}

fn choose_defense_action(
    state: &GameState,
    targets: PlanTargets,
    work: &mut PolicyWork,
) -> Option<Command> {
    if state.sawmill_count() == 0
        && let Some(command) = try_place_sawmill(state, work)
    {
        return Some(command);
    }
    if state.storage_house_count() == 0
        && let Some(command) = try_place_storage_house(state, work)
    {
        return Some(command);
    }
    if state.tower_count() < targets.towers
        && let Some(command) = try_place_tower(state, work)
    {
        return Some(command);
    }
    if let Some(command) = try_upgrade_tower(state, work) {
        return Some(command);
    }
    if state.house_count() < targets.houses
        && let Some(command) = try_place_house(state, work)
    {
        return Some(command);
    }
    if state.sawmill_count() < targets.sawmills
        && let Some(command) = try_place_sawmill(state, work)
    {
        return Some(command);
    }
    if state.storage_house_count() < targets.storage_houses
        && let Some(command) = try_place_storage_house(state, work)
    {
        return Some(command);
    }
    None
}

fn try_place_sawmill(state: &GameState, work: &mut PolicyWork) -> Option<Command> {
    let snapshot = state.snapshot();
    let forests = snapshot
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Forest && entity.stored_wood != 0)
        .map(|entity| entity.cell)
        .collect::<Vec<_>>();
    let mut cells = all_cells();
    cells.sort_by_key(|cell| {
        (
            nearest_manhattan(*cell, &forests),
            distance_to_center(*cell),
            cell.z,
            cell.x,
        )
    });
    first_valid_placement(
        state,
        cells,
        |cell| Command::PlaceSawmill {
            x: cell.x,
            z: cell.z,
        },
        work,
    )
}

fn try_place_storage_house(state: &GameState, work: &mut PolicyWork) -> Option<Command> {
    let snapshot = state.snapshot();
    let sawmills = snapshot
        .entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Sawmill)
        .map(|entity| entity.cell)
        .collect::<Vec<_>>();
    let mut cells = all_cells();
    cells.sort_by_key(|cell| {
        (
            nearest_manhattan(*cell, &sawmills),
            distance_to_center(*cell),
            cell.z,
            cell.x,
        )
    });
    first_valid_placement(
        state,
        cells,
        |cell| Command::PlaceStorageHouse {
            x: cell.x,
            z: cell.z,
        },
        work,
    )
}

fn try_place_house(state: &GameState, work: &mut PolicyWork) -> Option<Command> {
    let mut cells = all_cells();
    cells.sort_by_key(|cell| (distance_to_center(*cell), cell.z, cell.x));
    first_valid_placement(
        state,
        cells,
        |cell| Command::PlaceHouse {
            x: cell.x,
            z: cell.z,
        },
        work,
    )
}

fn try_place_tower(state: &GameState, work: &mut PolicyWork) -> Option<Command> {
    let slot = state.tower_count() + state.construction_site_count();
    let archetype = if slot % 3 == 2 {
        TowerArchetype::Cannon
    } else {
        TowerArchetype::Arrow
    };
    let mut cells = all_cells();
    cells.sort_by_key(|cell| {
        let center_distance = distance_to_center(*cell);
        (center_distance.abs_diff(2), center_distance, cell.z, cell.x)
    });
    first_valid_placement(
        state,
        cells,
        |cell| Command::PlaceTower {
            x: cell.x,
            z: cell.z,
            archetype,
        },
        work,
    )
}

fn try_upgrade_tower(state: &GameState, work: &mut PolicyWork) -> Option<Command> {
    let mut towers = state
        .snapshot()
        .entities
        .into_iter()
        .filter(|entity| {
            entity.kind == EntityKind::Tower
                && entity.tower_level != 0
                && entity.upgrade_cost.is_some()
        })
        .collect::<Vec<_>>();
    towers.sort_by_key(|entity| (entity.tower_level, entity.id));

    for tower in towers {
        let command = Command::UpgradeTower {
            x: tower.cell.x,
            z: tower.cell.z,
        };
        if command_is_valid(state, command, work) {
            return Some(command);
        }
    }
    None
}

fn first_valid_placement<F>(
    state: &GameState,
    cells: Vec<Cell>,
    command_for: F,
    work: &mut PolicyWork,
) -> Option<Command>
where
    F: Fn(Cell) -> Command,
{
    cells.into_iter().find_map(|cell| {
        let command = command_for(cell);
        command_is_valid(state, command, work).then_some(command)
    })
}

/// Probe a candidate command against the authoritative state without cloning
/// it: `validate_command` gives the same result as `apply` and mutates nothing.
fn command_is_valid(state: &GameState, command: Command, work: &mut PolicyWork) -> bool {
    work.validation_probes += 1;
    state.validate_command(command).is_ok()
}

fn all_cells() -> Vec<Cell> {
    (0..GRID_HEIGHT)
        .flat_map(|z| (0..GRID_WIDTH).map(move |x| Cell::new(x, z)))
        .collect()
}

fn distance_to_center(cell: Cell) -> u32 {
    let center_x = i32::from(GRID_WIDTH / 2);
    let center_z = i32::from(GRID_HEIGHT / 2);
    let dx = (i32::from(cell.x) - center_x).unsigned_abs();
    let dz = (i32::from(cell.z) - center_z).unsigned_abs();
    dx.max(dz)
}

fn nearest_manhattan(cell: Cell, targets: &[Cell]) -> u32 {
    targets
        .iter()
        .map(|target| {
            let dx = (i32::from(cell.x) - i32::from(target.x)).unsigned_abs();
            let dz = (i32::from(cell.z) - i32::from(target.z)).unsigned_abs();
            dx + dz
        })
        .min()
        .unwrap_or(u32::MAX)
}

fn print_report(config: &Config, results: &[SimulationResult]) {
    println!(
        "policy={} runs={} ticks={} first_seed={}",
        config.policy, config.runs, config.ticks, config.seed
    );
    println!(
        "seed\tpolicy\tstatus\tticks\twaves\thealth\twood\ttowers\tsites\tsawmills\tstorage\thouses\tpeople\tforests\tcommands\tpeak_raiders\tshots\timpacts\tkills\tproduced\tpicked\tdelivered\ttowers_completed\tstolen\tdamage\tchecksum"
    );

    for result in results {
        let status = if result.survived_horizon(config.ticks) {
            "survived"
        } else {
            "destroyed"
        };
        println!(
            "{}\t{}\t{}\t{}\t{}/{}\t{}/{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:016x}",
            result.seed,
            result.policy,
            status,
            result.ticks,
            result.completed_waves,
            result.wave,
            result.town_health,
            result.town_max_health,
            result.wood,
            result.towers,
            result.construction_sites,
            result.sawmills,
            result.storage_houses,
            result.houses,
            result.people,
            result.forests,
            result.commands_applied,
            result.metrics.peak_raiders,
            result.metrics.shots,
            result.metrics.impacts,
            result.metrics.kills,
            result.metrics.wood_produced,
            result.metrics.wood_picked_up,
            result.metrics.wood_delivered,
            result.metrics.towers_completed,
            result.metrics.wood_stolen,
            result.metrics.town_damage,
            result.checksum,
        );
    }

    for result in results {
        let work = result.work;
        println!(
            "work seed={} flow_field_rebuilds={} navigation_cells_visited={} path_follow_steps={} target_candidate_evaluations={} logistics_candidate_searches={} policy_validation_probes={} world_clones={}",
            result.seed,
            work.flow_field_rebuilds,
            work.navigation_cells_visited,
            work.path_follow_steps,
            work.target_candidate_evaluations,
            work.logistics_candidate_searches,
            result.policy_work.validation_probes,
            result.policy_work.world_clones,
        );
    }

    let survived = results
        .iter()
        .filter(|result| result.survived_horizon(config.ticks))
        .count();
    let completed_waves = results
        .iter()
        .map(|result| u64::from(result.completed_waves))
        .sum::<u64>();
    let kills = results
        .iter()
        .map(|result| result.metrics.kills)
        .sum::<u64>();
    let wood_produced = results
        .iter()
        .map(|result| result.metrics.wood_produced)
        .sum::<u64>();
    let wood_delivered = results
        .iter()
        .map(|result| result.metrics.wood_delivered)
        .sum::<u64>();
    let wood_stolen = results
        .iter()
        .map(|result| result.metrics.wood_stolen)
        .sum::<u64>();
    let runs = u64::try_from(results.len()).expect("run count fits u64");

    println!(
        "summary survived={survived}/{} avg_completed_waves={} avg_kills={} avg_wood_produced={} avg_wood_delivered={} avg_wood_stolen={}",
        results.len(),
        completed_waves / runs,
        kills / runs,
        wood_produced / runs,
        wood_delivered / runs,
        wood_stolen / runs,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_batch_configuration() {
        let args = vec![
            "--seed".to_owned(),
            "17".to_owned(),
            "--runs".to_owned(),
            "4".to_owned(),
            "--ticks".to_owned(),
            "900".to_owned(),
            "--policy".to_owned(),
            "defense".to_owned(),
        ];
        assert_eq!(
            Config::parse(&args),
            Ok(Config {
                seed: 17,
                runs: 4,
                ticks: 900,
                policy: Policy::Defense,
            })
        );
    }

    #[test]
    fn house_targets_follow_active_unlock_rule() {
        let targets = PlanTargets::for_policy(Policy::Balanced, 25, true, 20);
        assert_eq!(targets.houses, 2);
    }

    #[test]
    fn same_seed_policy_and_horizon_are_deterministic() {
        let first = simulate(0x5eed, 800, Policy::Balanced);
        let second = simulate(0x5eed, 800, Policy::Balanced);
        assert_eq!(first, second);
    }

    #[test]
    fn passive_policy_never_issues_player_commands() {
        let result = simulate(7, 100, Policy::Passive);
        assert_eq!(result.commands_applied, 0);
    }

    #[test]
    fn balanced_policy_builds_through_authoritative_commands() {
        let result = simulate(29, 500, Policy::Balanced);
        assert!(result.commands_applied >= 2);
        assert!(result.sawmills >= 1);
        assert!(result.storage_houses >= 1);
    }
}

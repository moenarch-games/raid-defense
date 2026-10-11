#[allow(dead_code)]
mod simulator {
    include!("../main.rs");

    use raid_defense_core::{
        DayLength, ForestDensity, ForestRegrowth, RaidEconomy, RaidSize, RaidTiming,
        RaiderStrength, SawmillThroughput, ScenarioOptions, StartingSupplies,
    };

    const MAX_SCENARIO_CASES: usize = 256;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum SweepDimension {
        StartingSupplies,
        ForestDensity,
        ForestRegrowth,
        SawmillThroughput,
        RaidSize,
        RaiderStrength,
        DayLength,
        RaidTiming,
        RaidEconomy,
    }

    impl SweepDimension {
        fn parse(value: &str) -> Result<Self, String> {
            match value {
                "starting-supplies" => Ok(Self::StartingSupplies),
                "forest-density" => Ok(Self::ForestDensity),
                "forest-regrowth" => Ok(Self::ForestRegrowth),
                "sawmill-throughput" => Ok(Self::SawmillThroughput),
                "raid-size" => Ok(Self::RaidSize),
                "raider-strength" => Ok(Self::RaiderStrength),
                "day-length" => Ok(Self::DayLength),
                "raid-timing" => Ok(Self::RaidTiming),
                "raid-economy" => Ok(Self::RaidEconomy),
                _ => Err(format!(
                    "unknown sweep {value:?}; expected starting-supplies, forest-density, forest-regrowth, sawmill-throughput, raid-size, raider-strength, day-length, raid-timing, or raid-economy"
                )),
            }
        }

        const fn name(self) -> &'static str {
            match self {
                Self::StartingSupplies => "starting-supplies",
                Self::ForestDensity => "forest-density",
                Self::ForestRegrowth => "forest-regrowth",
                Self::SawmillThroughput => "sawmill-throughput",
                Self::RaidSize => "raid-size",
                Self::RaiderStrength => "raider-strength",
                Self::DayLength => "day-length",
                Self::RaidTiming => "raid-timing",
                Self::RaidEconomy => "raid-economy",
            }
        }

        fn expand(self, base: ScenarioOptions) -> Vec<(&'static str, ScenarioOptions)> {
            match self {
                Self::StartingSupplies => [
                    ("lean", StartingSupplies::Lean),
                    ("standard", StartingSupplies::Standard),
                    ("rich", StartingSupplies::Rich),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.starting_supplies = value;
                    (label, scenario)
                })
                .collect(),
                Self::ForestDensity => [
                    ("sparse", ForestDensity::Sparse),
                    ("standard", ForestDensity::Standard),
                    ("dense", ForestDensity::Dense),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.forest_density = value;
                    (label, scenario)
                })
                .collect(),
                Self::ForestRegrowth => [
                    ("slow", ForestRegrowth::Slow),
                    ("standard", ForestRegrowth::Standard),
                    ("fast", ForestRegrowth::Fast),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.forest_regrowth = value;
                    (label, scenario)
                })
                .collect(),
                Self::SawmillThroughput => [
                    ("slow", SawmillThroughput::Slow),
                    ("standard", SawmillThroughput::Standard),
                    ("fast", SawmillThroughput::Fast),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.sawmill_throughput = value;
                    (label, scenario)
                })
                .collect(),
                Self::RaidSize => [
                    ("small", RaidSize::Small),
                    ("standard", RaidSize::Standard),
                    ("large", RaidSize::Large),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.raid_size = value;
                    (label, scenario)
                })
                .collect(),
                Self::RaiderStrength => [
                    ("gentle", RaiderStrength::Gentle),
                    ("standard", RaiderStrength::Standard),
                    ("harsh", RaiderStrength::Harsh),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.raider_strength = value;
                    (label, scenario)
                })
                .collect(),
                Self::DayLength => [
                    ("short", DayLength::Short),
                    ("standard", DayLength::Standard),
                    ("long", DayLength::Long),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.day_length = value;
                    (label, scenario)
                })
                .collect(),
                Self::RaidTiming => [
                    ("standard", RaidTiming::Standard),
                    ("manual", RaidTiming::Manual),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.raid_timing = value;
                    (label, scenario)
                })
                .collect(),
                Self::RaidEconomy => [
                    ("standard", RaidEconomy::Standard),
                    ("continuous", RaidEconomy::Continuous),
                ]
                .into_iter()
                .map(|(label, value)| {
                    let mut scenario = base;
                    scenario.raid_economy = value;
                    (label, scenario)
                })
                .collect(),
            }
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ExperimentConfig {
        seed: u64,
        runs: u32,
        ticks: u64,
        policy: Policy,
        scenario: ScenarioOptions,
        sweeps: Vec<SweepDimension>,
    }

    impl Default for ExperimentConfig {
        fn default() -> Self {
            Self {
                seed: DEFAULT_SEED,
                runs: DEFAULT_RUNS,
                ticks: DEFAULT_TICKS,
                policy: Policy::Balanced,
                scenario: ScenarioOptions::standard(),
                sweeps: Vec::new(),
            }
        }
    }

    impl ExperimentConfig {
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
                    "--starting-supplies" => {
                        config.scenario.starting_supplies = parse_starting_supplies(value)?;
                    }
                    "--forest-density" => {
                        config.scenario.forest_density = parse_forest_density(value)?;
                    }
                    "--forest-regrowth" => {
                        config.scenario.forest_regrowth = parse_forest_regrowth(value)?;
                    }
                    "--sawmill-throughput" => {
                        config.scenario.sawmill_throughput = parse_sawmill_throughput(value)?;
                    }
                    "--raid-size" => config.scenario.raid_size = parse_raid_size(value)?,
                    "--raider-strength" => {
                        config.scenario.raider_strength = parse_raider_strength(value)?;
                    }
                    "--day-length" => config.scenario.day_length = parse_day_length(value)?,
                    "--raid-timing" => config.scenario.raid_timing = parse_raid_timing(value)?,
                    "--raid-economy" => {
                        config.scenario.raid_economy = parse_raid_economy(value)?;
                    }
                    "--sweep" => {
                        let dimension = SweepDimension::parse(value)?;
                        if config.sweeps.contains(&dimension) {
                            return Err(format!("duplicate --sweep {value:?}"));
                        }
                        config.sweeps.push(dimension);
                    }
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
            config
                .scenario
                .into_rules()
                .map_err(|error| format!("scenario does not validate: {error:?}"))?;
            config.cases()?;
            Ok(config)
        }

        fn cases(&self) -> Result<Vec<ExperimentCase>, String> {
            let mut cases = vec![ExperimentCase {
                label: "custom".to_owned(),
                scenario: self.scenario,
            }];

            for sweep in &self.sweeps {
                let mut next = Vec::new();
                for case in cases {
                    for (value_label, scenario) in sweep.expand(case.scenario) {
                        let label = if case.label == "custom" {
                            format!("{}={value_label}", sweep.name())
                        } else {
                            format!("{},{}={value_label}", case.label, sweep.name())
                        };
                        next.push(ExperimentCase { label, scenario });
                    }
                }
                if next.len() > MAX_SCENARIO_CASES {
                    return Err(format!(
                        "scenario matrix expands to {} cases; maximum is {MAX_SCENARIO_CASES}",
                        next.len()
                    ));
                }
                cases = next;
            }

            Ok(cases)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ExperimentCase {
        label: String,
        scenario: ScenarioOptions,
    }

    pub(super) fn run_experiment_cli() -> Result<(), String> {
        let args = env::args().skip(1).collect::<Vec<_>>();
        if args
            .iter()
            .any(|argument| argument == "--help" || argument == "-h")
        {
            print_experiment_help();
            return Ok(());
        }

        let config = ExperimentConfig::parse(&args)?;
        let cases = config.cases()?;
        println!(
            "scenario_experiment policy={} cases={} runs_per_case={} ticks={} first_seed={}",
            config.policy,
            cases.len(),
            config.runs,
            config.ticks,
            config.seed
        );

        for case in cases {
            let rules = case
                .scenario
                .into_rules()
                .map_err(|error| format!("scenario {} does not validate: {error:?}", case.label))?;
            println!(
                "\nscenario_case={} rules_fingerprint={:016x} options={}",
                case.label,
                rules.fingerprint(),
                scenario_description(case.scenario)
            );

            let capacity = usize::try_from(config.runs).expect("run count fits usize");
            let mut results = Vec::with_capacity(capacity);
            for offset in 0..config.runs {
                let seed = config.seed.wrapping_add(u64::from(offset));
                results.push(simulate_scenario(
                    seed,
                    config.ticks,
                    config.policy,
                    case.scenario,
                ));
            }

            print_report(
                &Config {
                    seed: config.seed,
                    runs: config.runs,
                    ticks: config.ticks,
                    policy: config.policy,
                },
                &results,
            );
        }

        Ok(())
    }

    fn simulate_scenario(
        seed: u64,
        tick_limit: u64,
        policy: Policy,
        scenario: ScenarioOptions,
    ) -> SimulationResult {
        let rules = scenario
            .into_rules()
            .expect("experiment scenarios are validated before simulation");
        let manual_raids = !rules.cycle.automatic_raids;
        let manual_day_length = u64::from(rules.cycle.day_length_ticks);
        let mut next_manual_wave_tick = manual_raids.then_some(manual_day_length);
        let mut state = GameState::with_rules(seed, rules);
        let mut metrics = TickMetrics::default();
        let mut commands_applied = 0_u64;
        let mut policy_work = PolicyWork::default();

        while state.tick() < tick_limit && state.town_health() != 0 {
            if let Some(next_tick) = next_manual_wave_tick
                && !state.is_night()
                && state.tick() >= next_tick
            {
                state
                    .apply(Command::StartWave)
                    .expect("manual scenario driver starts only inactive live games");
                commands_applied += 1;
                next_manual_wave_tick = None;
            }

            if let Some(command) = policy.choose_action(&state, &mut policy_work) {
                state.apply(command).expect(
                    "a command accepted by the deterministic policy probe must remain valid",
                );
                commands_applied += 1;
            }

            let event = state
                .apply(Command::AdvanceTick)
                .expect("advancing a live simulation tick must succeed");
            let completed_manual_wave = manual_raids
                && matches!(
                    event,
                    Event::TickAdvanced {
                        completed_wave: Some(_),
                        ..
                    }
                );
            metrics.record(event, &state);
            if completed_manual_wave {
                next_manual_wave_tick = Some(state.tick().saturating_add(manual_day_length));
            }
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

    fn print_experiment_help() {
        println!(
            "Raid Defense deterministic scenario experiment matrix\n\n\
Usage:\n  cargo run -p raid-defense-sim --bin scenario-experiment -- [options]\n\n\
Run controls:\n  --seed <u64>                  First deterministic world seed (default: {DEFAULT_SEED})\n  --runs <u32>                  Consecutive seeds per scenario case (default: {DEFAULT_RUNS})\n  --ticks <u64>                 Maximum ticks per run (default: {DEFAULT_TICKS})\n  --policy <name>               passive | economy | balanced | defense (default: balanced)\n\n\
Scenario baseline overrides:\n  --starting-supplies <value>   lean | standard | rich\n  --forest-density <value>      sparse | standard | dense\n  --forest-regrowth <value>     slow | standard | fast\n  --sawmill-throughput <value>  slow | standard | fast\n  --raid-size <value>           small | standard | large\n  --raider-strength <value>     gentle | standard | harsh\n  --day-length <value>          short | standard | long\n  --raid-timing <value>         standard | manual\n  --raid-economy <value>        standard | continuous\n\n\
Matrix controls:\n  --sweep <dimension>           Repeat to create a Cartesian product. Dimensions use the flag names above.\n                                At most {MAX_SCENARIO_CASES} scenario cases are allowed.\n  -h, --help                    Show this help\n\n\
Examples:\n  cargo run -p raid-defense-sim --bin scenario-experiment -- --seed 40 --runs 16 --ticks 8000 --sweep raid-size\n  cargo run -p raid-defense-sim --bin scenario-experiment -- --seed 40 --runs 16 --ticks 8000 --sweep raid-size --sweep raider-strength\n  cargo run -p raid-defense-sim --bin scenario-experiment -- --starting-supplies lean --forest-density sparse --forest-regrowth slow --raid-size large --raider-strength harsh"
        );
    }

    fn scenario_description(scenario: ScenarioOptions) -> String {
        format!(
            "starting-supplies={},forest-density={},forest-regrowth={},sawmill-throughput={},raid-size={},raider-strength={},day-length={},raid-timing={},raid-economy={}",
            starting_supplies_name(scenario.starting_supplies),
            forest_density_name(scenario.forest_density),
            forest_regrowth_name(scenario.forest_regrowth),
            sawmill_throughput_name(scenario.sawmill_throughput),
            raid_size_name(scenario.raid_size),
            raider_strength_name(scenario.raider_strength),
            day_length_name(scenario.day_length),
            raid_timing_name(scenario.raid_timing),
            raid_economy_name(scenario.raid_economy),
        )
    }

    fn parse_starting_supplies(value: &str) -> Result<StartingSupplies, String> {
        match value {
            "lean" => Ok(StartingSupplies::Lean),
            "standard" => Ok(StartingSupplies::Standard),
            "rich" => Ok(StartingSupplies::Rich),
            _ => Err(format!("invalid starting supplies {value:?}")),
        }
    }

    fn parse_forest_density(value: &str) -> Result<ForestDensity, String> {
        match value {
            "sparse" => Ok(ForestDensity::Sparse),
            "standard" => Ok(ForestDensity::Standard),
            "dense" => Ok(ForestDensity::Dense),
            _ => Err(format!("invalid forest density {value:?}")),
        }
    }

    fn parse_forest_regrowth(value: &str) -> Result<ForestRegrowth, String> {
        match value {
            "slow" => Ok(ForestRegrowth::Slow),
            "standard" => Ok(ForestRegrowth::Standard),
            "fast" => Ok(ForestRegrowth::Fast),
            _ => Err(format!("invalid forest regrowth {value:?}")),
        }
    }

    fn parse_sawmill_throughput(value: &str) -> Result<SawmillThroughput, String> {
        match value {
            "slow" => Ok(SawmillThroughput::Slow),
            "standard" => Ok(SawmillThroughput::Standard),
            "fast" => Ok(SawmillThroughput::Fast),
            _ => Err(format!("invalid sawmill throughput {value:?}")),
        }
    }

    fn parse_raid_size(value: &str) -> Result<RaidSize, String> {
        match value {
            "small" => Ok(RaidSize::Small),
            "standard" => Ok(RaidSize::Standard),
            "large" => Ok(RaidSize::Large),
            _ => Err(format!("invalid raid size {value:?}")),
        }
    }

    fn parse_raider_strength(value: &str) -> Result<RaiderStrength, String> {
        match value {
            "gentle" => Ok(RaiderStrength::Gentle),
            "standard" => Ok(RaiderStrength::Standard),
            "harsh" => Ok(RaiderStrength::Harsh),
            _ => Err(format!("invalid raider strength {value:?}")),
        }
    }

    fn parse_day_length(value: &str) -> Result<DayLength, String> {
        match value {
            "short" => Ok(DayLength::Short),
            "standard" => Ok(DayLength::Standard),
            "long" => Ok(DayLength::Long),
            _ => Err(format!("invalid day length {value:?}")),
        }
    }

    fn parse_raid_timing(value: &str) -> Result<RaidTiming, String> {
        match value {
            "standard" => Ok(RaidTiming::Standard),
            "manual" => Ok(RaidTiming::Manual),
            _ => Err(format!("invalid raid timing {value:?}")),
        }
    }

    fn parse_raid_economy(value: &str) -> Result<RaidEconomy, String> {
        match value {
            "standard" => Ok(RaidEconomy::Standard),
            "continuous" => Ok(RaidEconomy::Continuous),
            _ => Err(format!("invalid raid economy {value:?}")),
        }
    }

    const fn starting_supplies_name(value: StartingSupplies) -> &'static str {
        match value {
            StartingSupplies::Lean => "lean",
            StartingSupplies::Standard => "standard",
            StartingSupplies::Rich => "rich",
        }
    }

    const fn forest_density_name(value: ForestDensity) -> &'static str {
        match value {
            ForestDensity::Sparse => "sparse",
            ForestDensity::Standard => "standard",
            ForestDensity::Dense => "dense",
        }
    }

    const fn forest_regrowth_name(value: ForestRegrowth) -> &'static str {
        match value {
            ForestRegrowth::Slow => "slow",
            ForestRegrowth::Standard => "standard",
            ForestRegrowth::Fast => "fast",
        }
    }

    const fn sawmill_throughput_name(value: SawmillThroughput) -> &'static str {
        match value {
            SawmillThroughput::Slow => "slow",
            SawmillThroughput::Standard => "standard",
            SawmillThroughput::Fast => "fast",
        }
    }

    const fn raid_size_name(value: RaidSize) -> &'static str {
        match value {
            RaidSize::Small => "small",
            RaidSize::Standard => "standard",
            RaidSize::Large => "large",
        }
    }

    const fn raider_strength_name(value: RaiderStrength) -> &'static str {
        match value {
            RaiderStrength::Gentle => "gentle",
            RaiderStrength::Standard => "standard",
            RaiderStrength::Harsh => "harsh",
        }
    }

    const fn day_length_name(value: DayLength) -> &'static str {
        match value {
            DayLength::Short => "short",
            DayLength::Standard => "standard",
            DayLength::Long => "long",
        }
    }

    const fn raid_timing_name(value: RaidTiming) -> &'static str {
        match value {
            RaidTiming::Standard => "standard",
            RaidTiming::Manual => "manual",
        }
    }

    const fn raid_economy_name(value: RaidEconomy) -> &'static str {
        match value {
            RaidEconomy::Standard => "standard",
            RaidEconomy::Continuous => "continuous",
        }
    }

    #[cfg(test)]
    mod experiment_tests {
        use super::*;

        #[test]
        fn parses_scenario_overrides_and_sweeps() {
            let config = ExperimentConfig::parse(&[
                "--runs".to_owned(),
                "4".to_owned(),
                "--starting-supplies".to_owned(),
                "rich".to_owned(),
                "--raid-size".to_owned(),
                "large".to_owned(),
                "--sweep".to_owned(),
                "raider-strength".to_owned(),
            ])
            .expect("experiment config should parse");

            assert_eq!(config.runs, 4);
            assert_eq!(config.scenario.starting_supplies, StartingSupplies::Rich);
            assert_eq!(config.scenario.raid_size, RaidSize::Large);
            assert_eq!(config.sweeps, vec![SweepDimension::RaiderStrength]);
        }

        #[test]
        fn two_three_value_sweeps_create_nine_cases() {
            let config = ExperimentConfig {
                sweeps: vec![SweepDimension::RaidSize, SweepDimension::RaiderStrength],
                ..ExperimentConfig::default()
            };
            let cases = config.cases().expect("matrix should remain bounded");
            assert_eq!(cases.len(), 9);
        }

        #[test]
        fn same_scenario_seed_policy_and_horizon_are_deterministic() {
            let scenario = ScenarioOptions::standard();
            let first = simulate_scenario(0x5eed, 800, Policy::Balanced, scenario);
            let second = simulate_scenario(0x5eed, 800, Policy::Balanced, scenario);
            assert_eq!(first, second);
        }

        #[test]
        fn scenario_rules_participate_in_simulation_identity() {
            let standard =
                simulate_scenario(0x5eed, 100, Policy::Passive, ScenarioOptions::standard());
            let mut harsh = ScenarioOptions::standard();
            harsh.raid_size = RaidSize::Large;
            harsh.raider_strength = RaiderStrength::Harsh;
            let harsh = simulate_scenario(0x5eed, 100, Policy::Passive, harsh);

            assert_ne!(standard.checksum, harsh.checksum);
        }

        #[test]
        fn manual_raid_timing_is_driven_deterministically_by_the_harness() {
            let mut scenario = ScenarioOptions::standard();
            scenario.raid_timing = RaidTiming::Manual;
            let result = simulate_scenario(17, 650, Policy::Passive, scenario);
            assert_eq!(result.wave, 1);
        }
    }
}

fn main() {
    if let Err(error) = simulator::run_experiment_cli() {
        eprintln!("raid-defense scenario-experiment: {error}");
        eprintln!("Run with --help for usage.");
        std::process::exit(2);
    }
}

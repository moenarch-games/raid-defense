//! Acceptance: the headless performance scenarios emit deterministic work
//! counters and the policy validates candidate commands without cloning the
//! world (#38).
//!
//! Report contract: for every simulated run the simulator prints one line that
//! starts with `work` and contains `seed=<seed>` followed by whitespace-separated
//! `<counter>=<u64>` tokens for every operation-count metric declared in
//! `.performance/contract.json`.

use std::{collections::BTreeMap, path::PathBuf, process::Command};

const CORE_COUNTERS: [&str; 5] = [
    "flow_field_rebuilds",
    "navigation_cells_visited",
    "path_follow_steps",
    "target_candidate_evaluations",
    "logistics_candidate_searches",
];
const POLICY_COUNTERS: [&str; 2] = ["policy_validation_probes", "world_clones"];

fn simulate(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_raid-defense-sim"))
        .args(args)
        .output()
        .expect("simulator binary must run");
    assert!(
        output.status.success(),
        "simulator failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("report must be UTF-8")
}

fn work_counters(report: &str, seed: u64) -> BTreeMap<String, u64> {
    let seed_token = format!("seed={seed}");
    let line = report
        .lines()
        .find(|line| {
            line.starts_with("work") && line.split_whitespace().any(|token| token == seed_token)
        })
        .unwrap_or_else(|| panic!("report must contain a work line for {seed_token}:\n{report}"));
    line.split_whitespace()
        .filter_map(|token| token.split_once('='))
        .filter(|(name, _)| *name != "seed")
        .map(|(name, value)| {
            let value = value
                .parse::<u64>()
                .unwrap_or_else(|_| panic!("counter {name} must be an integer, got {value:?}"));
            (name.to_owned(), value)
        })
        .collect()
}

fn checksum(report: &str, seed: u64) -> String {
    let prefix = format!("{seed}\t");
    report
        .lines()
        .find(|line| line.starts_with(&prefix))
        .and_then(|line| line.rsplit('\t').next())
        .unwrap_or_else(|| panic!("report must contain a result row for seed {seed}"))
        .to_owned()
}

fn contract() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.performance/contract.json");
    std::fs::read_to_string(path).expect(".performance/contract.json must exist")
}

#[test]
fn balanced_representative_scenario_reports_deterministic_work_counters() {
    // Dimensions of the `balanced-representative` contract scenario.
    let args = [
        "--seed", "41", "--runs", "1", "--ticks", "4000", "--policy", "balanced",
    ];
    let first = simulate(&args);
    let second = simulate(&args);
    assert_eq!(first, second, "report must be byte-identical across reruns");

    let counters = work_counters(&first, 41);
    for name in CORE_COUNTERS.iter().chain(POLICY_COUNTERS.iter()) {
        assert!(
            counters.contains_key(*name),
            "work line must report {name}: {counters:?}"
        );
    }
    for name in [
        "flow_field_rebuilds",
        "navigation_cells_visited",
        "path_follow_steps",
        "target_candidate_evaluations",
        "logistics_candidate_searches",
        "policy_validation_probes",
    ] {
        assert!(
            counters[name] > 0,
            "{name} must measure real work: {counters:?}"
        );
    }

    // Logistics candidate searching is reported separately from movement.
    assert_ne!(
        counters["logistics_candidate_searches"], counters["path_follow_steps"],
        "logistics searches and path-follow work must be distinct counters: {counters:?}"
    );
}

#[test]
fn policy_validates_candidate_commands_without_cloning_the_world() {
    let report = simulate(&[
        "--seed", "41", "--runs", "1", "--ticks", "4000", "--policy", "balanced",
    ]);
    let counters = work_counters(&report, 41);
    assert!(
        counters["policy_validation_probes"] > 0,
        "the balanced policy must probe candidate commands: {counters:?}"
    );
    assert_eq!(
        counters["world_clones"], 0,
        "probing a candidate command must not clone/materialise the full game state \
         (no explicit O(N) clone contract is declared): {counters:?}"
    );
}

#[test]
fn work_lines_are_reported_per_run() {
    let report = simulate(&[
        "--seed", "7", "--runs", "2", "--ticks", "300", "--policy", "economy",
    ]);
    for seed in [7, 8] {
        let counters = work_counters(&report, seed);
        assert!(counters.contains_key("flow_field_rebuilds"), "{counters:?}");
    }
}

#[test]
fn policy_change_preserves_baseline_simulation_outcome() {
    // Pinned on the pre-instrumentation baseline (origin/main 153ab0a):
    // non-cloning validation and counters must not alter what the policy does.
    let report = simulate(&[
        "--seed", "41", "--runs", "1", "--ticks", "4000", "--policy", "balanced",
    ]);
    assert_eq!(checksum(&report, 41), BALANCED_41_CHECKSUM);
    let report = simulate(&[
        "--seed", "41", "--runs", "1", "--ticks", "8000", "--policy", "defense",
    ]);
    assert_eq!(checksum(&report, 41), DEFENSE_41_CHECKSUM);
}

const BALANCED_41_CHECKSUM: &str = "8531da094a74946b";
const DEFENSE_41_CHECKSUM: &str = "1fa3d48b7ec81897";

#[test]
fn performance_contract_declares_every_reported_operation_counter() {
    let contract = contract();
    for name in CORE_COUNTERS.iter().chain(POLICY_COUNTERS.iter()) {
        assert!(
            contract.contains(&format!("\"name\": \"{name}\"")),
            ".performance/contract.json must declare operation-count metric {name}"
        );
    }
    for retired in ["\"target_candidates\"", "\"path_searches\""] {
        assert!(
            !contract.contains(retired),
            ".performance/contract.json still declares placeholder metric {retired}"
        );
    }
}

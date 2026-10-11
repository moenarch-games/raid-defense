//! Acceptance: the stable deterministic work counters are declared as
//! executable growth/budget contracts through coding-tooling's
//! `.performance/contract.json` schemaVersion 2 `operations` (#42, budget
//! promotion; coding-tooling#300 / PR #306).
//!
//! coding-tooling#300 is closed and its schema exists, so #42's "or" resolves to
//! the growth declarations rather than v1 `blocking` smoke flags. This test is
//! the repository-owned guard that the declaration exists, is well formed by
//! the coding-tooling v2 rules (`docs/work-complexity.md`), names the counters
//! the simulator really reports, and is wired to an evidence collector so
//! `coding-tooling performance work` can evaluate it (a declaration without a
//! collector is `unavailable`, never green). Wall-clock timing stays advisory.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use serde_json::Value;

const REPORTED_COUNTERS: [&str; 7] = [
    "flow_field_rebuilds",
    "navigation_cells_visited",
    "path_follow_steps",
    "target_candidate_evaluations",
    "logistics_candidate_searches",
    "policy_validation_probes",
    "world_clones",
];

const DETERMINISTIC_SIGNALS: [&str; 7] = [
    "operation-count",
    "allocation-count",
    "instruction-count",
    "cache-event-count",
    "memory",
    "size",
    "custom",
];

fn repo_file(relative: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{relative} must be JSON: {error}"))
}

fn contract() -> Value {
    repo_file(".performance/contract.json")
}

fn non_empty_str<'a>(value: &'a Value, what: &str) -> &'a str {
    let text = value
        .as_str()
        .unwrap_or_else(|| panic!("{what} must be a string: {value}"));
    assert!(!text.trim().is_empty(), "{what} must not be blank");
    text
}

#[derive(Debug, Default)]
struct Declared {
    /// metric -> set of (dimension, bound)
    growth: BTreeMap<String, BTreeSet<(String, String)>>,
    /// metric -> budget maxima
    budgets: BTreeMap<String, Vec<u64>>,
}

/// Validate the operations block by the coding-tooling v2 rules and collect
/// what each metric declares.
fn declared_operations(contract: &Value) -> Declared {
    let operations = contract["operations"]
        .as_array()
        .expect("schemaVersion 2 contract must declare an `operations` array");
    assert!(
        !operations.is_empty(),
        "operations must declare at least one product operation"
    );

    let mut ids = BTreeSet::new();
    let mut declared = Declared::default();
    for operation in operations {
        let id = non_empty_str(&operation["id"], "operation id");
        assert!(ids.insert(id.to_owned()), "operation id {id} is repeated");
        non_empty_str(&operation["description"], &format!("{id} description"));

        let dimensions = operation["dimensions"]
            .as_object()
            .unwrap_or_else(|| panic!("{id}: dimensions must be an object"));
        assert!(!dimensions.is_empty(), "{id}: dimensions must not be empty");
        for (name, dimension) in dimensions {
            non_empty_str(
                &dimension["description"],
                &format!("{id}.{name} description"),
            );
        }

        let points = operation["scalePoints"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: scalePoints must be an array"));
        let mut parsed_points = Vec::new();
        for point in points {
            let point = point
                .as_object()
                .unwrap_or_else(|| panic!("{id}: each scale point must be an object"));
            assert_eq!(
                point.keys().collect::<BTreeSet<_>>(),
                dimensions.keys().collect::<BTreeSet<_>>(),
                "{id}: every scale point gives exactly one value per declared dimension"
            );
            let values = point
                .iter()
                .map(|(name, value)| {
                    let value = value
                        .as_u64()
                        .filter(|value| *value > 0)
                        .unwrap_or_else(|| {
                            panic!("{id}: scale value {name} must be a positive integer")
                        });
                    (name.clone(), value)
                })
                .collect::<BTreeMap<_, _>>();
            assert!(
                !parsed_points.contains(&values),
                "{id}: scale point {values:?} is repeated"
            );
            parsed_points.push(values);
        }

        let metrics = operation["metrics"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: metrics must be an array"));
        assert!(!metrics.is_empty(), "{id}: metrics must not be empty");
        for metric in metrics {
            let name = non_empty_str(&metric["name"], &format!("{id} metric name"));
            non_empty_str(&metric["unit"], &format!("{id}.{name} unit"));
            let signal = non_empty_str(&metric["signal"], &format!("{id}.{name} signal"));
            assert!(
                DETERMINISTIC_SIGNALS.contains(&signal),
                "{id}.{name}: signal {signal} is not a deterministic counter signal"
            );
            assert!(
                REPORTED_COUNTERS.contains(&name),
                "{id}.{name}: operations may only declare counters the simulator reports"
            );

            let budget = metric.get("budget").map(|budget| {
                budget["max"].as_u64().unwrap_or_else(|| {
                    panic!("{id}.{name}: budget.max must be a non-negative integer")
                })
            });
            let growth = metric
                .get("growth")
                .map(|growth| {
                    growth
                        .as_array()
                        .unwrap_or_else(|| panic!("{id}.{name}: growth must be an array"))
                        .clone()
                })
                .unwrap_or_default();
            assert!(
                budget.is_some() || !growth.is_empty(),
                "{id}.{name}: every metric declares a budget, growth bounds, or both"
            );
            if let Some(max) = budget {
                declared
                    .budgets
                    .entry(name.to_owned())
                    .or_default()
                    .push(max);
            }
            for bound in growth {
                let dimension =
                    non_empty_str(&bound["dimension"], &format!("{id}.{name} dimension"));
                let class = non_empty_str(&bound["bound"], &format!("{id}.{name} bound"));
                assert!(
                    dimensions.contains_key(dimension),
                    "{id}.{name}: growth dimension {dimension} is not declared"
                );
                assert!(
                    matches!(class, "constant" | "linear"),
                    "{id}.{name}: growth bound {class} is outside the closed vocabulary"
                );
                // A growth bound needs a group of points that varies this
                // dimension while every other dimension is held fixed.
                let varied = parsed_points.iter().any(|a| {
                    parsed_points.iter().any(|b| {
                        a[dimension] != b[dimension]
                            && a.iter()
                                .all(|(other, value)| other == dimension || b[other] == *value)
                    })
                });
                assert!(
                    varied,
                    "{id}.{name}: no scale points vary {dimension} with the other dimensions fixed"
                );
                declared
                    .growth
                    .entry(name.to_owned())
                    .or_default()
                    .insert((dimension.to_owned(), class.to_owned()));
            }
        }
    }
    declared
}

fn has_growth(declared: &Declared, metric: &str, classes: &[&str]) -> bool {
    declared.growth.get(metric).is_some_and(|bounds| {
        bounds
            .iter()
            .any(|(_, class)| classes.contains(&class.as_str()))
    })
}

#[test]
fn performance_contract_uses_coding_tooling_growth_declarations() {
    let contract = contract();
    assert_eq!(
        contract["schemaVersion"].as_u64(),
        Some(2),
        "coding-tooling#300 is closed: .performance/contract.json must move to schemaVersion 2 \
         and declare the stable counters as operations with growth bounds"
    );
    let declared = declared_operations(&contract);

    for (metric, classes) in [
        ("flow_field_rebuilds", &["constant"][..]),
        ("navigation_cells_visited", &["constant"][..]),
        ("target_candidate_evaluations", &["constant"][..]),
        ("logistics_candidate_searches", &["constant", "linear"][..]),
        ("path_follow_steps", &["constant", "linear"][..]),
    ] {
        assert!(
            has_growth(&declared, metric, classes),
            "{metric} must be declared with a {classes:?} growth bound in some operation: {declared:?}"
        );
    }
    assert!(
        declared
            .budgets
            .get("world_clones")
            .is_some_and(|maxima| !maxima.is_empty() && maxima.iter().all(|max| *max == 0)),
        "world_clones must be declared with budget.max 0 (no O(N) clone contract): {declared:?}"
    );
}

#[test]
fn wall_clock_timing_stays_advisory() {
    let contract = contract();
    let scenarios = contract["scenarios"]
        .as_array()
        .expect("scenarios stay required in schemaVersion 2");
    assert!(!scenarios.is_empty());
    for scenario in scenarios {
        for metric in scenario["metrics"].as_array().expect("scenario metrics") {
            if metric["signal"] == "wall-clock" {
                assert_eq!(
                    metric["blocking"],
                    Value::Bool(false),
                    "wall-clock metric {} in {} must stay advisory",
                    metric["name"],
                    scenario["id"]
                );
            }
        }
    }
    if let Some(operations) = contract["operations"].as_array() {
        for operation in operations {
            for metric in operation["metrics"].as_array().into_iter().flatten() {
                assert_ne!(
                    metric["signal"], "wall-clock",
                    "operations hold deterministic counters only"
                );
            }
        }
    }
}

#[test]
fn work_evidence_collector_is_wired_for_coding_tooling() {
    let tooling = repo_file(".coding-tooling.json");
    let command = tooling["capabilityCommands"]["."]["performance:work"]
        .as_array()
        .expect(
            ".coding-tooling.json must declare a capabilityCommands[\".\"][\"performance:work\"] argv \
             that prints coding-tooling/work-evidence/v1 for the declared operations",
        );
    assert!(
        !command.is_empty()
            && command
                .iter()
                .all(|arg| arg.as_str().is_some_and(|arg| !arg.is_empty())),
        "performance:work must be a non-empty argv of strings: {command:?}"
    );
    let required = tooling["requiredCapabilities"]
        .as_array()
        .expect("requiredCapabilities");
    assert!(
        required
            .iter()
            .any(|capability| capability == "performance:work"),
        "performance:work must be a required capability so missing evidence is not green"
    );
}

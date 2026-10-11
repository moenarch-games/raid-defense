//! The `performance:work` collector (`raid-defense-sim --work-evidence`) emits
//! `coding-tooling/work-evidence/v1` that matches the `operations` declared in
//! `.performance/contract.json` exactly, and the emitted counters satisfy the
//! declared growth bounds and budgets.
//!
//! `coding-tooling performance work` is the authoritative evaluator (it also
//! binds the evidence to the contract's sha256). This test re-applies the same
//! documented checks (`docs/work-complexity.md` in coding-tooling) inside
//! `cargo test`, so hosted CI enforces the declarations without coding-tooling
//! installed. It runs the already-built simulator binary, never a nested cargo.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    process::Command,
};

use serde_json::Value;

const PLACEHOLDER_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

type Point = BTreeMap<String, u64>;

fn contract() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.performance/contract.json");
    let text = std::fs::read_to_string(path).expect(".performance/contract.json must exist");
    serde_json::from_str(&text).expect("contract must be JSON")
}

fn evidence() -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_raid-defense-sim"))
        .args(["--work-evidence", "--contract-sha256", PLACEHOLDER_SHA256])
        .output()
        .expect("simulator binary must run");
    assert!(
        output.status.success(),
        "collector failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("collector must print one JSON document")
}

fn point(value: &Value) -> Point {
    value
        .as_object()
        .expect("scale point object")
        .iter()
        .map(|(name, value)| (name.clone(), value.as_u64().expect("positive integer")))
        .collect()
}

/// Check one growth bound over the samples, grouped by every other dimension.
fn check_growth(samples: &[(Point, u64)], dimension: &str, bound: &str) -> Result<(), String> {
    let mut groups = BTreeMap::<Point, Vec<(u64, u64)>>::new();
    for (point, value) in samples {
        let mut key = point.clone();
        let x = key.remove(dimension).expect("dimension present");
        groups.entry(key).or_default().push((x, *value));
    }
    for group in groups.values_mut() {
        group.sort_unstable();
        let (x0, v0) = (u128::from(group[0].0), u128::from(group[0].1));
        match (bound, group.len()) {
            (_, 1) => {}
            ("constant", _) => {
                if let Some((x, v)) = group.iter().find(|(_, v)| u128::from(*v) > v0) {
                    return Err(format!("constant: {v} at {x} exceeds {v0} at {x0}"));
                }
            }
            ("linear", 2) => {
                let (x1, v1) = (u128::from(group[1].0), u128::from(group[1].1));
                if v1 * x0 > v0 * x1 {
                    return Err(format!("linear: {v1} at {x1} vs {v0} at {x0}"));
                }
            }
            ("linear", _) => {
                let (x1, v1) = (u128::from(group[1].0), u128::from(group[1].1));
                let rate = v1.saturating_sub(v0);
                for pair in group.windows(2).skip(1) {
                    let (xa, va) = (u128::from(pair[0].0), u128::from(pair[0].1));
                    let (xb, vb) = (u128::from(pair[1].0), u128::from(pair[1].1));
                    if vb.saturating_sub(va) * (x1 - x0) > rate * (xb - xa) {
                        return Err(format!(
                            "linear: marginal work {va}->{vb} over {xa}->{xb} exceeds the first interval"
                        ));
                    }
                }
            }
            (other, _) => return Err(format!("unknown bound {other}")),
        }
    }
    Ok(())
}

#[test]
fn collector_evidence_matches_the_declared_operations_and_bounds() {
    let contract = contract();
    let evidence = evidence();
    assert_eq!(evidence["schemaVersion"], "coding-tooling/work-evidence/v1");
    assert_eq!(evidence["suite"], contract["suite"]);
    assert_eq!(evidence["contractSha256"], PLACEHOLDER_SHA256);

    let declared = contract["operations"]
        .as_array()
        .expect("contract operations");
    let emitted = evidence["operations"]
        .as_array()
        .expect("evidence operations");
    let declared_ids = declared
        .iter()
        .map(|operation| operation["id"].as_str().expect("id"))
        .collect::<BTreeSet<_>>();
    let emitted_ids = emitted
        .iter()
        .map(|operation| operation["id"].as_str().expect("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        emitted_ids.iter().copied().collect::<BTreeSet<_>>(),
        declared_ids,
        "collector must emit exactly the declared operations"
    );
    assert_eq!(
        emitted_ids.len(),
        declared_ids.len(),
        "no repeated operations"
    );

    for operation in declared {
        let id = operation["id"].as_str().expect("id");
        let samples = emitted
            .iter()
            .find(|emitted| emitted["id"] == id)
            .and_then(|emitted| emitted["samples"].as_array())
            .expect("samples");
        let declared_points = operation["scalePoints"]
            .as_array()
            .expect("scalePoints")
            .iter()
            .map(point)
            .collect::<Vec<_>>();
        let emitted_points = samples
            .iter()
            .map(|sample| point(&sample["dimensions"]))
            .collect::<Vec<_>>();
        assert_eq!(
            emitted_points, declared_points,
            "{id}: collector must emit every declared scale point exactly once, in order"
        );

        let metric_names = operation["metrics"]
            .as_array()
            .expect("metrics")
            .iter()
            .map(|metric| metric["name"].as_str().expect("metric name").to_owned())
            .collect::<BTreeSet<_>>();
        for sample in samples {
            let emitted_names = sample["metrics"]
                .as_object()
                .expect("sample metrics")
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>();
            assert_eq!(
                emitted_names, metric_names,
                "{id}: collector must emit exactly the declared metrics"
            );
        }

        for metric in operation["metrics"].as_array().expect("metrics") {
            let name = metric["name"].as_str().expect("metric name");
            let values = samples
                .iter()
                .map(|sample| {
                    (
                        point(&sample["dimensions"]),
                        sample["metrics"][name].as_u64().expect("counter value"),
                    )
                })
                .collect::<Vec<_>>();
            if let Some(max) = metric.get("budget").map(|budget| budget["max"].as_u64()) {
                let max = max.expect("budget.max");
                for (point, value) in &values {
                    assert!(
                        *value <= max,
                        "{id}.{name}: {value} at {point:?} exceeds budget {max}"
                    );
                }
            }
            for growth in metric["growth"].as_array().into_iter().flatten() {
                let dimension = growth["dimension"].as_str().expect("dimension");
                let bound = growth["bound"].as_str().expect("bound");
                if let Err(violation) = check_growth(&values, dimension, bound) {
                    panic!("{id}.{name} ({dimension}): {violation}; samples {values:?}");
                }
            }
        }
    }
}

#[test]
fn collector_is_deterministic_and_rejects_malformed_sha() {
    assert_eq!(
        evidence(),
        evidence(),
        "evidence must be identical across runs"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_raid-defense-sim"))
        .args(["--work-evidence", "--contract-sha256", "not-a-sha"])
        .output()
        .expect("simulator binary must run");
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "no evidence on a malformed digest"
    );
}

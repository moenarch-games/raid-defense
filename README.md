# Raid Defense

A deterministic Warcraft III-style maul/tower-defense experiment: build on a 2D gameplay grid while the battlefield is rendered as a 3D world.

## Current MVP

The playable slice is intentionally small but architectural rather than mocked:

- a 21×21 authoritative build grid;
- a protected Town Hall at the center;
- one raider gate on each edge of the map;
- seed-derived renewable forest tiles with minimum-distance spacing;
- Sawmills that harvest the nearest reachable stocked forest without a fixed harvest radius, buffering wood locally;
- Storage Houses that extend distributed settlement wood storage;
- **real people/carrier ECS entities** that move wood between producers, storage, and construction sites;
- Arrow and Cannon tower construction sites that become active only after workers deliver their material;
- three deterministic tower upgrade levels;
- maul-style route shaping without allowing complete path sealing;
- population capacity and progression-gated Houses;
- separate tracking of started waves and completed waves so progression cannot unlock early;
- deterministic daytime preparation followed by automatic night raids;
- fixed-point movement for raiders and workers plus deterministic projectile movement;
- raiders that seek the nearest reachable stocked storage and steal wood before Town Hall damage becomes necessary;
- a React Three Fiber client that renders buildings, people, cargo, forests, raiders, and projectiles from the authoritative Rust/WASM snapshot;
- GitHub Pages deployment with browser acceptance against the real WASM game.

## Architecture

```text
rust-kernels / collection-kernels
        SparseSet + SparseMap<T>
                 |
                 v
crates/raid-defense-core
        rules.rs          typed rule schema + validation
        default_rules.rs  standard balance profile
        ECS + systems     authoritative simulation
          |           |
          |           +--> crates/raid-defense-sim
          |                headless policy/balance runner
          v
crates/raid-defense-wasm
        versioned JSON translation only
                 |
                 v
visualization/
        3D rendering + player input only
```

`raid-defense-core` is the sole game authority. The browser and headless simulator never recompute production, carrier assignments, cargo transfers, construction completion, housing unlocks, day/night transitions, path legality, combat, or theft. They only submit normal commands and observe authoritative state/events.

### Rules are explicit simulation input

Ordinary gameplay tuning is centralized in `STANDARD_RULES` rather than scattered through systems. The typed `GameRules` profile covers economy, population/logistics, buildings, towers, raids, progression, and day/night cadence.

Every `GameState` owns one immutable rules profile. Systems consume that profile directly, and the active profile is fingerprinted into deterministic checksums. This means costs, health, production rates, forest regrowth, carrier capacity, House unlock timing, tower stats, raid scaling, or day length can be adjusted without rewriting the corresponding ECS systems.

Structural simulation contracts remain separate: grid dimensions, fixed-point representation, deterministic path-neighbor order, and stable tie-breaking are engine invariants rather than ordinary balance knobs.

For normal balance changes, edit `default_rules.rs` and the relevant acceptance expectations; do not modify an ECS system merely to change a cost, rate, threshold, health value, or scaling curve.

Custom profiles are validated before simulation starts, including cross-field invariants such as starting storage not exceeding capacity, House population growth not exceeding its capacity growth, non-negative active tower ranges, and non-zero cadence divisors. Invalid profiles fail closed instead of being silently clamped into another game.

`GameState::new(seed)` creates the standard game. Validated custom rule profiles are available for tests and future game modes. External JSON/TOML rule loading is intentionally not part of the core yet; the typed profile gives one authoritative configuration surface without adding format/versioning complexity before it is needed.

## Headless simulator

`raid-defense-sim` fast-forwards the real core without loading WASM or the renderer. It is intended for balance experiments, regression investigations, and comparing settlement strategies across many deterministic world seeds.

```bash
cargo run -p raid-defense-sim -- --seed 42 --ticks 6000 --policy balanced
cargo run -p raid-defense-sim -- --seed 100 --runs 32 --ticks 10000 --policy economy
```

Built-in policies are deliberately small decision layers:

- `passive` issues no player commands and measures the untouched simulation;
- `economy` prioritizes production, storage, and population before additional defense;
- `balanced` maintains an economy baseline while scaling towers with completed waves;
- `defense` prioritizes towers and upgrades after establishing minimal logistics.

Policies do not bypass game rules. Candidate actions are probed with `GameState::validate_command`, which returns exactly what `apply` would return without mutating or cloning the authoritative state, and only normal `Command` values are submitted to the real simulation. Reports include survival, waves, health, wood, entity counts, combat/economy throughput, theft/damage, the final deterministic checksum, and one `work seed=<seed> …` line per run.

### Deterministic work counters

The core exposes `GameState::work_counters()`, a `WorkCounters` value accumulated since the state was created. Counters are deterministic for the same seed, rules and commands, but they are instrumentation only: they never enter the checksum, snapshots, equality or gameplay. The simulator's `work` line reports them together with two policy counters:

| Counter | Meaning | Allowed scaling |
| --- | --- | --- |
| `flow_field_rebuilds` | Grid-wide navigation builds (one multi-source distance field per goal-cell set and topology). | Zero across ticks with unchanged topology; after a topology/goal change at most one build per goal set, independent of agent count. |
| `navigation_cells_visited` | Cells expanded by those builds. | At most one grid sweep per rebuild. |
| `path_follow_steps` | Per-agent next-step lookups against retained fields. | Linear in active agents. |
| `target_candidate_evaluations` | Raiders examined by tower targeting. | Follows the raiders in the cells covered by each tower's range square, not total raiders. |
| `logistics_candidate_searches` | Worker/forest/sawmill/storage/site candidates examined by logistics selection. | At most `2 · C` per assignment plus one sweep of the `C` relevant candidates (forests, sawmills, settlement storage, construction sites) per tick, however many workers idle. One exception: a construction hauler still re-selects its source storage on every movement step. |
| `policy_validation_probes` | Candidate commands the simulator policy validated. | Policy-dependent. |
| `world_clones` | Full game-state clones made while probing candidates. | Must stay `0` unless an explicit, reviewed O(N) clone contract is declared. |

Navigation is retained per goal-cell set inside each `GameState` and discarded whenever a building is inserted. Placement validation builds fields for the hypothetical blocked topology once per goal set, so its cost does not grow with the number of raiders or workers. Every retained answer is identical to the from-scratch breadth-first search (same neighbor order and tie-breaking), which is pinned by oracle unit tests and baseline checksums.

Idle-worker assignment builds a per-tick logistics candidate index the first time an idle worker needs a decision in a tick. One sweep records the open construction sites, the stocked settlement storage, the sawmills with free capacity or unreserved stock, and the forests with unreserved stock. Each reservation prunes an exhausted entry, so a worker with nothing assignable examines no candidates. Every decision applies the same filters and tie-breaking to the indexed candidates as a full scan would, which the per-tick checksum traces in `crates/raid-defense-core/tests/logistics_targeting_contracts.rs` pin.

`.performance/contract.json` (coding-tooling schemaVersion 2) declares the counters in two ways. Its `scenarios` keep them as non-blocking metrics of the representative journeys, where wall-clock time stays advisory. Its `operations` declare them as executable growth and budget contracts:

| Operation | Varied dimension | Declaration |
| --- | --- | --- |
| `raid-tick-navigation` | raiders (8, 16, 32) | `flow_field_rebuilds` and `navigation_cells_visited` constant, `path_follow_steps` linear |
| `tower-targeting` | raiders (4, 10, 16; only the four gate raiders are local) | `target_candidate_evaluations` constant |
| `worker-logistics` | workers (2, 12) over fixed stock | `logistics_candidate_searches` linear (two points, so search work per worker must not grow) |
| `idle-workers-exhausted-stock` | workers (2, 4, 8, 12), all idle | `logistics_candidate_searches` constant |
| `policy-journey` | ticks (1000, 2000, 4000) | `world_clones` budget max 0 |

The `performance:work` capability in `.coding-tooling.json` runs `scripts/performance-work.sh`. The script hashes the contract and prints `coding-tooling/work-evidence/v1` from `raid-defense-sim --work-evidence --contract-sha256 <sha256>`. `coding-tooling performance work --root .` evaluates that evidence, and `.performance/applicability.json` wires it as the `work-complexity` family. The checks cover the declared scale points only.

These shapes are enforced inside `cargo test` too. `crates/raid-defense-core/tests/scaling_contracts.rs` and `logistics_targeting_contracts.rs` hold the counter bounds. `crates/raid-defense-sim/tests/work_budget_contract.rs` checks the declarations, and `work_evidence_collector.rs` checks that the collector's evidence matches them and meets every bound. `crates/raid-defense-sim/tests/work_counters_report.rs` and `scripts/performance-smoke.sh` check reporting and byte-identical determinism.

## ECS direction

The current component boundary includes:

- `Transform` — authoritative fixed-point world position;
- `Health` — Town Hall, buildings, people, and raiders;
- `Attack` — instantiated tower/raider combat values;
- `Building` — grid occupancy and building kind;
- `Tower` — tower archetype and level, including construction state;
- `ResourceStorage` — Town Hall, Sawmill, Storage House, forest, and construction material storage;
- `ResourceProducer` — timed Sawmill output backed by renewable forest harvesting;
- `Housing` — population capacity;
- `Person` — carrier task, target, cargo, and capacity;
- `Raider` — raid/spawn identity plus storage target;
- `Movement` — deterministic cell-to-cell unit motion;
- `Projectile` — authoritative in-flight attacks.

Future resources, worker roles, effects, units, status effects, movement traits, destructible objects, and additional tower/raider families should be added as components/systems rather than growing entity-specific inheritance trees. Tunable behavior should be added through the rules schema rather than new system-local constants.

## Validation

Hosted validation promotes changes in this order:

1. `fast` — Rust format, Clippy, unit and adapter tests, including headless simulator determinism.
2. `integration` — deterministic economy/defense replay traces and transactional rejection checks.
3. `workflow` — frozen browser install, lint, release WASM generation, strict TypeScript, and Vite build.
4. `e2e` — Playwright against the real WASM simulation and 3D client.

GitHub Pages is the integrated playable surface after merge.

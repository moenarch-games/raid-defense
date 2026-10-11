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
| `logistics_candidate_searches` | Worker/forest/sawmill/storage/site candidates examined by logistics selection. | Reported separately from movement; the population scaling bound is a follow-up of #38. |
| `policy_validation_probes` | Candidate commands the simulator policy validated. | Policy-dependent. |
| `world_clones` | Full game-state clones made while probing candidates. | Must stay `0` unless an explicit, reviewed O(N) clone contract is declared. |

Navigation is retained per goal-cell set inside each `GameState` and discarded whenever a building is inserted. Placement validation builds fields for the hypothetical blocked topology once per goal set, so its cost does not grow with the number of raiders or workers. Every retained answer is identical to the from-scratch breadth-first search (same neighbor order and tie-breaking), which is pinned by oracle unit tests and baseline checksums.

The counters are declared in `.performance/contract.json` as non-blocking operation-count metrics. Their scaling shapes are enforced by `crates/raid-defense-core/tests/scaling_contracts.rs`, and their reporting and byte-identical determinism by `crates/raid-defense-sim/tests/work_counters_report.rs` and `scripts/performance-smoke.sh`.

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

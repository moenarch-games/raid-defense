# Repository agent guidance

`raid-defense` is a deterministic grid-based tower-defense/maul game built around an economy-versus-defense tradeoff. The repository has one authoritative game implementation: the Rust workspace under `crates/`.

## Authority

- `crates/raid-defense-core` owns ECS entity/component state, resources, population, housing, production, logistics, spending, theft, game rules, command validation, grid occupancy, pathfinding, deterministic simulation, replay, and checksums.
- `crates/raid-defense-wasm` is a translation boundary only. It may serialize and map values, but it must not own game rules.
- `visualization/` owns browser input, camera, 3D presentation, and explanatory reference content only. It must not recompute authoritative game outcomes.
- The gameplay plane is a 2D grid. Rendering may be fully 3D, but visual transforms must derive from authoritative Rust positions.

## Application and simulation boundary

- Use lightweight command/query separation only at the player/application boundary. Commands represent player intent such as placing or upgrading a structure or manually starting a wave.
- Deterministic simulation steps are not CQRS commands. The owner of the simulation clock must call `GameState::advance_tick` (or the direct WASM `advance_tick` adapter) rather than route live ticks through command serialization or a mediator/bus.
- Keep the legacy `AdvanceTick` command/DTO only as a compatibility and replay seam while older saved inputs and tools still depend on it. Do not build new hot paths on it.
- Pathfinding, ECS systems, production, logistics, combat, movement, forest regrowth, raid scheduling, and other per-tick work execute directly inside the Rust simulation.
- Read snapshots are presentation/query projections; they do not imply a separate asynchronous read store or eventual-consistency architecture.
- Do not introduce CQRS infrastructure into shared performance-oriented foundations such as tables, maps, charts, rendering, physics, or game-server/runtime internals merely for architectural uniformity.

## Game-rules architecture

- `crates/raid-defense-core/src/rules.rs` defines the typed, authoritative `GameRules` schema, validation, rule queries, and deterministic rules fingerprint. It must not become a second simulation implementation.
- `crates/raid-defense-core/src/default_rules.rs` is the single source of ordinary standard-profile balance values: costs, health, production, population/logistics capacity, tower level stats, raid scaling, unlock thresholds, and day/night cadence.
- Every `GameState` owns one immutable `GameRules` value. Simulation systems must read gameplay tuning from `self.rules`; do not introduce new balance literals, duplicated lookup tables, or progression formulas inside systems.
- `GameState::new` uses `STANDARD_RULES`. Use validated custom rules when testing or intentionally creating another balance profile.
- Rules are part of deterministic identity. If a rule can affect authoritative outcomes, it must be represented in `GameRules` and in its fingerprint/checksum binding.
- WASM/browser snapshots expose values from the active state's rules. Do not duplicate default costs or unlock thresholds in adapters or presentation code.
- Keep structural engine invariants separate from balance rules. Grid dimensions, fixed-point cell representation, stable iteration/tie-breaking, pathfinding neighbor order, and serialization authority are deterministic engine contracts rather than ordinary tuning knobs.
- Prefer adding a typed rule field over adding a new global constant when designers may plausibly tune the value later. Add a new structural engine constant only when changing it would alter representation/topology rather than balance.
- Rule profiles must validate fail-closed before a configurable game starts. Do not silently clamp malformed profiles into a different game.

## Economy and logistics invariants

- The Town Hall and Storage Houses are authoritative settlement storage nodes. `GameState::wood()` and capacity represent the aggregate authoritative settlement inventory; do not add browser/global shadow resource counters.
- Settlement spending must operate on authoritative stored wood in a deterministic order. Tower upgrades and ordinary building purchases consume settlement storage through Rust commands.
- Forest entities are persistent renewable resource nodes. Their stock regrows deterministically up to the configured per-tile capacity; depletion must not despawn or relocate the seeded forest tile.
- Sawmills do **not** harvest or create wood autonomously. Their configured throughput describes worker forestry batch size/work duration and their bounded local storage.
- Sawmills have no fixed forest harvest radius. Idle people may be assigned to a reachable stocked Forest only when a reachable Sawmill can accept the resulting batch. The person travels to the Forest, completes the authoritative forestry work interval, removes real Forest stock, and physically carries that raw wood to a Sawmill.
- People are real ECS entities with bounded cargo. After raw wood reaches local Sawmill storage, people physically carry it from Sawmills into reachable settlement storage before that wood becomes spendable.
- Tower placement creates an inactive construction site. Workers withdraw material from stocked settlement storage, haul it to the site, and only the Rust core activates the tower after the full build requirement has been delivered.
- Forest regrowth, forest selection, storage selection, worker assignment, pathfinding, forestry work, pickup, delivery, construction completion, and resource depletion are authoritative Rust/ECS behavior. Presentation may visualize them but must not predict or complete them independently.
- Combat kills do not create wood. Worker forestry plus logistics remain the economic authority.
- Raiders target the nearest reachable settlement storage that currently contains wood. They may retarget as storage changes; the Town Hall remains the fallback target, and Town Hall damage occurs only through authoritative raid resolution.
- Economic buildings participate in grid occupancy and path shaping just like defensive buildings.
- Building placement must preserve required raider routes and worker access to active logistics/construction tasks.
- Extend generic storage/production/logistics components when adding resources or producers rather than introducing unrelated special-case counters.

## Population and progression invariants

- The Town Hall provides the initial population capacity and initial people according to the active rules profile.
- Population capacity comes from authoritative `Housing` components.
- House availability is based on **completed** waves through the active rules profile; merely starting a threshold wave must not unlock it early.
- Building a House increases population capacity and introduces its configured people in the same authoritative command transaction.
- People/capacity must remain visible in snapshots/checksums because logistics throughput is gameplay state, not presentation.

## Day/night invariants

- Day/night timing, raid rally timing, raid spawn cadence, and automatic raid cadence are authoritative Rust rules, not browser timers.
- The standard profile reaches raid time after 600 peaceful simulation ticks, then enters a 50-tick rally (5 seconds at the browser's standard 100 ms tick cadence). Manual `StartWave` enters the same authoritative rally instead of spawning a raider immediately.
- Starting a rally recalls people toward the Town Hall. No new jobs are assigned during rally; people already carrying material may finish their current delivery before returning. The first raider spawns only when the full rally expires, and the rest of the wave enter on the authoritative spawn schedule.
- Rally is distinct from the active raid. The standard profile pauses production/carrier logistics only for the active raid schedule after rally; the rally itself remains live simulation so people can return. Natural forest regrowth remains authoritative world progression throughout.
- A raid remains active while either live raiders or scheduled raiders remain. Zero-live-raider gaps must not complete the wave, permit another raid, or resume paused economy systems.
- The day resets only after the full scheduled wave has completed.
- Presentation may display phase/countdown state but must not decide when rally starts or ends, when a wave starts, when a scheduled raider appears, when a wave completes, or whether economic systems advance.

## ECS and reuse

- Compose typed component stores inside `raid-defense-core` over reusable low-level kernels such as `collection-kernels::SparseMap<T>` and `SparseSet`.
- Do not make `ecs-lab` a runtime dependency; it is an experiment/reference harness. Promote reusable low-level primitives through `rust-kernels` instead.
- Prefer components and systems over entity-specific inheritance or monolithic per-entity structs as buildings, towers, people, raiders, projectiles, effects, resources, and units grow.
- Keep game-specific components and systems in this repository unless a genuinely cross-repository low-level primitive emerges.

## Determinism

- Every simulation outcome must be a pure consequence of initial seed, immutable rules, authoritative state, ordered player commands, and the exact sequence/count of deterministic simulation steps.
- Validate a command completely before mutating state. Rejected commands must leave state unchanged.
- Prefer integer or fixed-point representations for authoritative movement, logistics, economy, and combat rules.
- System iteration/order must be deterministic when it can affect outcomes; tie-break with stable entity identifiers where needed.
- Add replay or checksum coverage whenever a new component/system or rule can affect authoritative state.

## Save and resume invariants

- Browser persistence must not deserialize a snapshot into authoritative game state. Persist the initial seed plus the replay action log described in `docs/replay-contract.md` (accepted player actions stamped with their simulation tick, plus the `recorded_through_tick` cursor), then reconstruct through the real Rust/WASM engine.
- Simulation steps are not persisted as replay actions. Reconstruction must advance to each recorded tick exactly, and it should call the direct simulation-step API rather than serialize synthetic tick commands.
- Legacy saves containing `advance_ticks` runs or `advance_tick` command entries may be migrated for compatibility; do not emit new saves in that form.
- Persist an authoritative checksum with the save and fail closed on resume if replay does not reproduce that checksum.
- Rejected commands are not part of the replay because they must leave authoritative state unchanged.
- Save schema and WASM contract versions must be explicit. Incompatible or malformed saves must not be silently coerced into a different game.
- UI/settings persistence is separate from gameplay persistence and may contain presentation preferences only.

## Tower-defense invariants

- Building placement happens on the authoritative grid.
- Buildings may shape routes but must not remove all valid routes from any active spawn or moving raider to the Town Hall.
- Spawn gates and Town Hall cells are protected from ordinary building placement.
- Browser path previews, range displays, logistics displays, economy projections, and help/wiki text are advisory presentation; Rust remains authoritative for legality, production, transfer, spending, theft, movement, construction, and combat.

## Validation

- Preserve the hosted layer order: fast -> integration -> workflow -> e2e. Do not skip a failed required layer.
- A verified Markdown-only pull request may finish at the `fast` change-scope check. Missing, empty, renamed or non-Markdown diffs, main pushes and manual runs use all four tiers; a failing required code check is never bypassed.
- Keep cheap deterministic core checks ahead of browser or hosted acceptance.
- GitHub Pages is the primary integrated browser acceptance surface after merge.
- Do not treat missing, cancelled, or unavailable evidence as green.
- Browser acceptance must include a phone-sized touch viewport whenever interaction controls or battlefield input change.
- Start-menu changes must cover a complete new-game -> progress -> menu -> reload -> resume path so persistence evidence is bound to a real reconstructed game.

## UI

- The main battlefield should be the 3D game world, not explanatory dashboard cards.
- Keep economy/build/raid/progression controls and feedback close to the world state they affect.
- Render people, cargo, forests, storage, construction sites, and buildings from authoritative snapshots rather than maintaining browser-owned substitutes.
- Keep browser state interaction-focused; the Rust core remains the game-state authority.
- Building buttons arm presentation-only placement intent; they must not issue a build command against a previously selected tile. A translucent building ghost follows the current battlefield target, and the authoritative placement command is dispatched only when the player activates that target tile.
- A new game has no preselected build tile. Inspection selection and placement intent are separate presentation concepts; entering placement mode clears stale inspection selection rather than silently reusing it as the build location.
- The default route is the start menu. Menu, game, and settings screens should be URL-addressable with query state rather than hidden SPA-only state.
- `Resume` is available only when a compatible saved replay exists. `New game` must make replacing an existing save explicit rather than silently discarding it.
- Settings must remain presentation-only unless a future setting is added to the typed authoritative `GameRules` model deliberately.
- In-game wiki/help content is explanatory only. Do not encode legality or progression logic there, and do not duplicate tunable numeric rule values in static prose. Read live values from authoritative snapshots when a number is useful, or explain the mechanic without a number.
- Mobile controls must not depend on hover, tiny coordinate fields, or pixel-precise taps. Keep primary touch targets at least 44 CSS pixels tall/wide, respect safe-area insets, and keep build/raid actions reachable without covering the battlefield unnecessarily.
- The battlefield should support direct touch manipulation: tap selects or places the armed building, drag orbits, and pinch zooms. Camera gestures must not accidentally issue grid-selection or gameplay commands.
- Preserve a precise mobile fallback for choosing a neighboring grid cell so players are not forced to hit a small 3D tile accurately. That fallback may preview/place the same armed ghost, but it must remain optional rather than restoring tile-first building placement.

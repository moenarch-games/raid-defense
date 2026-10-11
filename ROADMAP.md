# Raid Defense roadmap

The roadmap is vertical: every milestone should leave GitHub Pages more playable while preserving deterministic Rust/ECS authority.

## A. Grid-maul foundation

- [x] Replace the earlier province/turn prototype with a real-time grid tower-defense model.
- [x] Keep gameplay on an integer 2D grid while exposing 3D-ready transforms.
- [x] Put authoritative entity/component state directly in `raid-defense-core`.
- [x] Use reusable `rust-kernels` sparse collection primitives rather than depending on `ecs-lab` as a game framework.
- [x] Add a central 3×3 town and four protected edge spawn gates.
- [x] Expand the opening map to a 21×21 build grid.
- [x] Add deterministic tower placement, occupancy, resource cost, and transactional rejection.
- [x] Allow maul-style path shaping while rejecting placements that seal a route.
- [x] Add fixed-point raider movement from all four edges toward the settlement.
- [x] Add tower targeting, damage, health, kills, and town damage.
- [x] Render the playable grid, town, towers, resources, and raiders in 3D through React Three Fiber.
- [x] Cover the real WASM/3D client with browser acceptance.

## B. Economy and physical logistics

- [x] Seed renewable forest resource tiles deterministically with blue-noise-like spacing while preserving required routes.
- [x] Treat forests as renewable patches: a fully depleted patch waits through its configured recovery duration, then returns at full per-tile capacity.
- [x] Distribute forestry workers across reachable stocked forest patches before stacking additional workers onto already-reserved patches.
- [x] Keep sawmills free of a fixed harvest radius: workers may gather from any reachable stocked patch when a reachable sawmill can accept the batch.
- [x] Keep sawmill output in a bounded local buffer until a person carries it into settlement storage.
- [x] Add storage houses as distributed wood stores.
- [x] Make raiders target the nearest reachable storage that contains wood and retarget when it is emptied.
- [x] Make tower placement create a construction site; workers must fetch wood from the nearest stocked storage and deliver it before the tower activates.
- [ ] Add explicit worker assignment/priorities once multiple simultaneous construction and economy jobs need player control.
- [ ] Tune forest recovery and worker distribution from simulator evidence if renewable supply erases too much map/economy pressure.
- [ ] Add further resources only after wood logistics remain legible at larger settlement scale.

## C. Tower-defense combat depth

- [x] Add tower archetypes with distinct range, fire rate, damage, and cost.
- [x] Add deterministic tower upgrades.
- [ ] Add tower selling.
- [x] Add projectile entities for attacks that should travel rather than resolve instantly.
- [ ] Add splash, slow, armor, resistances, and status-effect components/systems.
- [ ] Add clear range/path/build previews without moving legality into the browser.

## D. Waves and raiders

- [x] Add timed spawn schedules rather than one simultaneous raider per edge.
- [ ] Add larger waves and deterministic composition recipes.
- [ ] Add fast, tank, swarm, ranged, boss, and eventually flying raider archetypes.
- [ ] Add explicit wave completion, preparation windows, and escalating rewards.
- [x] Add resource-seeking raider behavior without turning pathfinding into browser state.
- [ ] Add attack behavior for non-storage buildings when tactical destruction becomes useful.

## E. ECS scale and performance

- [ ] Add projectile/effect pools as entity populations grow.
- [ ] Measure targeting, logistics, and movement system costs with representative large waves and worker populations. Deterministic work counters and scaling contracts for navigation, targeting and policy validation exist (#38); the logistics population bound and blocking budgets remain.
- [x] Introduce spatial-query acceleration only when measurements justify it: tower targeting uses a per-tick raider cell index and navigation fields are retained per goal set (#38).
- [ ] Keep deterministic replay/checksum coverage across component lifecycle and system ordering.
- [ ] Use ECS Lab to compare storage/system strategies, then promote only reusable low-level primitives through `rust-kernels`.

## F. 3D presentation

- [ ] Replace primitive meshes with reproducible tower, town, raider, projectile, worker, storage, forest, and terrain assets.
- [ ] Add animation and effects driven from authoritative events/state.
- [ ] Reuse pinned `3d-lab` procedural skeletal animation for worker/raider two-bone IK, foot placement/locking, pelvis correction, surface alignment, and later bounded motion warping instead of building browser-local IK.
- [ ] Source physical contact point/normal evidence from `physics-engine` where terrain or obstacle presentation needs it; pose correction must not alter authoritative grid movement, combat, logistics, economy, or replay truth.
- [ ] Add camera presets and mobile-friendly interaction without coupling camera state to simulation.
- [ ] Add terrain dressing outside the build plane while keeping the gameplay grid legible.
- [ ] Preserve a performant fallback path for browsers/devices that cannot run the richest rendering path.

## G. Game progression

- [ ] Add build menus, unlocks, tower tech trees, and wave goals.
- [ ] Add deterministic scenarios/maps with different grid shapes, gates, resource layouts, and town layouts.
- [ ] Add score/replay persistence only after the combat loop is strong.
- [ ] Add multiplayer/co-op authority boundaries only after deterministic local simulation is mature.

## H. Repository hardening

- [ ] Generate browser contract types from Rust instead of maintaining handwritten DTO types.
- [ ] Keep dependency revisions immutable and update them through validated PRs.
- [ ] Continue fast → integration → workflow → e2e promotion and Pages acceptance.
- [ ] Add convergence checks as the new ECS/game boundaries stabilize.

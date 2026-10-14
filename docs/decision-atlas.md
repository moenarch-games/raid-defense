# Raid Defense Decision Atlas

This document is the append-only product decision authority for Raid Defense.

Questions capture unresolved product choices. Decisions answer earlier questions explicitly. Existing IDs must not be renumbered, removed, or reused.

## 1. Game structure

### Question 1.1 — Victory model

What is the victory model for a normal game?

- **A — Finite defense:** a scenario has a defined final wave, probably culminating in a major boss/raid. Survive it and win.
- **B — Endless survival:** there is no normal victory. Raids escalate indefinitely until the settlement falls.
- **C — Finite victory + optional endless continuation:** each normal scenario has a clear final raid and victory state. After winning, the player may continue into increasingly difficult endless raids.

### Decision 1.2 — Finite victory with optional endless continuation

**Answers 1.1.**

Choose **C**.

Each normal Raid Defense scenario has a defined victory point and final raid. Winning completes the scenario. After victory, the player may continue the same settlement into optional endless raids with increasing difficulty.

Consequences:

- normal scenarios can be balanced toward a concrete completion target;
- final raids and bosses can provide scenario climax;
- endless mode extends the same run rather than replacing the primary victory model;
- progression, scoring, replay, and scenario acceptance can distinguish “scenario won” from “run ended”;
- continuing after victory must not revoke the already-earned scenario victory.

### Question 1.3 — Persistence between scenarios

How much gameplay state should persist when moving from one completed scenario to another?

- **A — Independent scenarios:** every scenario starts from a fresh settlement and standard starting capabilities. Completion may unlock access to more scenarios, but no gameplay power carries over.
- **B — Persistent campaign settlement:** resources, buildings, population, upgrades, or other substantial gameplay state carry from scenario to scenario.
- **C — Fresh settlements with horizontal meta-progression:** each scenario starts a fresh economy and settlement, while completing scenarios can unlock new maps, tower/building choices, or other strategic options without carrying the previous settlement itself forward.


### Decision 1.4 — Fresh settlements with horizontal meta-progression

**Answers 1.3.**

Choose **C**.

Each scenario starts with a fresh settlement, economy, buildings, resources, and population. Completing scenarios may unlock additional maps and strategic options for future runs, but the previous settlement itself does not carry forward.

Consequences:

- scenario balance remains self-contained and reproducible from its starting state;
- campaign progression expands the player's option space rather than accumulating settlement resources or infrastructure;
- persistent progression must not silently inject previous-run economy, buildings, population, or combat state into a new scenario;
- replay/checksum identity for a scenario can remain grounded in that scenario's seed, rules, unlock/loadout inputs, commands, and simulation steps.

### Question 1.5 — How should unlocked strategic options enter a scenario?

Once towers, buildings, or other strategic options have been unlocked through progression, how should they be available in a fresh scenario?

- **A — All unlocked options available:** every unlocked option is available during every compatible scenario.
- **B — Pre-scenario loadout:** before starting, the player chooses a bounded subset of unlocked towers/buildings/options to bring into that scenario.
- **C — Scenario-defined roster:** each scenario defines which unlocked options may be used, with little or no player loadout choice.


### Decision 1.6 — Scenario-defined strategic roster

**Answers 1.5.**

Choose **C**.

Each scenario defines the towers, buildings, and other strategic options that may be used there. Meta-progression may unlock options globally, but an unlock does not imply universal availability in every scenario.

Consequences:

- scenario designers can shape distinct strategic problems instead of balancing every scenario against the full unlocked catalog;
- scenario acceptance can name the intended roster explicitly;
- progression unlocks expand the set of possibilities available to future scenario definitions rather than creating a universal player loadout;
- scenario rules remain authoritative inputs and must determine availability rather than browser-only hiding.

### Question 1.7 — How should scenario maps be authored?

What should define the battlefield, resource layout, gates, and other major scenario structure?

- **A — Authored scenarios:** each scenario has a deliberately designed map and fixed major layout. Randomness may affect cosmetic details only.
- **B — Procedural scenarios:** scenario identity is mostly rules plus a seed; battlefield layout, resources, and routes are generated.
- **C — Authored structure with deterministic procedural variation:** each scenario defines its major map shape, gates, objectives, roster, and important landmarks, while a seed can vary bounded details such as forest placement, minor terrain dressing, or optional resource distribution without changing the scenario's core identity.


### Decision 1.8 — Authored scenarios with deterministic procedural variation

**Answers 1.7.**

Choose **C**.

Each scenario has an authored core identity: its major battlefield shape, gates, objectives, strategic roster, important landmarks, and other defining structure are deliberate scenario content. A deterministic seed may vary bounded details that do not erase that identity, such as forest placement, minor terrain dressing, or controlled resource distribution.

Consequences:

- scenarios can be designed and balanced around intentional strategic problems;
- replayability may come from deterministic bounded variation without turning every map into an unrelated procedural level;
- scenario-defining topology and objectives remain explicit authored inputs;
- procedural generation must respect scenario constraints and remain reproducible from the scenario definition plus seed;
- acceptance tests can pin both the authored invariants and deterministic seeded variation.

### Question 1.9 — What is the objective model for normal scenarios?

What should the player fundamentally be defending or trying to accomplish?

- **A — Always defend the Town Hall:** every normal scenario has the same primary objective: keep the central settlement/Town Hall alive through the final raid.
- **B — Fully variable objectives:** scenarios may replace settlement defense with substantially different primary objectives such as escorts, timed survival, protecting multiple targets, evacuation, or destruction objectives.
- **C — Stable settlement-defense core with scenario-specific secondary objectives:** keeping the settlement/Town Hall alive remains the universal primary condition, while scenarios may add meaningful secondary objectives, constraints, or optional goals without replacing the core game.


### Decision 1.10 — Stable settlement-defense core with scenario-specific secondary objectives

**Answers 1.9.**

Choose **C**.

Keeping the settlement and Town Hall alive remains the universal primary objective of normal Raid Defense scenarios. Individual scenarios may add meaningful secondary objectives, constraints, or optional goals, but they do not replace the core settlement-defense game.

Consequences:

- every scenario remains recognizably Raid Defense;
- secondary objectives may change priorities without requiring a different primary game loop;
- scenario victory still requires surviving the authored final raid;
- optional goals can drive score, rewards, unlocks, or challenge variants without making them the sole authority for ordinary victory;
- the core simulation and acceptance model can retain one universal settlement-loss condition while scenario definitions add extra tracked objectives.

### Question 1.11 — How should partial raid damage affect a run?

If raiders get through defenses but do not destroy the Town Hall, what should the normal consequence be?

- **A — Mostly binary survival:** losses before Town Hall destruction are limited; surviving the wave largely resets the immediate danger, so failure pressure is concentrated on the final Town Hall loss.
- **B — Persistent attrition:** raids can permanently destroy buildings, kill population, steal resources, and leave the settlement substantially weakened across later waves.
- **C — Recoverable attrition:** raids can steal resources, damage buildings, and cause meaningful setbacks that persist into later waves, but core progression should generally remain recoverable rather than allowing one imperfect wave to create an unrecoverable death spiral.


### Decision 1.12 — Recoverable attrition

**Answers 1.11.**

Choose **C**.

Raids may cause meaningful persistent setbacks—such as stolen resources, damaged buildings, interrupted logistics, or population pressure—but surviving a wave should generally leave the run recoverable. One imperfect defense should not usually create an unavoidable death spiral.

Consequences:

- partial failure matters and can alter later strategic choices;
- recovery systems must exist for damage and economic setbacks that are expected to persist;
- scenario balance should distinguish “under pressure” from “functionally doomed”;
- deterministic simulation and scenario acceptance should cover recovery after imperfect waves, not only flawless wins and Town Hall destruction;
- severe losses may still make a run difficult, but ordinary attrition should preserve meaningful player agency.

### Question 1.13 — How should damaged settlement structures recover?

After a raid damages but does not destroy buildings, what should restore them?

- **A — Automatic repair between raids:** surviving structures repair themselves during preparation time, possibly at a fixed rate and without direct player allocation.
- **B — Fully explicit repair economy:** repairs require player-issued repair orders, worker time, and resource spending; unrepaired damage persists indefinitely.
- **C — Worker-driven repair with bounded automation:** repairs consume real resources and worker/logistics capacity, but eligible repair work can be assigned automatically according to player priorities rather than requiring repetitive per-building orders.

## 2. Day/night settlement cycle

### Decision 2.1 — Prepare, survive, recover

Daytime is for worker activity, harvesting, hauling, construction, deployment and expansion. A distinct pre-raid rally leads into nighttime raids, when workers retreat into settlement buildings rather than continue exposed outdoor jobs. The following morning gives the player a chance to assess stolen resources and damage, rebuild, redirect workers and revise the defensive layout before the next night.

Preserve authoritative phase timing and the existing pause of ordinary logistics during active raids. Losses are meaningful but recoverable under Decision 1.12. This does not answer the repair-allocation Question 1.13.

### Question 2.2 — Which buildings shelter workers overnight?

- **A — Central shelter:** everyone returns to the Town Hall.
- **B — Distributed shelter:** people use reachable Houses or the Town Hall, making shelter placement matter.
- **C — Assigned shelter:** the player designates each worker's building or garrison.

### Decision 2.3 — Distributed overnight shelters

**Answers 2.2.**

Choose **B — Distributed shelter** for the initial game.

During the pre-raid rally, workers automatically retreat to reachable completed Houses or the Town Hall and shelter there at night rather than continue exposed outdoor jobs. Building placement should matter to retreat distance and settlement safety; no per-worker manual shelter assignment is required initially. Workers return to ordinary logistics work during the next day. Use the existing physical-worker, authoritative pathing and day/night systems; do not teleport workers or model a browser-only refuge.

This is **not** approval to staff or man combat towers with people in the initial game. That remains a possible later progression feature (Question 4.4). The exact automatic refuge selection, handling of unavailable/full shelters and stragglers, and presentation must remain consistent with the existing authoritative occupancy, population and rally rules rather than creating a competing shelter subsystem.

## 3. Raiding and resource recovery

### Decision 3.1 — Raiders steal cargo and must escape

Raiders pursue reachable resource stockpiles, take only available material into bounded personal cargo and attempt to leave the battlefield once their bags are full. Resource theft is not complete merely because a raider reached storage: cargo remains physically at risk during the retreat. Killing a loaded raider before escape makes the stolen goods retrievable; a successful escape removes the carried goods from the settlement's recoverable resources.

A kill never generates fresh resources. Distinguish goods still stored, carried by raiders, dropped for recovery and successfully escaped. Raider targeting and retreat remain deterministic simulation behavior.

### Question 3.2 — How does dropped stolen cargo return to storage?

- **A — Automatic recovery:** a killed carrier's cargo goes straight back to settlement storage.
- **B — Physical recovery:** cargo drops where the raider died and workers collect and deliver it.
- **C — Player collection:** cargo drops in the world and requires an explicit player pickup action.

### Decision 3.3 — Worker recovery of dropped raid loot

**Answers 3.2.**

Choose **B — Physical recovery**.

When a loaded raider is killed before successfully escaping, its carried stolen resources are dropped at the death location as persistent world cargo. Workers must physically travel to the drop, pick up resources within their bounded carrying capacity, and transport them to reachable settlement storage before the resources become spendable again. Recovery is an ordinary worker/logistics task during the daytime; it does not auto-credit the settlement, require player clicking the drop, or generate new resources.

Consequences:

- the theft, retreat, death-drop, pickup, delivery, and successful-escape transitions must preserve resource quantities exactly;
- dropping loot during a raid creates visible recovery work for the next preparation phase rather than restoring resources instantly;
- dropped cargo must survive phase changes, save/resume/replay, and temporarily unavailable storage without disappearing or being counted as available settlement stock;
- worker allocation, navigation, delivery, presentation, and rules/checksum identity remain authoritative in the Rust core, consistent with Decisions 2.1 and 3.1.

## 4. Threats and staged defense complexity

### Decision 4.1 — Different raiders demand different counters

Raider archetypes must create recognizable tactical differences, with different tower or defensive types particularly effective against different threats. Progression should introduce stronger and specialized enemies, including climactic boss-scale raids, rather than relying on numbers and health scaling alone. Towers and threat types must communicate their tactical roles in actual play.

This builds on the current Arrow/Cannon towers and Basic/Advanced raider profiles without treating the presence of two named types as sufficient evidence of counterplay.

### Decision 4.2 — Complexity grows through the economy

Begin with a comprehensible resource economy and arrow towers that do not require finite ammunition. Later phases introduce stronger defenses that consume ammunition produced by the settlement, creating a consequential economic cost and the possibility of supply shortages. More elaborate routes and defenses follow the initial understandable combat/economy loop rather than overwhelming the opening.

The timing, concrete ammunition chain and affected tower roster remain scenario/progression design choices, not browser-only unlock logic.

### Question 4.3 — How should produced ammunition reach and supply towers?

- **A — Local stock:** workers deliver finite ammunition to each consuming tower.
- **B — Supply network:** towers draw from nearby connected depots.
- **C — Shared pool:** a centralized settlement ammunition stock is consumed by towers without deliveries.

### Question 4.4 — Later personnel-operated towers

Some more advanced towers might eventually need actual personnel stationed inside to operate. Should personnel-operated towers become a later gameplay mechanic, and if so, what tradeoff should exist between defense staffing and available daytime workers? This is exploratory and **not** an approved current feature; ordinary towers remain automated without requiring workers.

## 5. Movement, logistics and fortification

### Decision 5.1 — Efficient worker routes compete with difficult raider routes

Layout decisions must create a tradeoff between short, productive worker paths and long, defensible raider approaches. Resource buildings, defenses and terrain/path features all affect this balance; optimizing one side must not erase the strategic importance of the other. Preserve legal worker access and raider routes.

Potential later construction tools include roads, walls, barricades, gates, bridges, tunnels, mines and traps. These examples are not individually approved as first-release requirements.

### Question 5.2 — How should roads affect raiders?

- **A — Shared speed:** raiders and workers both benefit from roads, making efficient logistics corridors tempting invasion routes.
- **B — Worker advantage:** workers move faster on roads; raiders may traverse them but receive no speed bonus.
- **C — Restricted corridors:** worker roads can be made inaccessible to raiders through controlled access.

### Question 5.3 — Which advanced route-shaping construction should be introduced first?

Walls and barricades, controllable gates, traps and mines, and bridges or tunnels are candidates. Their order and concrete mechanics are open; do not treat this list as a promise to implement them all.

## 6. Expansion and frontier exposure

### Decision 6.1 — Expansion trades richer resources for harder defense

Within a scenario, the usable settlement territory gradually expands. New territory exposes additional resource sites and, over time, different resource kinds. Expansion also creates a larger defensive footprint and additional viable directions or fronts for raids, so additional production carries a real protection and logistics cost.

Keep this distinct from Decision 1.4: a completed scenario still starts a fresh settlement in the next scenario. How territory is acquired and how the authoritative grid/map represents expansion remain open.

### Question 6.2 — What unlocks the next territory during a run?

- **A — Player claim:** spend resources to claim an adjacent region during preparation.
- **B — Exploration:** send workers/scouts to discover and secure a new area.
- **C — Wave milestone:** surviving specified raids automatically opens additional territory.

### Decision 6.3 — Exploration-driven territorial expansion

**Answers 6.2.**

Choose **B — Exploration**.

During a scenario, expanding the usable settlement territory requires a physical expedition: the player sends workers or scouts to investigate and secure adjacent land. Merely paying for an adjacent tile or surviving a specified wave does not automatically expand the settlement. Exploration must have real travel and exposure consequences, grounded in authoritative unit movement, terrain, and day/night behavior.

Secured territory makes additional resource sites accessible and expands the frontier the player must protect, including credible additional raid approach angles. The gradual risk-versus-reward expansion in Decision 6.1 remains essential. Respect authored scenario identity and deterministic bounded variation (Decision 1.8), replay identity, valid routes, and Rust-owned world state.

This establishes **exploration as the acquisition mechanic** but does not yet decide whether existing workers or specialized scouts undertake expeditions, or what turns discovered land into secured/buildable territory. Those consequential conditions must not be silently selected by implementation.

### Question 6.4 — What secures an explored region?

After an expedition discovers new territory, what is required before the settlement may exploit resources and build there?

- **A — Expedition reaches the frontier:** reaching and surveying the region secures it; the challenge is the journey and ensuing exposure.
- **B — Establish an outpost:** the player must deliver resources and construct a forward claim outpost before the region becomes usable; the challenge is sustained logistics.
- **C — Remove hostile control:** the region is secured by defeating or driving off local enemies; the challenge is offensive combat beyond the established base.

### Decision 6.5 — Securing explored territory requires a constructed outpost

**Answers 6.4.**

Choose **B — Establish an outpost**.

Exploration reveals the adjacent frontier, but discovery by itself does not make that territory available for settlement construction or resource exploitation. The player must establish a forward claim outpost there: workers physically deliver construction resources, complete the outpost, and thereby secure the region for use. Both the expedition and the outpost's logistics have real path, time, and exposure consequences. The claim must not complete instantly on a click, on entering the region, or automatically after a wave.

The outpost links new usable land and resources to the settlement while extending the area that must be defended and exposing credible new raid approaches (Decisions 6.1 and 6.3). Reuse the authoritative economy, construction and worker movement principles; preserve valid routes, authored scenario structure, deterministic replay/checksums, and a playable day/night flow.

The specific exploring unit type (workers, scouts or an expedition group), the effect of later outpost destruction, and outpost tuning remain unresolved. Do not infer a new mandatory scout class or arbitrary regional ownership rules from this Decision.

### Question 6.6 — Who performs a frontier expedition?

Once the player chooses an adjacent region to explore, which kind of unit must travel there?

- **A — Existing workers:** send one or more normal workers, temporarily sacrificing resource production and hauling capacity.
- **B — Dedicated scouts:** recruit specialized non-production scouting units, creating a distinct exploration investment.
- **C — Organized expedition group:** assemble a group that includes explorers and escort personnel, making safe exploration depend on a managed expedition instead of an individual worker.


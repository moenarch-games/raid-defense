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

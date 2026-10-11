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

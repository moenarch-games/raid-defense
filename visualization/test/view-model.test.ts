import { describe, expect, test } from "bun:test";
import { entity, snapshot } from "./fixtures";

// Seam acceptance for moritzbrantner/raid-defense#22: pure view-model helpers derive selected
// building and settlement presentation from an authoritative snapshot, without React.
//
// Contract: `visualization/src/game/viewModel.ts` exports
//   findSelectedBuilding(snapshot, cell: CellView | null): EntityView | undefined
//   describeSelection(snapshot, cell: CellView | null, placement: PlacementChoice | null): string
//   deriveSettlementStatus(snapshot): SettlementStatusView (fields asserted below; extras allowed)
// where PlacementChoice is { kind: "sawmill" | "storage_house" | "house" } |
// { kind: "tower"; archetype: "arrow" | "cannon" }. Strings match the pre-refactor screen.

const viewModel = await import("../src/game/viewModel");
const { findSelectedBuilding, describeSelection, deriveSettlementStatus } = viewModel;

const arrowSite = entity("tower", 2, 2, {
  tower_archetype: "arrow",
  tower_level: 0,
  stored_wood: 10,
  wood_capacity: 25,
});
const cannon = entity("tower", 7, 2, {
  tower_archetype: "cannon",
  tower_level: 2,
  attack_damage: 30,
  upgrade_cost: 40,
});
const sawmill = entity("sawmill", 4, 4, { stored_wood: 4, wood_capacity: 24 });
const storage = entity("storage_house", 3, 2, { stored_wood: 12, wood_capacity: 160 });
const forest = entity("forest", 5, 10, { stored_wood: 60, wood_capacity: 80 });
const house = entity("house", 6, 6, { housing_capacity: 2 });
const carrier = entity("person", 8, 8, { cargo_wood: 4, cargo_capacity: 4 });
const idlePerson = entity("person", 9, 9);
const raiderA = entity("raider", 0, 10);
const raiderB = entity("raider", 1, 10);
const projectile = entity("projectile", 7, 3, { tower_archetype: "cannon" });
const townHall = entity("town_hall", 10, 10);

const world = snapshot({
  people: 2,
  population_capacity: 4,
  entities: [
    townHall,
    arrowSite,
    cannon,
    sawmill,
    storage,
    forest,
    house,
    carrier,
    idlePerson,
    raiderA,
    raiderB,
    projectile,
  ],
});

describe("findSelectedBuilding", () => {
  test("returns the building or resource on the selected cell", () => {
    expect(findSelectedBuilding(world, { x: 7, z: 2 })).toBe(cannon);
    expect(findSelectedBuilding(world, { x: 2, z: 2 })).toBe(arrowSite);
    expect(findSelectedBuilding(world, { x: 4, z: 4 })).toBe(sawmill);
    expect(findSelectedBuilding(world, { x: 3, z: 2 })).toBe(storage);
    expect(findSelectedBuilding(world, { x: 5, z: 10 })).toBe(forest);
    expect(findSelectedBuilding(world, { x: 6, z: 6 })).toBe(house);
  });

  test("ignores empty cells, mobile units and a missing selection", () => {
    expect(findSelectedBuilding(world, null)).toBeUndefined();
    expect(findSelectedBuilding(world, { x: 12, z: 12 })).toBeUndefined();
    expect(findSelectedBuilding(world, { x: 8, z: 8 })).toBeUndefined();
    expect(findSelectedBuilding(world, { x: 0, z: 10 })).toBeUndefined();
  });
});

describe("describeSelection", () => {
  test("placement intent wins over any inspected cell", () => {
    expect(describeSelection(world, { x: 7, z: 2 }, { kind: "tower", archetype: "cannon" })).toBe(
      "Placing Cannon tower · choose a tile on the battlefield",
    );
    expect(describeSelection(world, null, { kind: "storage_house" })).toBe(
      "Placing Storage house · choose a tile on the battlefield",
    );
    expect(describeSelection(world, null, { kind: "house" })).toBe(
      "Placing House · choose a tile on the battlefield",
    );
  });

  test("describes each inspected building from authoritative snapshot values", () => {
    expect(describeSelection(world, { x: 2, z: 2 }, null)).toBe(
      "Arrow tower construction · 10/25 wood delivered",
    );
    expect(describeSelection(world, { x: 7, z: 2 }, null)).toBe("Cannon tower · L2 · 30 dmg");
    expect(describeSelection(world, { x: 4, z: 4 }, null)).toBe(
      "Sawmill · gathered wood waiting 4/24",
    );
    expect(describeSelection(world, { x: 3, z: 2 }, null)).toBe("Storage house · 12/160 wood");
    expect(describeSelection(world, { x: 5, z: 10 }, null)).toBe(
      "Forest · 60/80 wood · regrows +1 every 20 ticks",
    );
    expect(describeSelection(world, { x: 6, z: 6 }, null)).toBe("House · +2 population capacity");
  });

  test("forest regrowth text follows the snapshot rules rather than fixed prose", () => {
    const tuned = snapshot({
      forest_regrowth_amount: 3,
      forest_regrowth_interval_ticks: 7,
      entities: [forest],
    });
    expect(describeSelection(tuned, { x: 5, z: 10 }, null)).toBe(
      "Forest · 60/80 wood · regrows +3 every 7 ticks",
    );
  });

  test("empty and missing selections", () => {
    expect(describeSelection(world, { x: 12, z: 12 }, null)).toBe("Empty build cell");
    expect(describeSelection(world, { x: 8, z: 8 }, null)).toBe("Empty build cell");
    expect(describeSelection(world, null, null)).toBe("No tile selected");
  });
});

describe("deriveSettlementStatus", () => {
  test("counts settlement entities and haulers from the snapshot", () => {
    expect(deriveSettlementStatus(world)).toMatchObject({
      wood: 120,
      people: 2,
      populationCapacity: 4,
      hauling: 1,
      wave: 0,
      completedWaves: 0,
      raiders: 2,
      forests: 1,
      storageHouses: 1,
      sawmills: 1,
      constructionSites: 1,
      towers: 1,
      projectiles: 1,
    });
  });

  test("formats the authoritative day, rally and night cycle", () => {
    expect(deriveSettlementStatus(snapshot({ day_ticks_remaining: 600 })).cycleLabel).toBe(
      "Day · 01:00",
    );
    expect(deriveSettlementStatus(snapshot({ day_ticks_remaining: 7 })).cycleLabel).toBe(
      "Day · 00:01",
    );
    expect(
      deriveSettlementStatus(snapshot({ is_rallying: true, raid_rally_ticks_remaining: 50 }))
        .cycleLabel,
    ).toBe("Rally · 00:05");
    expect(deriveSettlementStatus(snapshot({ is_night: true, wave: 3 })).cycleLabel).toBe(
      "Night · Wave 3",
    );
  });

  test("derives clamped Town Hall and storage meters", () => {
    const status = deriveSettlementStatus(
      snapshot({ town_health: 125, town_max_health: 250, wood: 600, wood_capacity: 500 }),
    );
    expect(status.townHealthPercent).toBe(50);
    expect(status.woodPercent).toBe(100);
    expect(deriveSettlementStatus(snapshot({ town_health: 0 })).townHealthPercent).toBe(0);
  });

  test("reports house unlock progress from authoritative completed waves", () => {
    expect(deriveSettlementStatus(snapshot({ completed_waves: 3 })).houseLockLabel).toBe(
      "Houses unlock after 10 completed waves (3/10)",
    );
    expect(
      deriveSettlementStatus(snapshot({ house_unlock_completed_waves: 4, completed_waves: 1 }))
        .houseLockLabel,
    ).toBe("Houses unlock after 4 completed waves (1/4)");
    expect(deriveSettlementStatus(snapshot({ houses_unlocked: true })).houseLockLabel).toBe(
      "Houses unlocked",
    );
  });

  test("allows starting a raid only during a live day", () => {
    expect(deriveSettlementStatus(snapshot()).canStartWave).toBe(true);
    expect(deriveSettlementStatus(snapshot({ is_rallying: true })).canStartWave).toBe(false);
    expect(deriveSettlementStatus(snapshot({ is_night: true })).canStartWave).toBe(false);
    expect(deriveSettlementStatus(snapshot({ town_health: 0 })).canStartWave).toBe(false);
  });
});

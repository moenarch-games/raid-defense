// Pure presentation derivations over an authoritative snapshot. Nothing here decides game
// outcomes: every number and rule-driven label is read from the snapshot the Rust core produced.

import type {
  CellView,
  EntityView,
  RaidDefenseEvent,
  SnapshotView,
  TowerArchetype,
} from "../simulationTypes";

/** Browser cadence used to present tick counts as wall-clock time. */
export const TICK_INTERVAL_MS = 100;

/** Presentation-only placement intent armed by a build button. */
export type PlacementChoice =
  | { kind: "sawmill" }
  | { kind: "storage_house" }
  | { kind: "house" }
  | { kind: "tower"; archetype: TowerArchetype };

export type SettlementStatusView = {
  wood: number;
  woodCapacity: number;
  woodPercent: number;
  people: number;
  populationCapacity: number;
  hauling: number;
  cycleLabel: string;
  wave: number;
  completedWaves: number;
  raiders: number;
  forests: number;
  storageHouses: number;
  sawmills: number;
  constructionSites: number;
  towers: number;
  projectiles: number;
  townHealth: number;
  townMaxHealth: number;
  townHealthPercent: number;
  houseLockLabel: string;
  canStartWave: boolean;
  waveActionLabel: string;
};

export type BuildOption = { label: string; available: boolean };

export type BuildOptionsView = {
  sawmill: BuildOption;
  storageHouse: BuildOption;
  house: BuildOption;
  arrowTower: BuildOption;
  cannonTower: BuildOption;
};

export type TowerUpgradeView = { label: string; available: boolean };

const BUILDING_KINDS = new Set(["tower", "sawmill", "storage_house", "house", "forest"]);

export function towerName(archetype: TowerArchetype) {
  return archetype === "arrow" ? "Arrow tower" : "Cannon tower";
}

export function placementName(placement: PlacementChoice) {
  if (placement.kind === "tower") return towerName(placement.archetype);
  if (placement.kind === "storage_house") return "Storage house";
  if (placement.kind === "house") return "House";
  return "Sawmill";
}

export function formatCycleTimer(ticks: number) {
  const totalSeconds = Math.ceil((ticks * TICK_INTERVAL_MS) / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

function clampPercent(value: number, max: number) {
  return Math.max(0, Math.min(100, (value / max) * 100));
}

export function clampCellCoordinate(value: number, size: number) {
  return Math.max(0, Math.min(size - 1, value));
}

/** The building or resource node occupying the selected cell, if any. */
export function findSelectedBuilding(
  snapshot: SnapshotView,
  cell: CellView | null,
): EntityView | undefined {
  if (!cell) return undefined;
  return snapshot.entities.find(
    (entity) =>
      BUILDING_KINDS.has(entity.kind) && entity.cell.x === cell.x && entity.cell.z === cell.z,
  );
}

export function describeSelection(
  snapshot: SnapshotView,
  cell: CellView | null,
  placement: PlacementChoice | null,
) {
  if (placement) return `Placing ${placementName(placement)} · choose a tile on the battlefield`;
  const building = findSelectedBuilding(snapshot, cell);
  switch (building?.kind) {
    case "tower":
      if (building.tower_archetype) {
        return building.tower_level === 0
          ? `${towerName(building.tower_archetype)} construction · ${building.stored_wood}/${building.wood_capacity} wood delivered`
          : `${towerName(building.tower_archetype)} · L${building.tower_level} · ${building.attack_damage} dmg`;
      }
      break;
    case "sawmill":
      return `Sawmill · gathered wood waiting ${building.stored_wood}/${building.wood_capacity}`;
    case "storage_house":
      return `Storage house · ${building.stored_wood}/${building.wood_capacity} wood`;
    case "forest":
      return `Forest · ${building.stored_wood}/${building.wood_capacity} wood · regrows +${snapshot.forest_regrowth_amount} every ${snapshot.forest_regrowth_interval_ticks} ticks`;
    case "house":
      return `House · +${building.housing_capacity} population capacity`;
  }
  return cell ? "Empty build cell" : "No tile selected";
}

export function deriveSettlementStatus(snapshot: SnapshotView): SettlementStatusView {
  const count = (predicate: (entity: EntityView) => boolean) =>
    snapshot.entities.filter(predicate).length;
  const cycleLabel = snapshot.is_night
    ? `Night · Wave ${snapshot.wave}`
    : snapshot.is_rallying
      ? `Rally · ${formatCycleTimer(snapshot.raid_rally_ticks_remaining)}`
      : `Day · ${formatCycleTimer(snapshot.day_ticks_remaining)}`;

  return {
    wood: snapshot.wood,
    woodCapacity: snapshot.wood_capacity,
    woodPercent: clampPercent(snapshot.wood, snapshot.wood_capacity),
    people: snapshot.people,
    populationCapacity: snapshot.population_capacity,
    hauling: count((entity) => entity.kind === "person" && entity.cargo_wood > 0),
    cycleLabel,
    wave: snapshot.wave,
    completedWaves: snapshot.completed_waves,
    raiders: count((entity) => entity.kind === "raider"),
    forests: count((entity) => entity.kind === "forest"),
    storageHouses: count((entity) => entity.kind === "storage_house"),
    sawmills: count((entity) => entity.kind === "sawmill"),
    constructionSites: count((entity) => entity.kind === "tower" && entity.tower_level === 0),
    towers: count((entity) => entity.kind === "tower" && entity.tower_level > 0),
    projectiles: count((entity) => entity.kind === "projectile"),
    townHealth: snapshot.town_health,
    townMaxHealth: snapshot.town_max_health,
    townHealthPercent: clampPercent(snapshot.town_health, snapshot.town_max_health),
    houseLockLabel: snapshot.houses_unlocked
      ? "Houses unlocked"
      : `Houses unlock after ${snapshot.house_unlock_completed_waves} completed waves (${snapshot.completed_waves}/${snapshot.house_unlock_completed_waves})`,
    canStartWave: !snapshot.is_night && !snapshot.is_rallying && snapshot.town_health > 0,
    waveActionLabel: snapshot.is_rallying
      ? `Raid in ${formatCycleTimer(snapshot.raid_rally_ticks_remaining)}`
      : "Start raid",
  };
}

/** Labels and affordability of the build buttons, read from snapshot costs and storage. */
export function deriveBuildOptions(snapshot: SnapshotView): BuildOptionsView {
  const standing = snapshot.town_health > 0;
  const affordable = (cost: number) => standing && snapshot.wood >= cost;
  return {
    sawmill: { label: `Sawmill · ${snapshot.sawmill_cost}w`, available: affordable(snapshot.sawmill_cost) },
    storageHouse: {
      label: `Storage · ${snapshot.storage_house_cost}w`,
      available: affordable(snapshot.storage_house_cost),
    },
    house: {
      label: snapshot.houses_unlocked
        ? `House · ${snapshot.house_cost}w`
        : `House · unlocks after ${snapshot.house_unlock_completed_waves} waves`,
      available: snapshot.houses_unlocked && affordable(snapshot.house_cost),
    },
    arrowTower: {
      label: `Arrow · ${snapshot.arrow_tower_cost}w`,
      available: affordable(snapshot.arrow_tower_cost),
    },
    cannonTower: {
      label: `Cannon · ${snapshot.cannon_tower_cost}w`,
      available: affordable(snapshot.cannon_tower_cost),
    },
  };
}

/** Upgrade affordance for the tower on the selected cell. */
export function deriveTowerUpgrade(snapshot: SnapshotView, cell: CellView | null): TowerUpgradeView {
  const building = findSelectedBuilding(snapshot, cell);
  const tower = building?.kind === "tower" ? building : undefined;
  const label =
    tower?.tower_level === 0
      ? "Awaiting materials"
      : tower?.upgrade_cost === null
        ? "Max level"
        : `Upgrade${tower?.upgrade_cost ? ` · ${tower.upgrade_cost}w` : ""}`;
  const available =
    tower !== undefined &&
    tower.tower_level > 0 &&
    tower.upgrade_cost !== null &&
    snapshot.wood >= tower.upgrade_cost &&
    snapshot.town_health > 0;
  return { label, available };
}

/** Whether a simulation step produced something the player should hear about. */
export function isConsequentialTick(event: RaidDefenseEvent | null) {
  return (
    event?.type === "tick_advanced" &&
    (event.wood_produced > 0 ||
      event.wood_delivered > 0 ||
      event.towers_completed > 0 ||
      event.wood_stolen > 0 ||
      event.kills > 0 ||
      event.town_damage > 0 ||
      event.completed_wave !== null)
  );
}

export function describeEvent(event: RaidDefenseEvent | null) {
  if (!event) {
    return "Command accepted.";
  }

  switch (event.type) {
    case "tower_construction_started":
      return `${towerName(event.archetype)} construction started at ${event.cell.x}, ${event.cell.z}. Workers must haul ${event.wood_required} wood to the site.`;
    case "sawmill_built":
      return `Sawmill built at ${event.cell.x}, ${event.cell.z}. People now gather wood from forests, carry it to the sawmill, then haul the mill stock into settlement storage.`;
    case "storage_house_built":
      return `Storage house built at ${event.cell.x}, ${event.cell.z} with capacity for ${event.wood_capacity} wood.`;
    case "house_built":
      return `House built at ${event.cell.x}, ${event.cell.z}. ${event.people_added} people arrived; capacity is now ${event.population_capacity}.`;
    case "tower_upgraded":
      return `${towerName(event.archetype)} upgraded to level ${event.level} for ${event.wood_cost} wood.`;
    case "wave_started":
      return `Wave ${event.wave} called. Workers have ${formatCycleTimer(event.rally_ticks)} to finish carrying materials and return to the Town Hall before raiders enter.`;
    case "tick_advanced": {
      const consequences = [
        event.wood_produced > 0 ? `${event.wood_produced} wood gathered from forests` : null,
        event.wood_delivered > 0 ? `${event.wood_delivered} wood delivered` : null,
        event.towers_completed > 0
          ? `${event.towers_completed} tower${event.towers_completed === 1 ? "" : "s"} completed`
          : null,
        event.wood_stolen > 0 ? `${event.wood_stolen} wood stolen` : null,
        event.kills > 0 ? `${event.kills} kill${event.kills === 1 ? "" : "s"}` : null,
        event.town_damage > 0 ? `${event.town_damage} Town Hall damage` : null,
        event.completed_wave !== null ? `wave ${event.completed_wave} completed` : null,
      ].filter(Boolean);
      return consequences.length > 0
        ? `Tick ${event.tick}: ${consequences.join(" · ")}.`
        : `Tick ${event.tick}.`;
    }
  }
}

export function describeError(code: string) {
  const labels: Record<string, string> = {
    out_of_bounds: "That cell is outside the build grid.",
    cell_occupied: "That cell is occupied by a resource, building, or moving unit.",
    protected_cell: "That cell is reserved for the Town Hall or an edge spawn gate.",
    path_blocked: "That building would seal a required raider or worker route.",
    insufficient_wood: "Settlement storage does not contain enough wood for that action.",
    no_forest_in_range: "No reachable stocked forest is currently available for that sawmill.",
    house_locked: "Houses unlock after you complete the first 10 waves.",
    no_tower: "There is no completed tower on the selected cell to upgrade.",
    max_tower_level: "That tower is already at the maximum level.",
    raiders_still_active: "Finish the current raid or rally before starting another one.",
    game_over: "The Town Hall has fallen.",
    invalid_command_json: "The browser produced an invalid command envelope.",
  };
  return labels[code] ?? `Command rejected: ${code}.`;
}

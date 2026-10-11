// Browser harness for the `useRaidDefenseSession` acceptance seam (moritzbrantner/raid-defense#22).
//
// Served by the Vite dev server at /e2e/harness/session.html. It mounts the session hook in a
// real React root WITHOUT the game screen and drives it with a fake simulation client, so
// session lifetime, ticking, the command path and feedback events are observable independently
// of the rendered UI. It is acceptance infrastructure: changing the fake's semantics requires
// independent acceptance re-verification.

import { createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import type {
  DispatchResponse,
  RaidDefenseCommand,
  RaidDefenseEvent,
  SnapshotView,
} from "../../src/simulationTypes";

type TickEvent = Extract<RaidDefenseEvent, { type: "tick_advanced" }>;
type Session = {
  snapshot: SnapshotView | null;
  feedback: string;
  issue: (command: RaidDefenseCommand) => DispatchResponse | null;
};
type SessionOptions = {
  createClient: () => Promise<FakeSimulationClient>;
  tickIntervalMs: number;
};
type UseSession = (options: SessionOptions) => Session;

type CallRecord =
  | { method: "snapshot" }
  | { method: "advanceTick" }
  | { method: "dispatch"; command: RaidDefenseCommand };

export type HarnessState = {
  mounted: boolean;
  createClientCalls: number;
  snapshotNull: boolean;
  snapshotTick: number | null;
  snapshotWood: number | null;
  snapshotTownHealth: number | null;
  feedback: string | null;
  clientTick: number;
  advanceTickCalls: number;
  dispatchedCommands: RaidDefenseCommand[];
  observedSnapshotTicks: number[];
};

function makeSnapshot(tick: number, wood: number, townHealth: number): SnapshotView {
  return {
    contract_version: 10,
    seed: "24301",
    tick,
    wood,
    wood_capacity: 500,
    wave: 0,
    completed_waves: 0,
    day_ticks_remaining: 600 - tick,
    raid_rally_ticks_remaining: 0,
    is_rallying: false,
    is_night: false,
    people: 2,
    population_capacity: 2,
    houses_unlocked: false,
    town_health: townHealth,
    town_max_health: 250,
    grid_width: 21,
    grid_height: 21,
    forest_tile_count: 18,
    forest_tile_wood: 80,
    forest_regrowth_amount: 1,
    forest_regrowth_interval_ticks: 20,
    sawmill_cost: 40,
    sawmill_output: 4,
    sawmill_interval_ticks: 20,
    sawmill_local_wood_capacity: 24,
    storage_house_cost: 50,
    storage_house_wood_capacity: 160,
    house_cost: 60,
    house_unlock_completed_waves: 10,
    house_population_capacity: 2,
    person_carry_capacity: 4,
    raid_rally_ticks: 50,
    automatic_raids: true,
    pause_economy_during_raids: true,
    arrow_tower_cost: 25,
    cannon_tower_cost: 45,
    max_tower_level: 3,
    checksum: `fake-${tick}-${wood}-${townHealth}`,
    entities: [],
  };
}

function quietTick(tick: number): TickEvent {
  return {
    type: "tick_advanced",
    tick,
    shots: 0,
    impacts: 0,
    kills: 0,
    wood_produced: 0,
    wood_picked_up: 0,
    wood_delivered: 0,
    towers_completed: 0,
    wood_stolen: 0,
    town_damage: 0,
    completed_wave: null,
  };
}

/** Minimal deterministic stand-in for RaidDefenseSimulationClient. */
class FakeSimulationClient {
  readonly calls: CallRecord[] = [];
  tick = 0;
  wood = 120;
  townHealth = 250;
  readonly queuedTickEvents: Array<Partial<TickEvent>> = [];

  snapshot(): SnapshotView {
    this.calls.push({ method: "snapshot" });
    return makeSnapshot(this.tick, this.wood, this.townHealth);
  }

  advanceTick(): DispatchResponse {
    this.calls.push({ method: "advanceTick" });
    this.tick += 1;
    const extra = this.queuedTickEvents.shift() ?? {};
    return {
      contract_version: 10,
      ok: true,
      event: { ...quietTick(this.tick), ...extra, type: "tick_advanced", tick: this.tick },
      error: null,
      snapshot: makeSnapshot(this.tick, this.wood, this.townHealth),
    };
  }

  dispatch(command: RaidDefenseCommand): DispatchResponse {
    this.calls.push({ method: "dispatch", command });
    if (command.type === "advance_tick") {
      // Legacy compatibility seam: still answer, but the call is recorded so acceptance can
      // prove the live clock does not route ticks through command dispatch.
      this.tick += 1;
      return {
        contract_version: 10,
        ok: true,
        event: quietTick(this.tick),
        error: null,
        snapshot: makeSnapshot(this.tick, this.wood, this.townHealth),
      };
    }
    const reject = (code: string): DispatchResponse => ({
      contract_version: 10,
      ok: false,
      event: null,
      error: { code },
      snapshot: makeSnapshot(this.tick, this.wood, this.townHealth),
    });
    const accept = (event: RaidDefenseEvent): DispatchResponse => ({
      contract_version: 10,
      ok: true,
      event,
      error: null,
      snapshot: makeSnapshot(this.tick, this.wood, this.townHealth),
    });

    switch (command.type) {
      case "place_storage_house":
        if (Math.abs(command.x - 10) <= 1 && Math.abs(command.z - 10) <= 1) {
          return reject("protected_cell");
        }
        this.wood -= 50;
        return accept({
          type: "storage_house_built",
          entity: 900,
          cell: { x: command.x, z: command.z },
          wood_cost: 50,
          wood_capacity: 160,
        });
      case "start_wave":
        return accept({ type: "wave_started", wave: 1, raiders: 3, rally_ticks: 50 });
      default:
        return reject("unsupported_by_fake");
    }
  }
}

let useSession: UseSession | null = null;
let root: Root | null = null;
let container: HTMLElement | null = null;
let client: FakeSimulationClient | null = null;
let latest: Session | null = null;
let createClientCalls = 0;
let observedSnapshotTicks: number[] = [];
let pendingResolve: (() => void) | null = null;

function Probe({ options }: { options: SessionOptions }) {
  const session = (useSession as UseSession)(options);
  latest = session;
  const tick = session.snapshot?.tick ?? null;
  useEffect(() => {
    if (tick !== null) observedSnapshotTicks.push(tick);
  }, [tick]);
  return null;
}

const harness = {
  mount(config: { tickIntervalMs?: number; deferClient?: boolean; failWith?: string } = {}) {
    if (root) throw new Error("harness already mounted");
    client = new FakeSimulationClient();
    latest = null;
    createClientCalls = 0;
    observedSnapshotTicks = [];
    pendingResolve = null;
    const fake = client;
    const options: SessionOptions = {
      tickIntervalMs: config.tickIntervalMs ?? 25,
      createClient: () => {
        createClientCalls += 1;
        if (config.failWith) return Promise.reject(new Error(config.failWith));
        if (!config.deferClient) return Promise.resolve(fake);
        return new Promise<FakeSimulationClient>((resolve) => {
          pendingResolve = () => resolve(fake);
        });
      },
    };
    container = document.createElement("div");
    document.getElementById("harness-root")?.append(container);
    root = createRoot(container);
    root.render(createElement(Probe, { options }));
  },
  resolveClient() {
    if (!pendingResolve) throw new Error("no deferred client is pending");
    pendingResolve();
    pendingResolve = null;
  },
  unmount() {
    root?.unmount();
    root = null;
    container?.remove();
    container = null;
  },
  issue(command: RaidDefenseCommand) {
    if (!latest) throw new Error("session hook has not rendered");
    const response = latest.issue(command);
    return response === null ? null : { ok: response.ok, error: response.error?.code ?? null };
  },
  queueTickEvent(event: Partial<TickEvent>) {
    client?.queuedTickEvents.push(event);
  },
  setTownHealth(value: number) {
    if (client) client.townHealth = value;
  },
  state(): HarnessState {
    const calls = client?.calls ?? [];
    return {
      mounted: root !== null,
      createClientCalls,
      snapshotNull: latest?.snapshot == null,
      snapshotTick: latest?.snapshot?.tick ?? null,
      snapshotWood: latest?.snapshot?.wood ?? null,
      snapshotTownHealth: latest?.snapshot?.town_health ?? null,
      feedback: latest?.feedback ?? null,
      clientTick: client?.tick ?? 0,
      advanceTickCalls: calls.filter((call) => call.method === "advanceTick").length,
      dispatchedCommands: calls.flatMap((call) => (call.method === "dispatch" ? [call.command] : [])),
      observedSnapshotTicks: [...observedSnapshotTicks],
    };
  },
};

declare global {
  interface Window {
    __raidDefenseSessionHarness?: typeof harness;
    __raidDefenseSessionHarnessError?: string;
  }
}

// The seam named by the acceptance contract. A dynamic import keeps a missing seam observable as
// a harness error instead of a blank page.
import("../../src/game/useRaidDefenseSession")
  .then((module: { useRaidDefenseSession?: unknown }) => {
    if (typeof module.useRaidDefenseSession !== "function") {
      throw new Error("src/game/useRaidDefenseSession must export a useRaidDefenseSession hook");
    }
    useSession = module.useRaidDefenseSession as UseSession;
    window.__raidDefenseSessionHarness = harness;
  })
  .catch((error: unknown) => {
    window.__raidDefenseSessionHarnessError =
      error instanceof Error ? error.message : String(error);
  });

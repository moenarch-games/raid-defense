import { expect, test, type Page } from "@playwright/test";
import type { RaidDefenseCommand } from "../src/simulationTypes";
import type { HarnessState } from "./harness/sessionHarness";

// Seam acceptance for moritzbrantner/raid-defense#22: session lifetime, ticking, the single
// command path and feedback events are testable WITHOUT the rendered game screen.
//
// Contract: `visualization/src/game/useRaidDefenseSession.ts` exports
//   useRaidDefenseSession(options?: {
//     createClient?: () => Promise<{ snapshot(); advanceTick(); dispatch(command) }>;
//     tickIntervalMs?: number; // live cadence, default 100 ms
//   }): { snapshot: SnapshotView | null; feedback: string; issue(command): DispatchResponse | null }
// The default createClient creates the real RaidDefenseSimulationClient.

const INITIALIZING = "Initializing deterministic defense grid…";
const READY =
  "People gather trees, carry raw wood to sawmills, haul mill stock into settlement storage, and physically deliver tower materials.";

async function openHarness(page: Page) {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto("/e2e/harness/session.html");
  await expect
    .poll(
      async () =>
        page.evaluate(() =>
          window.__raidDefenseSessionHarness
            ? "ready"
            : (window.__raidDefenseSessionHarnessError ?? "loading"),
        ),
      { message: "useRaidDefenseSession seam must load in the browser harness", timeout: 10_000 },
    )
    .toBe("ready");
  expect(pageErrors).toEqual([]);
}

function mount(
  page: Page,
  config: { tickIntervalMs?: number; deferClient?: boolean; failWith?: string } = {},
) {
  return page.evaluate((value) => window.__raidDefenseSessionHarness!.mount(value), config);
}

function state(page: Page): Promise<HarnessState> {
  return page.evaluate(() => window.__raidDefenseSessionHarness!.state());
}

function issue(page: Page, command: RaidDefenseCommand) {
  return page.evaluate((value) => window.__raidDefenseSessionHarness!.issue(value), command);
}

async function advanceTickCallsStayAt(page: Page, ms = 250) {
  const before = (await state(page)).advanceTickCalls;
  await page.waitForTimeout(ms);
  expect((await state(page)).advanceTickCalls).toBe(before);
  return before;
}

test.beforeEach(async ({ page }) => {
  await openHarness(page);
});

test("session creates one simulation client and publishes its initial snapshot", async ({ page }) => {
  await mount(page, { deferClient: true });
  await expect.poll(async () => (await state(page)).createClientCalls).toBe(1);

  let current = await state(page);
  expect(current.createClientCalls).toBe(1);
  expect(current.snapshotNull).toBe(true);
  expect(current.feedback).toBe(INITIALIZING);
  expect(current.advanceTickCalls).toBe(0);
  // A command before the client exists is ignored rather than thrown or queued.
  expect(await issue(page, { type: "start_wave" })).toBeNull();

  await page.evaluate(() => window.__raidDefenseSessionHarness!.resolveClient());
  await expect.poll(async () => (await state(page)).snapshotNull).toBe(false);
  current = await state(page);
  expect(current.feedback).toBe(READY);
  expect(current.createClientCalls).toBe(1);
  expect(current.observedSnapshotTicks[0]).toBe(0);
});

test("live clock advances the simulation directly and publishes each tick snapshot", async ({ page }) => {
  await mount(page, { tickIntervalMs: 25 });

  await expect.poll(async () => (await state(page)).advanceTickCalls).toBeGreaterThanOrEqual(4);
  const running = await state(page);
  const ticks = running.observedSnapshotTicks;
  expect(ticks.length).toBeGreaterThanOrEqual(3);
  for (let index = 1; index < ticks.length; index += 1) {
    expect(ticks[index]).toBeGreaterThan(ticks[index - 1]);
  }
  // Ticks are deterministic simulation steps, not CQRS commands.
  expect(running.dispatchedCommands.filter((command) => command.type === "advance_tick")).toEqual([]);

  // Quiet ticks never overwrite the current feedback.
  expect(running.feedback).toBe(READY);
});

test("consequential ticks become feedback events while quiet ticks keep the last message", async ({ page }) => {
  await mount(page, { tickIntervalMs: 25 });
  await expect.poll(async () => (await state(page)).snapshotNull).toBe(false);

  await page.evaluate(() =>
    window.__raidDefenseSessionHarness!.queueTickEvent({ wood_delivered: 3, kills: 1 }),
  );
  await expect
    .poll(async () => (await state(page)).feedback)
    .toMatch(/^Tick \d+: 3 wood delivered · 1 kill\.$/);
  const message = (await state(page)).feedback;
  await page.waitForTimeout(200);
  expect((await state(page)).feedback).toBe(message);
});

test("player commands use one dispatch path and update snapshot and feedback", async ({ page }) => {
  await mount(page, { tickIntervalMs: 25 });
  await expect.poll(async () => (await state(page)).snapshotNull).toBe(false);

  const placed = await issue(page, { type: "place_storage_house", x: 3, z: 2 });
  expect(placed).toEqual({ ok: true, error: null });
  await expect
    .poll(async () => (await state(page)).feedback)
    .toBe("Storage house built at 3, 2 with capacity for 160 wood.");
  await expect.poll(async () => (await state(page)).snapshotWood).toBe(70);

  const rejected = await issue(page, { type: "place_storage_house", x: 10, z: 10 });
  expect(rejected).toEqual({ ok: false, error: "protected_cell" });
  await expect
    .poll(async () => (await state(page)).feedback)
    .toBe("That cell is reserved for the Town Hall or an edge spawn gate.");
  expect((await state(page)).snapshotWood).toBe(70);

  await issue(page, { type: "start_wave" });
  await expect
    .poll(async () => (await state(page)).feedback)
    .toBe(
      "Wave 1 called. Workers have 00:05 to finish carrying materials and return to the Town Hall before raiders enter.",
    );

  expect((await state(page)).dispatchedCommands).toEqual([
    { type: "place_storage_house", x: 3, z: 2 },
    { type: "place_storage_house", x: 10, z: 10 },
    { type: "start_wave" },
  ]);
});

test("the live clock stops once the Town Hall has fallen", async ({ page }) => {
  await mount(page, { tickIntervalMs: 25 });
  await expect.poll(async () => (await state(page)).advanceTickCalls).toBeGreaterThanOrEqual(2);

  await page.evaluate(() => window.__raidDefenseSessionHarness!.setTownHealth(0));
  await expect.poll(async () => (await state(page)).snapshotTownHealth).toBe(0);
  await advanceTickCallsStayAt(page);
  const stopped = await state(page);
  expect(stopped.snapshotTick).toBe(stopped.clientTick);
});

test("unmounting disposes the session and stops ticking", async ({ page }) => {
  await mount(page, { tickIntervalMs: 25 });
  await expect.poll(async () => (await state(page)).advanceTickCalls).toBeGreaterThanOrEqual(2);

  await page.evaluate(() => window.__raidDefenseSessionHarness!.unmount());
  expect((await state(page)).mounted).toBe(false);
  await advanceTickCallsStayAt(page);
});

test("a client that resolves after unmount is never ticked", async ({ page }) => {
  await mount(page, { tickIntervalMs: 25, deferClient: true });
  await page.evaluate(() => window.__raidDefenseSessionHarness!.unmount());
  await page.evaluate(() => window.__raidDefenseSessionHarness!.resolveClient());

  expect(await advanceTickCallsStayAt(page)).toBe(0);
});

test("a failed session start reports the error as feedback without ticking", async ({ page }) => {
  await mount(page, {
    tickIntervalMs: 25,
    failWith: "No compatible saved game is available to resume.",
  });

  await expect
    .poll(async () => (await state(page)).feedback)
    .toBe("No compatible saved game is available to resume.");
  const failed = await state(page);
  expect(failed.snapshotNull).toBe(true);
  expect(await advanceTickCallsStayAt(page)).toBe(0);
});

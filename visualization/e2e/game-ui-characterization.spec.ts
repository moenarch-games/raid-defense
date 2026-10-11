import { expect, test, type Page } from "@playwright/test";

// Characterization acceptance for moritzbrantner/raid-defense#22.
// These journeys pin the observable game-screen behavior that the building-block refactor must
// preserve. They pass on the pre-refactor baseline and complement app-screens, mobile-controls,
// timed-wave, start-menu and save-contract specs (which already pin placement, construction,
// upgrades, raids, rally, touch sizing and persistence).

const GAME_URL = "/?screen=game&new=1&seed=24301";

async function openGame(page: Page) {
  await page.goto(GAME_URL);
  await expect(page.getByTestId("raid-defense-game")).toBeVisible();
  await expect(page.getByTestId("wood-value")).toHaveText("120");
}

async function inspectCell(page: Page, x: number, z: number) {
  await page.getByTestId("cell-x").fill(String(x));
  await page.getByTestId("cell-z").fill(String(z));
  await expect(page.getByTestId("mobile-selected-cell")).toHaveText(`${x},${z}`);
}

test("session start shows the authoritative opening state and evidence footer", async ({ page }) => {
  await openGame(page);

  await expect(page.getByTestId("event-feedback")).toHaveText(
    "People gather trees, carry raw wood to sawmills, haul mill stock into settlement storage, and physically deliver tower materials.",
  );
  await expect(page.getByTestId("event-feedback")).toHaveAttribute("role", "status");
  await expect(page.getByTestId("event-feedback")).toHaveAttribute("aria-live", "polite");

  const footer = page.locator("footer");
  await expect(footer).toContainText("Seed 24301");
  await expect(footer).toContainText("Map 21×21");
  await expect(footer).toContainText("Contract v10");
  await expect(page.getByTestId("projectile-count")).toHaveText("Projectiles 0");
  await expect(page.getByTestId("checksum")).toContainText("Checksum ");
});

test("battlefield shell frames the 3D world with settlement health, storage and hints", async ({ page }) => {
  await openGame(page);

  const battlefield = page.getByTestId("world-3d");
  await expect(battlefield).toHaveAttribute("aria-label", "3D defense grid");
  await expect(battlefield.locator("canvas")).toHaveCount(1);
  await expect(battlefield).toContainText("Town Hall 250/250");
  await expect(battlefield).toContainText("Settlement wood 120/500");
  await expect(battlefield.getByTestId("house-lock-state")).toBeVisible();
  await expect(battlefield.getByTestId("mobile-world-hint")).toHaveText(
    "Tap a tile to inspect · drag to orbit · pinch to zoom",
  );

  // The settlement status strip and wave control live in the top bar, not in the battlefield.
  await expect(page.locator("header").getByTestId("wood-value")).toBeVisible();
  await expect(battlefield.getByTestId("wood-value")).toHaveCount(0);
  await expect(page.locator("header").getByTestId("start-wave")).toHaveText("Start raid");
  await expect(page.locator("header").getByTestId("cycle-timer")).toContainText("Day ·");
  // Build controls live in the command dock outside the battlefield.
  await expect(battlefield.getByTestId("build-sawmill")).toHaveCount(0);
  await expect(page.getByTestId("mobile-command-dock").getByTestId("build-sawmill")).toBeVisible();
});

test("live ticking advances authoritative simulation state while the game is open", async ({ page }) => {
  await openGame(page);

  const initialChecksum = await page.getByTestId("checksum").textContent();
  const initialTimer = await page.getByTestId("cycle-timer").textContent();
  await expect
    .poll(async () => page.getByTestId("cycle-timer").textContent(), { timeout: 3_000 })
    .not.toBe(initialTimer);
  await expect
    .poll(async () => page.getByTestId("checksum").textContent(), { timeout: 3_000 })
    .not.toBe(initialChecksum);
});

test("leaving the game disposes the session so simulation time stops advancing", async ({ page }) => {
  await openGame(page);
  // Let some simulation time pass so a persisted summary exists.
  await page.waitForTimeout(1_500);
  await page.getByTestId("return-to-menu").click();
  await expect(page.getByTestId("start-menu")).toBeVisible();
  await expect(page.getByTestId("raid-defense-game")).toHaveCount(0);

  const readTick = () =>
    page.evaluate(() => {
      const raw = window.localStorage.getItem("raid-defense.save-summary.v1");
      return raw ? (JSON.parse(raw) as { tick: number }).tick : null;
    });
  const tickAtExit = await readTick();
  expect(tickAtExit).not.toBeNull();
  await page.waitForTimeout(2_000);
  expect(await readTick()).toBe(tickAtExit);
});

test("inspection selection describes empty cells and forests without arming placement", async ({ page }) => {
  await openGame(page);

  await expect(page.getByTestId("selected-building")).toHaveText("No tile selected");
  await expect(page.locator(".cell-controls .eyebrow")).toHaveText("Selected grid cell");
  await expect(page.getByTestId("upgrade-tower")).toBeDisabled();
  await expect(page.getByTestId("upgrade-tower")).toHaveText("Upgrade");

  await inspectCell(page, 3, 3);
  await expect(page.getByTestId("selected-building")).toHaveText("Empty build cell");
  await expect(page.getByTestId("place-selected-cell")).toHaveCount(0);

  await inspectCell(page, 5, 10);
  await expect(page.getByTestId("selected-building")).toHaveText(
    /^Forest · \d+\/80 wood · regrows \+1 every 20 ticks$/,
  );

  // Clearing a coordinate clears the inspection selection.
  await page.getByTestId("cell-x").fill("");
  await expect(page.getByTestId("mobile-selected-cell")).toHaveText("No tile selected");
  await expect(page.getByTestId("selected-building")).toHaveText("No tile selected");

  // Out-of-range coordinates are clamped to the authoritative grid size.
  await page.getByTestId("cell-x").fill("99");
  await page.getByTestId("cell-z").fill("4");
  await expect(page.getByTestId("mobile-selected-cell")).toHaveText("20,4");
  await expect(page.getByTestId("cell-east")).toBeDisabled();
  await expect(page.getByTestId("cell-west")).toBeEnabled();
  await expect(page.getByTestId("selected-building")).toHaveText("Empty build cell");

  // Inspection never issues a gameplay command.
  await expect(page.getByTestId("wood-value")).toHaveText("120");
  await expect(page.getByTestId("storage-count")).toHaveText("0");
});

test("arming placement clears a stale inspection selection and Escape cancels it", async ({ page }) => {
  await openGame(page);

  await inspectCell(page, 3, 3);
  await page.getByTestId("build-arrow-tower").click();
  await expect(page.getByTestId("build-arrow-tower")).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByTestId("build-sawmill")).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByTestId("mobile-selected-cell")).toHaveText("No tile selected");
  await expect(page.getByTestId("cell-x")).toHaveValue("");
  await expect(page.locator(".cell-controls .eyebrow")).toHaveText("Placement mode");
  await expect(page.getByTestId("selected-building")).toHaveText(
    "Placing Arrow tower · choose a tile on the battlefield",
  );
  await expect(page.getByTestId("event-feedback")).toHaveText(
    "Arrow tower selected. Move over the battlefield and click or tap the tile where it should be placed. Press Escape to cancel.",
  );

  await page.keyboard.press("Escape");
  await expect(page.getByTestId("build-arrow-tower")).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByTestId("event-feedback")).toHaveText("Building placement cancelled.");
  await expect(page.getByTestId("selected-building")).toHaveText("No tile selected");
  await expect(page.getByTestId("mobile-world-hint")).toHaveText(
    "Tap a tile to inspect · drag to orbit · pinch to zoom",
  );
  await expect(page.getByTestId("construction-count")).toHaveText("0");
  await expect(page.getByTestId("wood-value")).toHaveText("120");
});

test("an accepted placement disarms placement mode and inspects the placed building", async ({ page }) => {
  await openGame(page);

  await page.getByTestId("build-storage-house").click();
  await inspectCell(page, 3, 2);
  await expect(page.getByTestId("place-selected-cell")).toHaveText("Place at 3,2");
  await page.getByTestId("place-selected-cell").click();

  await expect(page.getByTestId("event-feedback")).toHaveText(
    "Storage house built at 3, 2 with capacity for 160 wood.",
  );
  await expect(page.getByTestId("build-storage-house")).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByTestId("place-selected-cell")).toHaveCount(0);
  await expect(page.getByTestId("mobile-selected-cell")).toHaveText("3,2");
  await expect(page.getByTestId("cell-x")).toHaveValue("3");
  await expect(page.getByTestId("cell-z")).toHaveValue("2");
  await expect(page.getByTestId("selected-building")).toHaveText(/^Storage house · \d+\/160 wood$/);
});

test("wave control reflects rally countdown and is disabled until the raid completes", async ({ page }) => {
  await openGame(page);

  await expect(page.getByTestId("start-wave")).toBeEnabled();
  await page.getByTestId("start-wave").click();
  await expect(page.getByTestId("start-wave")).toHaveText(/^Raid in 00:0\d$/);
  await expect(page.getByTestId("start-wave")).toBeDisabled();
  await expect(page.getByTestId("event-feedback")).toHaveText(
    "Wave 1 called. Workers have 00:05 to finish carrying materials and return to the Town Hall before raiders enter.",
  );
  await expect(page.getByTestId("start-wave")).toHaveText("Start raid", { timeout: 12_000 });
  await expect(page.getByTestId("cycle-timer")).toHaveText("Night · Wave 1");
  await expect(page.getByTestId("start-wave")).toBeDisabled();
});

import { describe, expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

// Architecture acceptance for moritzbrantner/raid-defense#22: the game screen is composed from
// focused Raid Defense building blocks, App is a thin composition, and the session hook is the
// single owner of the simulation client, the live clock and the command path.
//
// These checks are structural (imports, ownership, forbidden calls), not line counts.

const visualization = join(import.meta.dir, "..");
const src = join(visualization, "src");
const gameDir = join(src, "game");

function read(path: string) {
  return readFileSync(path, "utf8");
}

function stripComments(code: string) {
  return code.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
}

function sourceFiles(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    return /\.(ts|tsx)$/.test(entry.name) ? [path] : [];
  });
}

function importsOf(code: string) {
  return [...code.matchAll(/(?:import|export)\s[^;]*?from\s+["']([^"']+)["']/g)].map(
    (match) => match[1],
  );
}

function exportsComponent(code: string, name: string) {
  return new RegExp(
    `export\\s+(?:default\\s+)?(?:function|const|class)\\s+${name}\\b|export\\s*\\{[^}]*\\b${name}\\b[^}]*\\}`,
  ).test(code);
}

const BUILDING_BLOCKS = [
  "SettlementStatus",
  "BattlefieldPanel",
  "SelectedCellControls",
  "BuildActions",
  "WaveControl",
  "GameFeedback",
] as const;

const SESSION_HOOK = join(gameDir, "useRaidDefenseSession.ts");
const VIEW_MODEL = join(gameDir, "viewModel.ts");
const APP = join(src, "App.tsx");

describe("semantic building blocks", () => {
  for (const name of BUILDING_BLOCKS) {
    test(`${name} is its own module under src/game`, () => {
      const path = join(gameDir, `${name}.tsx`);
      expect(existsSync(path)).toBe(true);
      expect(exportsComponent(read(path), name)).toBe(true);
    });
  }

  test("building blocks receive data and actions instead of owning the simulation", () => {
    for (const name of BUILDING_BLOCKS) {
      const path = join(gameDir, `${name}.tsx`);
      expect(existsSync(path)).toBe(true);
      const code = stripComments(read(path));
      expect(importsOf(code).some((spec) => spec.endsWith("simulationClient"))).toBe(false);
      expect(importsOf(code).some((spec) => spec.endsWith("useRaidDefenseSession"))).toBe(false);
      expect(code).not.toMatch(/\.dispatch\s*\(/);
      expect(code).not.toMatch(/\.advanceTick\s*\(/);
      expect(code).not.toMatch(/\bset(?:Interval|Timeout)\s*\(/);
    }
  });

  test("no universal dashboard or card system replaces game concepts", () => {
    const names = sourceFiles(gameDir).map((path) => path.slice(gameDir.length + 1));
    expect(names.filter((name) => /(?:^|\/)(?:Dashboard|Card|Panel|Widget)\.tsx$/.test(name))).toEqual(
      [],
    );
  });
});

describe("thin App composition", () => {
  test("App wires the session hook and the building blocks", () => {
    const code = stripComments(read(APP));
    expect(importsOf(code).some((spec) => /(?:^|\/)game\/useRaidDefenseSession(?:\.ts)?$/.test(spec))).toBe(
      true,
    );
    expect(code).toMatch(/\buseRaidDefenseSession\s*\(/);
    for (const name of BUILDING_BLOCKS) {
      expect(code).toMatch(new RegExp(`<${name}\\b`));
    }
  });

  test("App does not own the simulation client, clock, commands or 3D scene", () => {
    const code = stripComments(read(APP));
    expect(importsOf(code).some((spec) => spec.endsWith("simulationClient"))).toBe(false);
    expect(code).not.toMatch(/RaidDefenseSimulationClient/);
    expect(code).not.toMatch(/\.dispatch\s*\(/);
    expect(code).not.toMatch(/\.advanceTick\s*\(/);
    expect(code).not.toMatch(/\bsetInterval\s*\(/);
    expect(code).not.toMatch(/<Canvas\b|<mesh\b|@react-three\/fiber/);
  });
});

describe("single session authority", () => {
  test("only the session hook creates the simulation client and drives the live clock", () => {
    const owners = [APP, ...sourceFiles(gameDir)].filter((path) => {
      const code = stripComments(read(path));
      return /RaidDefenseSimulationClient\s*\.\s*create\s*\(/.test(code) || /\bsetInterval\s*\(/.test(code);
    });
    expect(owners).toEqual([SESSION_HOOK]);
  });

  test("live ticks call the direct simulation step, never the legacy tick command", () => {
    const code = stripComments(read(SESSION_HOOK));
    expect(code).toMatch(/\.advanceTick\s*\(/);
    for (const path of [APP, ...sourceFiles(gameDir)]) {
      expect(stripComments(read(path))).not.toMatch(/["']advance_tick["']/);
    }
  });

  test("view-model helpers are pure presentation derivations", () => {
    const code = stripComments(read(VIEW_MODEL));
    const imports = importsOf(code);
    expect(imports.some((spec) => spec === "react" || spec.startsWith("react-dom"))).toBe(false);
    expect(imports.some((spec) => /simulationClient$|replayStore$|generated\//.test(spec))).toBe(false);
    expect(code).not.toMatch(/\.dispatch\s*\(|\.advanceTick\s*\(|localStorage/);
  });

  test("no global state store is introduced for one screen", () => {
    const pkg = JSON.parse(read(join(visualization, "package.json"))) as {
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
    };
    const deps = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies });
    expect(
      deps.filter((dep) => /^(zustand|jotai|valtio|mobx|redux|@reduxjs\/toolkit|recoil|xstate)$/.test(dep)),
    ).toEqual([]);
  });
});

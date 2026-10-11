import { useEffect, useState } from "react";
import type { CellView, DispatchResponse, RaidDefenseCommand } from "../simulationTypes";
import { clampCellCoordinate, placementName, type PlacementChoice } from "./viewModel";

export type CoordinateDraft = { x: string; z: string };

const EMPTY_DRAFT: CoordinateDraft = { x: "", z: "" };

function placementCommand(placement: PlacementChoice, cell: CellView): RaidDefenseCommand {
  switch (placement.kind) {
    case "tower":
      return { type: "place_tower", x: cell.x, z: cell.z, archetype: placement.archetype };
    case "sawmill":
      return { type: "place_sawmill", x: cell.x, z: cell.z };
    case "storage_house":
      return { type: "place_storage_house", x: cell.x, z: cell.z };
    case "house":
      return { type: "place_house", x: cell.x, z: cell.z };
  }
}

/**
 * Presentation-only battlefield interaction state: the inspected cell, its coordinate draft and
 * the armed placement intent. Placement is turned into a player command only when a target cell
 * is activated, through the session's single command path.
 */
export function useBattlefieldSelection({
  grid,
  issue,
  announce,
}: {
  grid: { width: number; height: number } | null;
  issue: (command: RaidDefenseCommand) => DispatchResponse | null;
  announce: (message: string) => void;
}) {
  const [selectedCell, setSelectedCell] = useState<CellView | null>(null);
  const [coordinateDraft, setCoordinateDraft] = useState<CoordinateDraft>(EMPTY_DRAFT);
  const [placement, setPlacement] = useState<PlacementChoice | null>(null);

  useEffect(() => {
    if (!placement) return;
    const cancelPlacement = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setPlacement(null);
      setSelectedCell(null);
      setCoordinateDraft(EMPTY_DRAFT);
      announce("Building placement cancelled.");
    };
    window.addEventListener("keydown", cancelPlacement);
    return () => window.removeEventListener("keydown", cancelPlacement);
  }, [placement, announce]);

  function inspectCell(cell: CellView) {
    setSelectedCell(cell);
    setCoordinateDraft({ x: String(cell.x), z: String(cell.z) });
  }

  function editCoordinate(axis: keyof CoordinateDraft, rawValue: string) {
    const size = axis === "x" ? grid?.width : grid?.height;
    const value =
      rawValue === "" || size === undefined
        ? rawValue
        : String(clampCellCoordinate(Number(rawValue), size));
    const next = { ...coordinateDraft, [axis]: value };
    setCoordinateDraft(next);
    if (next.x === "" || next.z === "") {
      setSelectedCell(null);
      return;
    }
    setSelectedCell({ x: Number(next.x), z: Number(next.z) });
  }

  function armPlacement(nextPlacement: PlacementChoice) {
    setPlacement(nextPlacement);
    setSelectedCell(null);
    setCoordinateDraft(EMPTY_DRAFT);
    announce(
      `${placementName(nextPlacement)} selected. Move over the battlefield and click or tap the tile where it should be placed. Press Escape to cancel.`,
    );
  }

  function placeAt(cell: CellView) {
    if (!placement) {
      inspectCell(cell);
      return;
    }
    const response = issue(placementCommand(placement, cell));
    if (response?.ok) {
      setPlacement(null);
      inspectCell(cell);
    }
  }

  return { selectedCell, coordinateDraft, placement, inspectCell, editCoordinate, armPlacement, placeAt };
}

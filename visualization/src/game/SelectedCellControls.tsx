import type { CellView } from "../simulationTypes";
import type { CoordinateDraft } from "./useBattlefieldSelection";
import { clampCellCoordinate } from "./viewModel";

type Grid = { width: number; height: number };

type SelectedCellControlsProps = {
  grid: Grid;
  selectedCell: CellView | null;
  draft: CoordinateDraft;
  /** Name of the armed building, or null while the player is inspecting. */
  armedBuildingName: string | null;
  description: string;
  onEditCoordinate: (axis: keyof CoordinateDraft, value: string) => void;
  onSelectCell: (cell: CellView) => void;
  onPlaceAt: (cell: CellView) => void;
};

function MobileCellNudge({
  selectedCell,
  grid,
  onSelectCell,
}: {
  selectedCell: CellView;
  grid: Grid;
  onSelectCell: (cell: CellView) => void;
}) {
  function move(dx: number, dz: number) {
    onSelectCell({
      x: clampCellCoordinate(selectedCell.x + dx, grid.width),
      z: clampCellCoordinate(selectedCell.z + dz, grid.height),
    });
  }

  return (
    <div className="mobile-cell-nudge" aria-label="Move selected grid cell">
      <button
        type="button"
        aria-label="Move selected cell west"
        data-testid="cell-west"
        disabled={selectedCell.x <= 0}
        onClick={() => move(-1, 0)}
      >
        ←
      </button>
      <button
        type="button"
        aria-label="Move selected cell north"
        data-testid="cell-north"
        disabled={selectedCell.z <= 0}
        onClick={() => move(0, -1)}
      >
        ↑
      </button>
      <output data-testid="mobile-selected-cell" aria-label="Selected grid cell">
        {selectedCell.x},{selectedCell.z}
      </output>
      <button
        type="button"
        aria-label="Move selected cell south"
        data-testid="cell-south"
        disabled={selectedCell.z >= grid.height - 1}
        onClick={() => move(0, 1)}
      >
        ↓
      </button>
      <button
        type="button"
        aria-label="Move selected cell east"
        data-testid="cell-east"
        disabled={selectedCell.x >= grid.width - 1}
        onClick={() => move(1, 0)}
      >
        →
      </button>
    </div>
  );
}

/**
 * Precise cell selection: coordinate fields, the touch nudge fallback, the optional
 * "place at selected cell" action while a building is armed, and the selection description.
 */
export function SelectedCellControls({
  grid,
  selectedCell,
  draft,
  armedBuildingName,
  description,
  onEditCoordinate,
  onSelectCell,
  onPlaceAt,
}: SelectedCellControlsProps) {
  return (
    <div className="cell-controls">
      <span className="eyebrow">{armedBuildingName ? "Placement mode" : "Selected grid cell"}</span>
      <div className="coordinate-inputs">
        <label>
          X
          <input
            data-testid="cell-x"
            type="number"
            min={0}
            max={grid.width - 1}
            placeholder="–"
            value={draft.x}
            onChange={(event) => onEditCoordinate("x", event.target.value)}
          />
        </label>
        <label>
          Z
          <input
            data-testid="cell-z"
            type="number"
            min={0}
            max={grid.height - 1}
            placeholder="–"
            value={draft.z}
            onChange={(event) => onEditCoordinate("z", event.target.value)}
          />
        </label>
      </div>
      {selectedCell ? (
        <MobileCellNudge selectedCell={selectedCell} grid={grid} onSelectCell={onSelectCell} />
      ) : (
        <output className="selected-tower" data-testid="mobile-selected-cell">
          No tile selected
        </output>
      )}
      {armedBuildingName && selectedCell && (
        <button
          className="build-button precise-place-button"
          type="button"
          data-testid="place-selected-cell"
          onClick={() => onPlaceAt(selectedCell)}
        >
          Place at {selectedCell.x},{selectedCell.z}
        </button>
      )}
      <span className="selected-tower" data-testid="selected-building">
        {description}
      </span>
    </div>
  );
}

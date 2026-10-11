import type { ReactNode } from "react";
import type { SettlementStatusView } from "./viewModel";

type BattlefieldPanelProps = {
  meters: Pick<
    SettlementStatusView,
    | "townHealth"
    | "townMaxHealth"
    | "townHealthPercent"
    | "wood"
    | "woodCapacity"
    | "woodPercent"
    | "houseLockLabel"
  >;
  /** Name of the armed building, or null while the player is inspecting. */
  armedBuildingName: string | null;
  /** The 3D world framed by this panel. */
  children: ReactNode;
};

/** Frames the battlefield with Town Hall health, settlement storage and the touch hint. */
export function BattlefieldPanel({ meters, armedBuildingName, children }: BattlefieldPanelProps) {
  return (
    <section className="battlefield" data-testid="world-3d" aria-label="3D defense grid">
      {children}
      <div className="town-health" aria-label="Settlement status">
        <span>
          Town Hall {meters.townHealth}/{meters.townMaxHealth}
        </span>
        <span className="health-track" aria-hidden="true">
          <span style={{ width: `${meters.townHealthPercent}%` }} />
        </span>
        <span>
          Settlement wood {meters.wood}/{meters.woodCapacity}
        </span>
        <span className="wood-track" aria-hidden="true">
          <span style={{ width: `${meters.woodPercent}%` }} />
        </span>
        <span data-testid="house-lock-state">{meters.houseLockLabel}</span>
      </div>
      <p className="mobile-world-hint" data-testid="mobile-world-hint">
        {armedBuildingName
          ? `Tap a tile to place ${armedBuildingName} · drag to orbit · pinch to zoom`
          : "Tap a tile to inspect · drag to orbit · pinch to zoom"}
      </p>
    </section>
  );
}

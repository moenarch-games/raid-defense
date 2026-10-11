import type { BuildOption, BuildOptionsView, PlacementChoice, TowerUpgradeView } from "./viewModel";

type BuildActionsProps = {
  options: BuildOptionsView;
  armed: PlacementChoice | null;
  upgrade: TowerUpgradeView;
  onArm: (placement: PlacementChoice) => void;
  onUpgrade: () => void;
};

function BuildButton({
  option,
  pressed,
  className,
  testId,
  onClick,
}: {
  option: BuildOption;
  pressed: boolean;
  className: string;
  testId: string;
  onClick: () => void;
}) {
  return (
    <button
      className={className}
      type="button"
      aria-pressed={pressed}
      disabled={!option.available}
      onClick={onClick}
      data-testid={testId}
    >
      {option.label}
    </button>
  );
}

/**
 * Build buttons arm presentation-only placement intent; the upgrade button issues an upgrade
 * for the inspected tower. Neither owns the command path.
 */
export function BuildActions({ options, armed, upgrade, onArm, onUpgrade }: BuildActionsProps) {
  return (
    <div className="tower-actions">
      <BuildButton
        className="build-button economy-button"
        option={options.sawmill}
        pressed={armed?.kind === "sawmill"}
        onClick={() => onArm({ kind: "sawmill" })}
        testId="build-sawmill"
      />
      <BuildButton
        className="build-button economy-button"
        option={options.storageHouse}
        pressed={armed?.kind === "storage_house"}
        onClick={() => onArm({ kind: "storage_house" })}
        testId="build-storage-house"
      />
      <BuildButton
        className="build-button economy-button"
        option={options.house}
        pressed={armed?.kind === "house"}
        onClick={() => onArm({ kind: "house" })}
        testId="build-house"
      />
      <BuildButton
        className="build-button"
        option={options.arrowTower}
        pressed={armed?.kind === "tower" && armed.archetype === "arrow"}
        onClick={() => onArm({ kind: "tower", archetype: "arrow" })}
        testId="build-arrow-tower"
      />
      <BuildButton
        className="build-button"
        option={options.cannonTower}
        pressed={armed?.kind === "tower" && armed.archetype === "cannon"}
        onClick={() => onArm({ kind: "tower", archetype: "cannon" })}
        testId="build-cannon-tower"
      />
      <button
        className="build-button upgrade-button"
        type="button"
        disabled={!upgrade.available}
        onClick={onUpgrade}
        data-testid="upgrade-tower"
      >
        {upgrade.label}
      </button>
    </div>
  );
}

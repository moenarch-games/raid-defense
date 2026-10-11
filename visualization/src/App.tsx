import { BattlefieldPanel } from "./game/BattlefieldPanel";
import { BuildActions } from "./game/BuildActions";
import { GameFeedback } from "./game/GameFeedback";
import { GameWorld } from "./game/GameWorld";
import { SelectedCellControls } from "./game/SelectedCellControls";
import { SessionEvidence } from "./game/SessionEvidence";
import { SettlementStatus } from "./game/SettlementStatus";
import { useBattlefieldSelection } from "./game/useBattlefieldSelection";
import { useRaidDefenseSession } from "./game/useRaidDefenseSession";
import {
  deriveBuildOptions,
  deriveSettlementStatus,
  deriveTowerUpgrade,
  describeSelection,
  placementName,
} from "./game/viewModel";
import { WaveControl } from "./game/WaveControl";

/** The Raid Defense game screen: wires one simulation session to the game building blocks. */
function App() {
  const { snapshot, feedback, issue, announce } = useRaidDefenseSession();
  const selection = useBattlefieldSelection({
    grid: snapshot ? { width: snapshot.grid_width, height: snapshot.grid_height } : null,
    issue,
    announce,
  });

  if (!snapshot) {
    return (
      <main className="loading-shell" data-testid="loading-state">
        <h1>Raid Defense</h1>
        <p>{feedback}</p>
      </main>
    );
  }

  const { selectedCell, placement } = selection;
  const status = deriveSettlementStatus(snapshot);
  const armedBuildingName = placement ? placementName(placement) : null;
  const grid = { width: snapshot.grid_width, height: snapshot.grid_height };

  return (
    <main className="game-shell" data-testid="raid-defense-game">
      <header className="top-bar">
        <div className="identity">
          <span className="eyebrow">Economy-defense prototype</span>
          <h1>Raid Defense</h1>
        </div>
        <SettlementStatus status={status} />
        <WaveControl
          label={status.waveActionLabel}
          canStart={status.canStartWave}
          onStartWave={() => issue({ type: "start_wave" })}
        />
      </header>

      <BattlefieldPanel meters={status} armedBuildingName={armedBuildingName}>
        <GameWorld
          snapshot={snapshot}
          selectedCell={selectedCell}
          placement={placement}
          onSelectCell={selection.inspectCell}
          onPlace={selection.placeAt}
        />
      </BattlefieldPanel>

      <section className="build-bar" aria-label="Build controls" data-testid="mobile-command-dock">
        <SelectedCellControls
          grid={grid}
          selectedCell={selectedCell}
          draft={selection.coordinateDraft}
          armedBuildingName={armedBuildingName}
          description={describeSelection(snapshot, selectedCell, placement)}
          onEditCoordinate={selection.editCoordinate}
          onSelectCell={selection.inspectCell}
          onPlaceAt={selection.placeAt}
        />
        <BuildActions
          options={deriveBuildOptions(snapshot)}
          armed={placement}
          upgrade={deriveTowerUpgrade(snapshot, selectedCell)}
          onArm={selection.armPlacement}
          onUpgrade={() =>
            selectedCell && issue({ type: "upgrade_tower", x: selectedCell.x, z: selectedCell.z })
          }
        />
        <GameFeedback message={feedback} />
      </section>

      <SessionEvidence
        seed={snapshot.seed}
        gridWidth={snapshot.grid_width}
        gridHeight={snapshot.grid_height}
        contractVersion={snapshot.contract_version}
        projectiles={status.projectiles}
        checksum={snapshot.checksum}
      />
    </main>
  );
}

export default App;

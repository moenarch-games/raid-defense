import type { SettlementStatusView } from "./viewModel";

type SettlementStatusProps = {
  status: Pick<
    SettlementStatusView,
    | "wood"
    | "people"
    | "populationCapacity"
    | "hauling"
    | "cycleLabel"
    | "wave"
    | "completedWaves"
    | "raiders"
    | "forests"
    | "storageHouses"
    | "sawmills"
    | "constructionSites"
    | "towers"
  >;
};

/** Header strip of authoritative settlement counts and the day/rally/night cycle. */
export function SettlementStatus({ status }: SettlementStatusProps) {
  return (
    <dl className="status-strip" aria-label="Settlement status">
      <div>
        <dt>Wood</dt>
        <dd data-testid="wood-value">{status.wood}</dd>
      </div>
      <div>
        <dt>People</dt>
        <dd data-testid="people-value">
          {status.people}/{status.populationCapacity}
        </dd>
      </div>
      <div>
        <dt>Hauling</dt>
        <dd data-testid="hauling-value">{status.hauling}</dd>
      </div>
      <div>
        <dt>Cycle</dt>
        <dd data-testid="cycle-timer">{status.cycleLabel}</dd>
      </div>
      <div>
        <dt>Wave</dt>
        <dd data-testid="wave-value">{status.wave}</dd>
      </div>
      <div>
        <dt>Cleared</dt>
        <dd data-testid="completed-waves-value">{status.completedWaves}</dd>
      </div>
      <div>
        <dt>Raiders</dt>
        <dd data-testid="raider-count">{status.raiders}</dd>
      </div>
      <div>
        <dt>Forests</dt>
        <dd data-testid="forest-count">{status.forests}</dd>
      </div>
      <div>
        <dt>Stores</dt>
        <dd data-testid="storage-count">{status.storageHouses}</dd>
      </div>
      <div>
        <dt>Sawmills</dt>
        <dd data-testid="sawmill-count">{status.sawmills}</dd>
      </div>
      <div>
        <dt>Building</dt>
        <dd data-testid="construction-count">{status.constructionSites}</dd>
      </div>
      <div>
        <dt>Towers</dt>
        <dd data-testid="tower-count">{status.towers}</dd>
      </div>
    </dl>
  );
}

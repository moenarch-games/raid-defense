type SessionEvidenceProps = {
  seed: string;
  gridWidth: number;
  gridHeight: number;
  contractVersion: number;
  projectiles: number;
  checksum: string;
};

/** Footer evidence binding the visible game to its deterministic seed, contract and checksum. */
export function SessionEvidence({
  seed,
  gridWidth,
  gridHeight,
  contractVersion,
  projectiles,
  checksum,
}: SessionEvidenceProps) {
  return (
    <footer className="evidence-line">
      <span>Seed {seed}</span>
      <span>
        Map {gridWidth}×{gridHeight}
      </span>
      <span>Contract v{contractVersion}</span>
      <span data-testid="projectile-count">Projectiles {projectiles}</span>
      <span data-testid="checksum">Checksum {checksum}</span>
    </footer>
  );
}

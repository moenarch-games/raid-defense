type WaveControlProps = {
  label: string;
  canStart: boolean;
  onStartWave: () => void;
};

/** Manual raid trigger; the Rust core decides when the rally and the raid actually happen. */
export function WaveControl({ label, canStart, onStartWave }: WaveControlProps) {
  return (
    <button
      className="wave-button"
      type="button"
      disabled={!canStart}
      onClick={onStartWave}
      data-testid="start-wave"
    >
      {label}
    </button>
  );
}

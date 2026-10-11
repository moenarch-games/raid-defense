import { useCallback, useEffect, useRef, useState } from "react";
import { RaidDefenseSimulationClient } from "../simulationClient";
import type { DispatchResponse, RaidDefenseCommand, SnapshotView } from "../simulationTypes";
import { describeError, describeEvent, isConsequentialTick, TICK_INTERVAL_MS } from "./viewModel";

const DEFAULT_SEED = 0x5eed;
const INITIALIZING_FEEDBACK = "Initializing deterministic defense grid…";
const READY_FEEDBACK =
  "People gather trees, carry raw wood to sawmills, haul mill stock into settlement storage, and physically deliver tower materials.";

/** The part of the simulation client the session needs: one query, one clock step, one command path. */
export type SimulationSessionClient = {
  snapshot(): SnapshotView;
  advanceTick(): DispatchResponse;
  dispatch(command: RaidDefenseCommand): DispatchResponse;
};

export type RaidDefenseSessionOptions = {
  createClient?: () => Promise<SimulationSessionClient>;
  tickIntervalMs?: number;
};

export type RaidDefenseSession = {
  /** Latest authoritative snapshot, or null until the simulation has started. */
  snapshot: SnapshotView | null;
  /** Latest player-facing feedback message. */
  feedback: string;
  /** The single player-command path. Returns null while no simulation is running. */
  issue: (command: RaidDefenseCommand) => DispatchResponse | null;
  /** Reports a presentation-only event (such as arming or cancelling placement) as feedback. */
  announce: (message: string) => void;
};

function createDefaultClient(): Promise<SimulationSessionClient> {
  return RaidDefenseSimulationClient.create(DEFAULT_SEED);
}

/**
 * Owns one Raid Defense simulation session: the client lifetime, the live clock (direct
 * simulation steps, never tick commands), the player-command path, and feedback events.
 */
export function useRaidDefenseSession(options: RaidDefenseSessionOptions = {}): RaidDefenseSession {
  const createClientRef = useRef(options.createClient ?? createDefaultClient);
  const tickIntervalMs = options.tickIntervalMs ?? TICK_INTERVAL_MS;
  const [client, setClient] = useState<SimulationSessionClient | null>(null);
  const [snapshot, setSnapshot] = useState<SnapshotView | null>(null);
  const [feedback, setFeedback] = useState(INITIALIZING_FEEDBACK);

  useEffect(() => {
    let cancelled = false;
    void createClientRef
      .current()
      .then((simulation) => {
        if (cancelled) return;
        const initial = simulation.snapshot();
        setClient(simulation);
        setSnapshot(initial);
        setFeedback(READY_FEEDBACK);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setFeedback(error instanceof Error ? error.message : "Failed to initialize the simulation.");
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const townHealth = snapshot?.town_health;

  useEffect(() => {
    if (!client || townHealth === undefined || townHealth === 0) return;
    const timer = window.setInterval(() => {
      const response = client.advanceTick();
      setSnapshot(response.snapshot);
      if (!response.ok) {
        setFeedback(describeError(response.error?.code ?? "unknown_error"));
        return;
      }
      if (isConsequentialTick(response.event)) {
        setFeedback(describeEvent(response.event));
      }
    }, tickIntervalMs);
    return () => window.clearInterval(timer);
  }, [client, townHealth, tickIntervalMs]);

  const issue = useCallback(
    (command: RaidDefenseCommand) => {
      if (!client) return null;
      const response = client.dispatch(command);
      setSnapshot(response.snapshot);
      setFeedback(
        response.ok
          ? describeEvent(response.event)
          : describeError(response.error?.code ?? "unknown_error"),
      );
      return response;
    },
    [client],
  );

  return { snapshot, feedback, issue, announce: setFeedback };
}

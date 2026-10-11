/** Live region announcing the latest command, simulation or placement feedback. */
export function GameFeedback({ message }: { message: string }) {
  return (
    <p className="feedback-line" role="status" aria-live="polite" data-testid="event-feedback">
      {message}
    </p>
  );
}

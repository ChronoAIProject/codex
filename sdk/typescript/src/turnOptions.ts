export type TurnOptions = {
  /** JSON schema describing the expected agent output. */
  outputSchema?: unknown;
  /** AbortSignal to cancel the turn. */
  signal?: AbortSignal;
  /** Additional directories to make available to Codex for this turn. */
  additionalDirectories?: string[];
};

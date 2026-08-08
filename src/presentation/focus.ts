/**
 * Keyboard-only focus treatment for text-entry controls on the dark app surface.
 *
 * Keep this as a literal so Tailwind includes the utilities in production builds.
 */
export const focusIndicatorClass =
  "outline-none focus-visible:border-sky-300 focus-visible:ring-2 focus-visible:ring-sky-400 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 forced-colors:focus-visible:outline forced-colors:focus-visible:outline-2 forced-colors:focus-visible:outline-[Highlight]";

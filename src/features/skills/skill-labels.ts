/**
 * Provider display labels.
 *
 * Separate from `skill-badges.tsx` because React Fast Refresh only works in a
 * file that exports components and nothing else — mixing the two silently
 * disables hot reload for the badges.
 */
import type { ProviderId } from "@/types/skill";

const PROVIDER_LABEL: Record<ProviderId, string> = {
  claude: "[CC]",
  cursor: "Cursor",
  codex: "Codex",
  opencode: "OpenCode",
};

export function providerLabel(id: ProviderId): string {
  return PROVIDER_LABEL[id];
}

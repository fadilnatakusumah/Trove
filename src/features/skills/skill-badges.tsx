/**
 * Shared skill UI atoms (plan 05): provider/scope/status badges and
 * scope-type selector. Kept tiny and dumb — no business logic (Rule 5).
 */
import { Globe, FolderOpen } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import type { ProviderId, SkillScope } from "@/types/skill";

const PROVIDER_LABEL: Record<ProviderId, string> = {
  claude: "Claude Code",
  cursor: "Cursor",
  codex: "Codex",
  opencode: "OpenCode",
};

export function providerLabel(id: ProviderId): string {
  return PROVIDER_LABEL[id];
}

export function ProviderBadge({ provider }: { provider: ProviderId }) {
  return <Badge variant="outline">{providerLabel(provider)}</Badge>;
}

export function ScopeBadge({ scope }: { scope: SkillScope }) {
  return scope.type === "global" ? (
    <Badge variant="secondary">
      <Globe className="size-3" /> Global
    </Badge>
  ) : (
    <Badge variant="secondary">
      <FolderOpen className="size-3" /> Project
    </Badge>
  );
}

export function EnabledDot({ enabled }: { enabled: boolean }) {
  return (
    <span
      aria-label={enabled ? "Enabled" : "Disabled"}
      className={`inline-block size-2 shrink-0 rounded-full ${
        enabled ? "bg-emerald-500" : "bg-muted-foreground/40"
      }`}
    />
  );
}

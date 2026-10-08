import { useMemo } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useProviders } from "@/features/providers/queries";
import { useAllSkills, type Combo } from "@/features/skills/useAllSkills";
import { listProjects } from "@/features/projects/projects";
import { providerLabel } from "@/features/skills/skill-badges";
import type { ProviderId, SkillScope } from "@/types/skill";

/**
 * Dashboard (§12): counts + recently changed skills. Deliberately dumb —
 * no analytics, no charts. "Recently changed" = filesystem mtime reported
 * by the skill listing order (ids only in Phase 1; ordering comes free from
 * skills_for_scope sorting). Complexity would be invention.
 */
export function DashboardPage() {
  const providers = useProviders();
  const projects = useQuery({ queryKey: ["projects"], queryFn: listProjects });

  const combos = useMemo<Combo[]>(() => {
    const ps = (providers.data ?? []).map((p) => p.providerId);
    const scopes = [
      { type: "global" } as const,
      ...(projects.data ?? []).map(
        (p) => ({ type: "project", projectPath: p.path }) as SkillScope,
      ),
    ];
    return ps.flatMap((providerId) =>
      scopes.map((scope) => ({ providerId, scope })),
    );
  }, [providers.data, projects.data]);

  const { data: skills = [], isPending: skillsPending } = useAllSkills(combos);

  const detected = (providers.data ?? []).filter((p) => p.detected);
  const byProvider = useMemo(() => {
    const counts = new Map<ProviderId, number>();
    for (const s of skills) {
      counts.set(s.provider, (counts.get(s.provider) ?? 0) + 1);
    }
    return counts;
  }, [skills]);

  const loading = providers.isPending || projects.isPending || skillsPending;

  return (
    <main className="flex flex-1 flex-col gap-4 p-6">
      <header>
        <h1 className="text-2xl font-semibold tracking-tight">Dashboard</h1>
        <p className="text-sm text-muted-foreground">
          Overview of detected agents, projects, and installed skills.
        </p>
      </header>

      <div className="grid gap-3 sm:grid-cols-3">
        <StatCard label="Agents" value={loading ? "…" : `${detected.length}`} hint={`of ${(providers.data ?? []).length} supported`} to="/providers" />
        <StatCard label="Projects" value={loading ? "…" : `${(projects.data ?? []).length}`} hint="registered" to="/projects" />
        <StatCard label="Skills" value={loading ? "…" : `${skills.length}`} hint="installed" to="/skills" />
      </div>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">Detected agents</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground">
          {loading ? (
            "Detecting…"
          ) : detected.length === 0 ? (
            "No agents detected yet. Install Claude Code, Cursor, or Codex to get started."
          ) : (
            <ul className="space-y-1">
              {detected.map((p) => (
                <li key={p.providerId} className="flex items-center justify-between">
                  <span className="text-foreground">{p.name}</span>
                  <span>
                    {byProvider.get(p.providerId) ?? 0} skill
                    {(byProvider.get(p.providerId) ?? 0) === 1 ? "" : "s"} ·{" "}
                    <span className="font-mono text-xs">{p.globalSkillsDir}</span>
                  </span>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">Skills by provider</CardTitle>
        </CardHeader>
        <CardContent className="text-sm text-muted-foreground">
          {loading ? (
            "Loading…"
          ) : skills.length === 0 ? (
            "No skills installed yet. Head to Skills to install one from a Git repository."
          ) : (
            <ul className="space-y-1">
              {[...byProvider.entries()].map(([id, count]) => (
                <li key={id} className="flex items-center justify-between">
                  <span className="text-foreground">{providerLabel(id)}</span>
                  <span>
                    {count} skill{count === 1 ? "" : "s"}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </main>
  );
}

function StatCard({
  label,
  value,
  hint,
  to,
}: {
  label: string;
  value: string;
  hint: string;
  to: string;
}) {
  return (
    <Link to={to} className="block outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-lg">
      <Card className="transition-colors hover:bg-accent/50">
        <CardHeader className="pb-0">
          <CardTitle className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {label}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-3xl font-semibold tracking-tight">{value}</p>
          <p className="text-xs text-muted-foreground">{hint}</p>
        </CardContent>
      </Card>
    </Link>
  );
}

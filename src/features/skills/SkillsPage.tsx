import { useMemo, useState } from "react";
import { Link } from "react-router-dom";

import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useProviders } from "@/features/providers/queries";
import { useAllSkills, type Combo } from "@/features/skills/useAllSkills";
import { listProjects } from "@/features/projects/projects";
import { useQuery } from "@tanstack/react-query";
import {
  EnabledDot,
  ProviderBadge,
  ScopeBadge,
  providerLabel,
} from "@/features/skills/skill-badges";
import { Card, CardContent } from "@/components/ui/card";
import { EmptyState, ErrorState, LoadingState } from "@/features/states";
import type { ProviderId, Skill } from "@/types/skill";

type ProviderFilter = ProviderId | "all";
type ScopeFilter = "all" | "global" | "project";
type StatusFilter = "all" | "enabled" | "disabled";

/**
 * Skills screen (§13) — the primary screen.
 *
 * The whole provider × scope matrix is fetched via one hook (rules-of-hooks
 * safe); a skill present in several scopes appears once per scope with its
 * badge — filesystem reality, no virtual state (§16).
 */
export function SkillsPage() {
  const providers = useProviders();
  const projects = useQuery({ queryKey: ["projects"], queryFn: listProjects });

  const [search, setSearch] = useState("");
  const [providerFilter, setProviderFilter] = useState<ProviderFilter>("all");
  const [scopeFilter, setScopeFilter] = useState<ScopeFilter>("all");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");

  const combos = useMemo<Combo[]>(() => {
    const ps =
      providerFilter === "all"
        ? (providers.data ?? []).map((p) => p.providerId)
        : [providerFilter];
    const out: Combo[] = [];
    for (const providerId of ps) {
      if (scopeFilter !== "project") {
        out.push({ providerId, scope: { type: "global" } });
      }
      if (scopeFilter !== "global") {
        for (const project of projects.data ?? []) {
          out.push({ providerId, scope: { type: "project", projectPath: project.path } });
        }
      }
    }
    return out;
  }, [providers.data, projects.data, providerFilter, scopeFilter]);

  const { data: skills = [], isPending, isError, error } = useAllSkills(combos);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    return skills
      .filter((s) =>
        q === "" ||
        s.name.toLowerCase().includes(q) ||
        (s.description ?? "").toLowerCase().includes(q),
      )
      .filter((s) => statusFilter === "all" || (statusFilter === "enabled") === s.enabled)
      .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  }, [skills, search, statusFilter]);

  const loading = isPending || providers.isPending || projects.isPending;

  return (
    <main className="flex flex-1 flex-col gap-4 p-6">
      <header>
        <h1 className="text-2xl font-semibold tracking-tight">Skills</h1>
        <p className="text-sm text-muted-foreground">
          {loading ? "Loading…" : `${filtered.length} skill${filtered.length === 1 ? "" : "s"}`}
        </p>
      </header>

      <div className="flex flex-wrap items-center gap-2">
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search skills…"
          className="w-64"
          aria-label="Search skills"
        />
        <Select value={providerFilter} onValueChange={(v) => setProviderFilter(v as ProviderFilter)}>
          <SelectTrigger className="w-40" aria-label="Filter by provider">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All providers</SelectItem>
            {(providers.data ?? []).map((p) => (
              <SelectItem key={p.providerId} value={p.providerId}>
                {p.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={scopeFilter} onValueChange={(v) => setScopeFilter(v as ScopeFilter)}>
          <SelectTrigger className="w-36" aria-label="Filter by scope">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All scopes</SelectItem>
            <SelectItem value="global">Global</SelectItem>
            <SelectItem value="project">Project</SelectItem>
          </SelectContent>
        </Select>
        <Select value={statusFilter} onValueChange={(v) => setStatusFilter(v as StatusFilter)}>
          <SelectTrigger className="w-36" aria-label="Filter by status">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All statuses</SelectItem>
            <SelectItem value="enabled">Enabled</SelectItem>
            <SelectItem value="disabled">Disabled</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {loading ? (
        <LoadingState label="Loading skills…" />
      ) : isError ? (
        <ErrorState message={error?.message ?? "Failed to load skills"} />
      ) : filtered.length === 0 ? (
        <EmptyState
          title={skills.length === 0 ? "No skills found" : "No skills match your filters"}
          hint={
            skills.length === 0
              ? "Install a skill or check that an agent is detected in Providers."
              : "Try clearing the search or filters."
          }
        />
      ) : (
        <div className="grid gap-2">
          {filtered.map((skill) => (
            <SkillRow key={`${skill.provider}-${skill.scope.type}-${skill.path}`} skill={skill} />
          ))}
        </div>
      )}
    </main>
  );
}

function SkillRow({ skill }: { skill: Skill }) {
  // Detail route key: path identifies the skill uniquely across providers/scopes.
  const detailId = encodeURIComponent(skill.path);
  const scopeName =
    skill.scope.type === "project"
      ? (skill.scope.projectPath.replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? "Project")
      : undefined;

  return (
    <Link to={`/skills/${detailId}`} className="block outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-lg">
      <Card className="transition-colors hover:bg-accent/50">
        <CardContent className="flex items-center gap-3 py-3">
          <EnabledDot enabled={skill.enabled} />
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium">{skill.name}</p>
            {skill.description ? (
              <p className="truncate text-xs text-muted-foreground">{skill.description}</p>
            ) : null}
          </div>
          <div className="hidden shrink-0 items-center gap-1.5 sm:flex">
            <ProviderBadge provider={skill.provider} />
            <ScopeBadge scope={skill.scope} />
            {scopeName ? (
              <span className="max-w-40 truncate font-mono text-xs text-muted-foreground">
                {scopeName}
              </span>
            ) : null}
            <span className="sr-only">{providerLabel(skill.provider)}</span>
          </div>
        </CardContent>
      </Card>
    </Link>
  );
}

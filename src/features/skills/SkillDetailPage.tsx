import { useMemo } from "react";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft, FolderOpen } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { useProviders } from "@/features/providers/queries";
import { useAllSkills, type Combo } from "@/features/skills/useAllSkills";
import { listProjects } from "@/features/projects/projects";
import {
  EnabledDot,
  ProviderBadge,
  ScopeBadge,
} from "@/features/skills/skill-badges";
import { EmptyState, LoadingState } from "@/features/states";
import { invokeCommand } from "@/lib/tauri";

interface DirEntry {
  name: string;
  isDir: boolean;
}

export function SkillDetailPage() {
  // The router param is the skill's display path (stable across searches).
  const { skillId: encodedPath } = useParams<{ skillId: string }>();
  const skillPath = encodedPath ? decodeURIComponent(encodedPath) : "";

  const providers = useProviders();
  const projects = useQuery({ queryKey: ["projects"], queryFn: listProjects });

  // Find the skill across the same provider × scope matrix as the list screen.
  const combos = useMemo<Combo[]>(() => {
    const ps = (providers.data ?? []).map((p) => p.providerId);
    const scopes = [
      { type: "global" } as const,
      ...(projects.data ?? []).map((p) => ({ type: "project", projectPath: p.path }) as const),
    ];
    return ps.flatMap((providerId) => scopes.map((scope) => ({ providerId, scope })));
  }, [providers.data, projects.data]);

  const { data: allSkills = [], isPending: skillsPending } = useAllSkills(combos);
  const skill = useMemo(() => allSkills.find((s) => s.path === skillPath), [allSkills, skillPath]);

  const files = useQuery({
    queryKey: ["skill-files", skillPath],
    queryFn: () => invokeCommand<DirEntry[]>("list_skill_files", { path: skillPath }),
    enabled: Boolean(skill),
  });

  if (!skillPath || (providers.isPending || projects.isPending || skillsPending)) {
    return <LoadingState label="Loading skill…" />;
  }

  if (!skill) {
    return (
      <main className="flex flex-1 flex-col gap-4 p-6">
        <BackLink />
        <EmptyState title="Skill not found" hint="It may have been uninstalled or moved." />
      </main>
    );
  }

  return (
    <main className="flex flex-1 flex-col gap-4 p-6">
      <BackLink />

      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <EnabledDot enabled={skill.enabled} />
            <h1 className="text-2xl font-semibold tracking-tight">{skill.name}</h1>
          </div>
          {skill.description ? (
            <p className="mt-1 text-sm text-muted-foreground">{skill.description}</p>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {/* Lifecycle actions (enable/disable/uninstall) land in plans 06–07. */}
          <Button
            variant="outline"
            onClick={() => {
              invokeCommand("open_folder", { path: skill.path })
                .then(() => toast.success("Opened in file manager"))
                .catch((e: Error) => toast.error(e.message));
            }}
          >
            <FolderOpen /> Open Folder
          </Button>
        </div>
      </header>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">Details</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-2 text-sm">
          <DetailRow label="Provider">
            <ProviderBadge provider={skill.provider} />
          </DetailRow>
          <DetailRow label="Scope">
            <div className="flex items-center gap-2">
              <ScopeBadge scope={skill.scope} />
              {skill.scope.type === "project" ? (
                <span className="font-mono text-xs text-muted-foreground">
                  {skill.scope.projectPath}
                </span>
              ) : null}
            </div>
          </DetailRow>
          <DetailRow label="Location">
            <span className="font-mono text-xs">{skill.path}</span>
          </DetailRow>
          {skill.version ? (
            <DetailRow label="Version">
              <Badge variant="outline">{skill.version}</Badge>
            </DetailRow>
          ) : null}
          {Object.entries(skill.metadata ?? {}).map(([k, v]) => (
            <DetailRow key={k} label={k}>
              <span className="font-mono text-xs text-muted-foreground">{v}</span>
            </DetailRow>
          ))}
          <Separator className="my-1" />
          <DetailRow label="Files">
            {files.isPending ? (
              <span className="text-xs text-muted-foreground">Reading…</span>
            ) : files.isError ? (
              <span className="text-xs text-muted-foreground">Could not list files</span>
            ) : (
              <span className="font-mono text-xs">
                {(files.data ?? []).map((f) => f.name).join("  ·  ")}
              </span>
            )}
          </DetailRow>
        </CardContent>
      </Card>
    </main>
  );
}

function BackLink() {
  return (
    <Link
      to="/skills"
      className="inline-flex w-fit items-center gap-1 text-sm text-muted-foreground hover:text-foreground"
    >
      <ArrowLeft className="size-4" /> Back to Skills
    </Link>
  );
}

function DetailRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-4">
      <span className="w-24 shrink-0 text-xs uppercase tracking-wide text-muted-foreground">
        {label}
      </span>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

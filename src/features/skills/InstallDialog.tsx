/**
 * Install Skill dialog (plan 06, §17/§19).
 *
 * Flow: pick source (Git URL / local folder) → scope → [project] → Inspect →
 * review discovered skills (checkbox selection for collections) → Install.
 * Busy state is derived from the mutations; failures arrive as structured
 * AppErrors and surface as toasts with actionable copy.
 */
import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, FolderOpen, Globe, FolderGit2, Loader2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/features/skills/ConfirmDialog";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useProviders } from "@/features/providers/queries";
import { listProjects, pickFolder } from "@/features/projects/projects";
import {
  inspectSource,
  installSkills,
  errorDetail,
  skillsOf,
  type DiscoveredSkill,
  type SourceInspection,
} from "@/features/skills/install";
import { isAppError } from "@/types/error";
import type { ProviderId, SkillScope, SkillSource } from "@/types/skill";

export function InstallDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const providers = useProviders();
  const projects = useProjects();
  const queryClient = useQueryClient();

  const [mode, setMode] = useState<"git" | "local">("git");
  const [url, setUrl] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [scopeType, setScopeType] = useState<"global" | "project">("global");
  const [projectPath, setProjectPath] = useState<string>("");
  const [inspection, setInspection] = useState<SourceInspection | null>(null);
  // Handle for the staging this inspection came from. Sent back with the
  // install so the backend copies from the clone the user just reviewed.
  const [stagingId, setStagingId] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // A refused install, held so the user can choose to replace and retry.
  const [conflict, setConflict] = useState<string | null>(null);

  // Offer EVERY supported provider, not just detected ones: installing into
  // an agent for the first time is exactly when its skills dir doesn't exist
  // yet (the installer creates it). Detected ones are marked, not filtered.
  const allProviders = providers.data ?? [];
  const [providerId, setProviderId] = useState<ProviderId | "">("");

  // Default to a detected agent when one exists, else the first supported.
  const effectiveProvider: ProviderId | null =
    providerId ||
    allProviders.find((p) => p.detected)?.providerId ||
    allProviders[0]?.providerId ||
    null;

  const source: SkillSource | null = useMemo(() => {
    if (mode === "git") {
      return url.trim() ? { type: "git", url: url.trim() } : null;
    }
    return localPath ? { type: "local", path: localPath } : null;
  }, [mode, url, localPath]);

  const scope: SkillScope | null =
    scopeType === "global"
      ? { type: "global" }
      : projectPath
        ? { type: "project", projectPath }
        : null;

  const reset = () => {
    setInspection(null);
    setStagingId(null);
    setSelected(new Set());
    setConflict(null);
  };

  // Every close path (success, Cancel, Escape, backdrop, X) routes through
  // Radix's onOpenChange, so resetting here covers all of them.
  const close = () => {
    onOpenChange(false);
    reset();
  };

  const handleOpenChange = (next: boolean) => {
    if (next) onOpenChange(true);
    else close();
  };

  const inspect = useMutation({
    mutationFn: () => {
      if (!source) throw new Error("Enter a source first");
      return inspectSource(source);
    },
    onSuccess: (result) => {
      setStagingId(result.stagingId);
      setInspection(result.inspection);
      // Default: select all discovered skills.
      setSelected(new Set(skillsOf(result.inspection).map((s) => s.dir)));
    },
    onError: (e) => {
      // Structured AppError: message plus the underlying reason (e.g. git's
      // own "repository not found") so failures are actionable.
      toast.error("Could not read that source", { description: errorDetail(e) });
    },
  });

  const install = useMutation({
    // `replace` arrives from the conflict confirmation below; it is never
    // inferred, so an overwrite can only happen because the user asked.
    mutationFn: async (replace: boolean) => {
      if (!source || !scope || !effectiveProvider || !inspection || !stagingId) {
        throw new Error("Complete the install options first");
      }
      const chosen = skillsOf(inspection)
        .map((s) => s.dir)
        .filter((d) => selected.has(d));
      if (chosen.length === 0) throw new Error("Select at least one skill");
      return installSkills({
        source,
        providerId: effectiveProvider,
        scope,
        skillDirs: chosen,
        stagingId,
        replace,
      });
    },
    onSuccess: (installed, replace) => {
      // Refresh the skills matrix so the new skill appears (§26), then close.
      void queryClient.invalidateQueries({ queryKey: ["skills-all"] });
      void queryClient.invalidateQueries({ queryKey: ["providers"] });

      const where = scope?.type === "project" ? "to this project" : "globally";
      // Name the skill from the inspection — that is what the user picked and
      // what its directory is called. The installed `Skill` carries a
      // frontmatter name, which can disagree with the directory.
      const chosenNames = inspection
        ? skillsOf(inspection)
            .filter((s) => selected.has(s.dir))
            .map((s) => s.name ?? s.id)
        : [];
      toast.success(
        installed.length === 1 ? "Skill installed" : "Skills installed",
        {
          description:
            installed.length === 1
              ? `“${chosenNames[0] ?? installed[0].id}” was installed ${where}.`
              : `${installed.length} skills were installed ${where}.`,
          // Replacing trashed whatever was there, so say it plainly.
          icon: replace ? undefined : (
            <CheckCircle2 className="size-4 text-emerald-500" />
          ),
        },
      );
      if (replace) {
        toast.info("The previous contents were moved to the trash");
      }
      setConflict(null);
      close();
    },
    onError: (e) => {
      // A CONFLICT is resolvable here, so offer the choice instead of a dead
      // end. Everything else stays a plain error toast.
      if (isAppError(e) && e.code === "CONFLICT") {
        setConflict(e.detail ? `${e.message} ${e.detail}` : e.message);
        return;
      }
      toast.error("Installation failed", { description: errorDetail(e) });
    },
  });

  // Derived, not stored: a stuck phase flag can't leave the dialog unusable.
  const inspecting = inspect.isPending;
  const installing = install.isPending;
  const busy = inspecting || installing;

  const toggleSkill = (dir: string, checked: boolean) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (checked) next.add(dir);
      else next.delete(dir);
      return next;
    });
  };

  const pickLocal = async () => {
    const path = await pickFolder();
    if (path) setLocalPath(path);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      {/* Capped height + scroll keeps the dialog usable in short windows;
          overflow-hidden stops long content spilling outside the panel. */}
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto overflow-x-hidden sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Install Skill</DialogTitle>
          <DialogDescription>
            Files are copied into the agent&apos;s skills directory. Nothing
            from the source is executed.
          </DialogDescription>
        </DialogHeader>

        {/* flex-col, not grid: a single-column grid track sizes to the
            widest child's min-content and blows out the dialog width. */}
        <div className="flex min-w-0 flex-col gap-4">
          <SegmentedControl
            value={mode}
            onChange={(m) => {
              setMode(m);
              setInspection(null);
            }}
            options={[
              { value: "git", label: "Git Repository", icon: FolderGit2 },
              { value: "local", label: "Local Folder", icon: FolderOpen },
            ]}
          />

          {mode === "git" ? (
            <label className="flex min-w-0 flex-col gap-1.5">
              <span className="text-sm font-medium">Repository URL</span>
              <Input
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://github.com/example/react-skill"
                autoComplete="url"
                className="w-full min-w-0"
              />
            </label>
          ) : (
            <label className="flex min-w-0 flex-col gap-1.5">
              <span className="text-sm font-medium">Local folder</span>
              <div className="flex min-w-0 gap-2">
                <Input
                  value={localPath}
                  onChange={(e) => setLocalPath(e.target.value)}
                  placeholder="~/Projects/my-skill"
                  className="w-full min-w-0"
                />
                <Button
                  variant="outline"
                  onClick={pickLocal}
                  type="button"
                  className="shrink-0"
                >
                  <FolderOpen /> Browse
                </Button>
              </div>
            </label>
          )}

          {/* Two columns only when there's room; stacked in narrow windows. */}
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <label className="flex min-w-0 flex-col gap-1.5">
              <span className="text-sm font-medium">Install to</span>
              <Select
                value={scopeType}
                onValueChange={(v) => setScopeType(v as "global" | "project")}
              >
                <SelectTrigger aria-label="Install scope">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent position="popper" align="start">
                  <SelectItem value="global">
                    <Globe className="mr-1 inline size-3" /> Global
                  </SelectItem>
                  <SelectItem value="project">Project</SelectItem>
                </SelectContent>
              </Select>
            </label>

            {scopeType === "project" ? (
              <label className="flex min-w-0 flex-col gap-1.5">
                <span className="text-sm font-medium">Project</span>
                <Select
                  value={projectPath}
                  onValueChange={setProjectPath}
                  disabled={projects.isPending}
                >
                  <SelectTrigger aria-label="Target project">
                    <SelectValue placeholder="Choose…" />
                  </SelectTrigger>
                  <SelectContent position="popper" align="start">
                    {(projects.data ?? []).map((p) => (
                      <SelectItem key={p.id} value={p.path}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </label>
            ) : (
              <label className="flex min-w-0 flex-col gap-1.5">
                <span className="text-sm font-medium">Agent</span>
                <Select
                  value={effectiveProvider ?? undefined}
                  onValueChange={(v) => setProviderId(v as ProviderId)}
                >
                  <SelectTrigger aria-label="Target agent">
                    <SelectValue placeholder="Choose an agent…" />
                  </SelectTrigger>
                  <SelectContent position="popper" align="start">
                    {allProviders.map((p) => (
                      <SelectItem key={p.providerId} value={p.providerId}>
                        {p.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </label>
            )}
          </div>

          {inspection ? (
            <SkillSelection
              inspection={inspection}
              selected={selected}
              onToggle={toggleSkill}
            />
          ) : null}
        </div>

        <DialogFooter className="gap-2">
          <Button variant="ghost" onClick={close} disabled={busy}>
            Cancel
          </Button>
          {!inspection ? (
            <Button
              onClick={() => inspect.mutate()}
              disabled={
                !source || busy || (scopeType === "project" && !projectPath)
              }
            >
              {inspecting ? (
                <>
                  <Loader2 className="animate-spin" /> Inspecting…
                </>
              ) : (
                "Inspect"
              )}
            </Button>
          ) : (
            <Button
              onClick={() => install.mutate(false)}
              disabled={
                busy || selected.size === 0 || !scope || !effectiveProvider
              }
            >
              {installing ? (
                <>
                  <Loader2 className="animate-spin" /> Installing…
                </>
              ) : (
                `Install ${selected.size} skill${selected.size === 1 ? "" : "s"}`
              )}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>

      {/* The destination is occupied. Installing over it moves the existing
          contents to the trash, so it needs an explicit choice — never a
          silent overwrite (Rule 14). */}
      {conflict ? (
        <ConfirmDialog
          open
          onOpenChange={(next) => {
            if (!next) setConflict(null);
          }}
          title="That name is already taken"
          description="Installing here moves whatever is currently in this folder to your system trash, then installs the skill. You can restore the old contents from the trash."
          detail={conflict}
          confirmLabel="Move aside and install"
          busy={installing}
          onConfirm={() => install.mutate(true)}
        />
      ) : null}
    </Dialog>
  );
}

function SkillSelection({
  inspection,
  selected,
  onToggle,
}: {
  inspection: SourceInspection;
  selected: Set<string>;
  onToggle: (dir: string, checked: boolean) => void;
}) {
  const skills: DiscoveredSkill[] = skillsOf(inspection);
  return (
    <div className="min-w-0 rounded-md border p-3">
      <p className="mb-2 text-xs text-muted-foreground">
        {skills.length === 1
          ? "✓ Valid skill found"
          : `✓ Collection repo — ${skills.length} skills discovered (select which to install)`}
      </p>
      <div className="flex max-h-44 min-w-0 flex-col gap-2 overflow-y-auto">
        {skills.map((s) => (
          <label
            key={s.dir}
            className="flex min-w-0 cursor-pointer items-start gap-2 rounded p-1 text-sm hover:bg-accent/50"
          >
            <input
              type="checkbox"
              checked={selected.has(s.dir)}
              onChange={(e) => onToggle(s.dir, e.target.checked)}
              className="mt-0.5 shrink-0"
            />
            <span className="min-w-0 flex-1">
              <span className="block truncate font-medium">
                {s.name ?? s.id}
              </span>
              {s.description ? (
                <span className="block truncate text-xs text-muted-foreground">
                  {s.description}
                </span>
              ) : null}
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}

function SegmentedControl<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T;
  onChange: (value: T) => void;
  options: Array<{
    value: T;
    label: string;
    icon: React.ComponentType<{ className?: string }>;
  }>;
}) {
  return (
    <div className="flex min-w-0 rounded-md border p-1" role="tablist">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="tab"
          aria-selected={value === o.value}
          onClick={() => onChange(o.value)}
          className={`flex min-w-0 flex-1 items-center justify-center gap-1.5 truncate rounded px-2 py-1.5 text-sm transition-colors sm:px-3 ${
            value === o.value
              ? "bg-accent font-medium"
              : "text-muted-foreground hover:bg-accent/50"
          }`}
        >
          <o.icon className="size-4 shrink-0" />
          <span className="truncate">{o.label}</span>
        </button>
      ))}
    </div>
  );
}

function useProjects() {
  // Same query key as the Projects screen so caches stay coherent.
  return useQuery({ queryKey: ["projects"], queryFn: listProjects });
}

/**
 * Settings (plan 08 §8.2): diagnostics, not configuration.
 *
 * Phase 1 has no user preferences by design (§34 "minimal configuration"), so
 * this screen exists for the one action the tech-plan requires — Settings →
 * Open Logs (§31) — plus the environment facts that explain a broken install.
 */
import { useMutation, useQuery } from "@tanstack/react-query";
import { FolderOpen, FileWarning } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { errorDetail } from "@/features/skills/install";
import { invokeCommand } from "@/lib/tauri";

interface GitProbe {
  available: boolean;
  version?: string;
  errorDetail?: string;
}

export function SettingsPage() {
  // Git is only needed for Git installs, so a missing git is a warning the
  // user can act on rather than a blocking error.
  const git = useQuery({
    queryKey: ["git"],
    queryFn: () => invokeCommand<GitProbe>("check_git"),
    staleTime: 5 * 60_000,
  });

  const openLogs = useMutation({
    mutationFn: () => invokeCommand<string>("open_logs"),
    onSuccess: (path) => {
      toast.success("Opened the log folder", { description: path });
    },
    onError: (e) => {
      toast.error("Could not open logs", { description: errorDetail(e) });
    },
  });

  return (
    <main className="flex flex-1 flex-col gap-4 p-4 sm:p-6">
      <header>
        <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
        <p className="text-sm text-muted-foreground">
          Application preferences and diagnostics.
        </p>
      </header>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">Logs</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-2 text-sm text-muted-foreground">
          <p>
            Agent Manager writes a log file for every run, including packaged
            builds. It never records credentials, tokens, or skill file
            contents.
          </p>
          <Button
            variant="outline"
            className="w-fit"
            disabled={openLogs.isPending}
            onClick={() => openLogs.mutate()}
          >
            <FolderOpen /> Open Logs
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">Diagnostics</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-2 text-sm">
          <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:gap-4">
            <span className="shrink-0 text-xs uppercase tracking-wide text-muted-foreground sm:w-24">
              Git
            </span>
            <div className="min-w-0 break-all">
              {git.isPending ? (
                <span className="text-xs text-muted-foreground">Checking…</span>
              ) : git.isError ? (
                <span className="text-xs text-muted-foreground">
                  Could not check — {errorDetail(git.error)}
                </span>
              ) : git.data?.available ? (
                <Badge variant="outline">git {git.data.version}</Badge>
              ) : (
                <span className="flex items-center gap-1.5 text-xs">
                  <FileWarning className="size-3.5 text-amber-500" />
                  Not found. Installing from a Git URL requires git on your
                  PATH; local folders still work.
                </span>
              )}
            </div>
          </div>
        </CardContent>
      </Card>
    </main>
  );
}

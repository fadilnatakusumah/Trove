import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderPlus, Trash2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { addProject, listProjects, pickFolder, removeProject, type RegisteredProject } from "@/features/projects/projects";
import { ErrorState, EmptyState, LoadingState } from "@/features/states";

const projectsKey = ["projects"] as const;

export function ProjectsPage() {
  const queryClient = useQueryClient();
  const projects = useQuery({ queryKey: projectsKey, queryFn: listProjects });

  const invalidate = () => queryClient.invalidateQueries({ queryKey: projectsKey });

  const add = useMutation({
    mutationFn: async () => {
      const path = await pickFolder();
      if (!path) return null;
      return addProject(path);
    },
    onSuccess: (added) => {
      if (added) {
        toast.success(`Added project “${added.name}”`);
      }
      void invalidate();
    },
    onError: (e) => toast.error(e.message),
  });

  const remove = useMutation({
    mutationFn: removeProject,
    onSuccess: () => {
      toast.success("Project removed");
      void invalidate();
    },
    onError: (e) => toast.error(e.message),
  });

  if (projects.isPending) return <LoadingState label="Loading projects…" />;
  if (projects.isError)
    return <ErrorState message={projects.error?.message ?? "Failed to load projects"} />;

  const data = projects.data;

  return (
    <main className="flex flex-1 flex-col gap-4 p-4 sm:p-6">
      <header className="flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Projects</h1>
          <p className="text-sm text-muted-foreground">
            Registered project directories. Agent Manager only reads their
            agent configuration — nothing is copied or moved.
          </p>
        </div>
        <Button onClick={() => add.mutate()} disabled={add.isPending}>
          <FolderPlus /> Add Project
        </Button>
      </header>

      {data.length === 0 ? (
        <EmptyState
          title="No projects registered"
          hint="Add a project folder to see its installed agent skills."
        />
      ) : (
        <div className="grid gap-3">
          {data.map((p) => (
            <ProjectRow
              key={p.id}
              project={p}
              onRemove={() => remove.mutate(p.id)}
              removing={remove.isPending && remove.variables === p.id}
            />
          ))}
        </div>
      )}
    </main>
  );
}

function ProjectRow({
  project,
  onRemove,
  removing,
}: {
  project: RegisteredProject;
  onRemove: () => void;
  removing: boolean;
}) {
  return (
    <Card>
      <CardContent className="flex items-center justify-between py-3">
        <div className="min-w-0">
          <p className="truncate text-sm font-medium">{project.name}</p>
          <p className="truncate font-mono text-xs text-muted-foreground">
            {project.path}
          </p>
        </div>
        <Button
          variant="ghost"
          size="icon"
          aria-label={`Remove ${project.name}`}
          onClick={onRemove}
          disabled={removing}
        >
          <Trash2 className="text-muted-foreground" />
        </Button>
      </CardContent>
    </Card>
  );
}

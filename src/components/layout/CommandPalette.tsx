/**
 * Global command palette (⌘K / Ctrl+K) — plan 05 §5.6.
 * Navigation + jump-to-skill. Built on shadcn's Command (cmdk).
 */
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Blocks,
  FolderKanban,
  LayoutDashboard,
  Puzzle,
  Settings,
} from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { useProviders } from "@/features/providers/queries";
import { useAllSkills } from "@/features/skills/useAllSkills";
import { listProjects } from "@/features/projects/projects";
import { providerLabel } from "@/features/skills/skill-labels";
import type { SkillScope } from "@/types/skill";

const ROUTES = [
  { label: "Dashboard", to: "/", icon: LayoutDashboard },
  { label: "Skills", to: "/skills", icon: Blocks },
  { label: "Projects", to: "/projects", icon: FolderKanban },
  { label: "Providers", to: "/providers", icon: Puzzle },
  { label: "Settings", to: "/settings", icon: Settings },
] as const;

export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const providers = useProviders();
  const projects = useQuery({ queryKey: ["projects"], queryFn: listProjects });

  const combos = useMemo(() => {
    const ps = (providers.data ?? []).map((p) => p.providerId);
    const scopes = [
      { type: "global" } as const,
      ...(projects.data ?? []).map(
        (p) => ({ type: "project", projectPath: p.path }) as SkillScope,
      ),
    ];
    return ps.flatMap((providerId) => scopes.map((scope) => ({ providerId, scope })));
  }, [providers.data, projects.data]);

  // Fetch only while open — the palette shouldn't drive background disk reads.
  const { data: skills = [] } = useAllSkills(combos, open);

  const go = (to: string) => {
    setOpen(false);
    navigate(to);
  };

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput placeholder="Type a command or search skills…" />
      <CommandList>
        <CommandEmpty>No results found.</CommandEmpty>
        <CommandGroup heading="Navigate">
          {ROUTES.map((r) => (
            <CommandItem key={r.to} onSelect={() => go(r.to)}>
              <r.icon /> {r.label}
            </CommandItem>
          ))}
        </CommandGroup>
        {skills.length > 0 ? (
          <>
            <CommandSeparator />
            <CommandGroup heading="Skills">
              {skills.map((s) => (
                <CommandItem
                  key={`${s.provider}-${s.path}`}
                  value={`${s.name} ${providerLabel(s.provider)} ${s.id}`}
                  onSelect={() => go(`/skills/${encodeURIComponent(s.path)}`)}
                >
                  <Blocks /> {s.name}
                  <span className="ml-auto text-xs text-muted-foreground">
                    {providerLabel(s.provider)}
                  </span>
                </CommandItem>
              ))}
            </CommandGroup>
          </>
        ) : null}
      </CommandList>
    </CommandDialog>
  );
}

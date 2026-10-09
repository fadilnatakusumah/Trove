import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import { errorDetail } from "@/features/skills/install";
import { invokeCommand } from "@/lib/tauri";
import type { ProviderId, Skill, SkillScope } from "@/types/skill";

/**
 * Toggle a skill. Disabling renames its `SKILL.md` and enabling renames it
 * back, so the action is lossless and works for every provider (§20).
 */
export async function setSkillEnabled(args: {
  providerId: ProviderId;
  skillId: string;
  scope: SkillScope;
  enabled: boolean;
}): Promise<void> {
  return invokeCommand<void>("set_skill_enabled", args);
}

/**
 * Remove a skill. Moves it to the OS trash by default (§21); `permanent`
 * deletes it outright and is only ever set on explicit user confirmation.
 */
export async function uninstallSkill(args: {
  providerId: ProviderId;
  skillId: string;
  scope: SkillScope;
  permanent?: boolean;
}): Promise<void> {
  // Send the flag explicitly: the command expects it, and an omitted argument
  // is a confusing deserialization error rather than the safe default.
  return invokeCommand<void>("uninstall_skill", {
    ...args,
    permanent: args.permanent ?? false,
  });
}

/**
 * Refresh the skills matrix after a mutation. Both hooks below invalidate
 * rather than editing the cache — the filesystem stays the source of truth
 * (§26).
 */
function useSkillsRefresh() {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: ["skills-all"] });
  };
}

/** Enable/disable for one skill. Used by the list row and the detail view. */
export function useSkillToggle(skill: Skill) {
  const refresh = useSkillsRefresh();

  return useMutation({
    mutationFn: () =>
      setSkillEnabled({
        providerId: skill.provider,
        skillId: skill.id,
        scope: skill.scope,
        enabled: !skill.enabled,
      }),
    onSuccess: () => {
      refresh();
      toast.success(skill.enabled ? "Skill disabled" : "Skill enabled", {
        description: skill.enabled
          ? `“${skill.name}” is skipped until you enable it again.`
          : `“${skill.name}” is active again.`,
      });
    },
    onError: (e) =>
      toast.error("Could not change that", { description: errorDetail(e) }),
  });
}

/** Uninstall for one skill. `onRemoved` runs after the skill is gone. */
export function useSkillRemoval(skill: Skill, onRemoved?: () => void) {
  const refresh = useSkillsRefresh();

  return useMutation({
    mutationFn: (permanent: boolean) =>
      uninstallSkill({
        providerId: skill.provider,
        skillId: skill.id,
        scope: skill.scope,
        permanent,
      }),
    onSuccess: (_data, permanent) => {
      refresh();
      toast.success("Skill uninstalled", {
        description: permanent
          ? `“${skill.name}” was deleted permanently.`
          : `“${skill.name}” was moved to the trash.`,
      });
      onRemoved?.();
    },
    onError: (e) =>
      toast.error("Uninstall failed", { description: errorDetail(e) }),
  });
}

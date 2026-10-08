/**
 * Skills fetching (plan 05).
 *
 * Rules-of-hooks-safe: one query for the whole provider × scope matrix, with
 * Promise.all inside the queryFn — never a hook per combo (hook count would
 * change when providers/projects load and crash the tree).
 */
import { useQuery } from "@tanstack/react-query";

import { invokeCommand } from "@/lib/tauri";
import type { ProviderId, Skill, SkillScope } from "@/types/skill";

export interface Combo {
  providerId: ProviderId;
  scope: SkillScope;
}

export function combosKey(combos: Combo[]): string {
  return JSON.stringify(combos);
}

export function useAllSkills(combos: Combo[], enabled = true) {
  return useQuery({
    queryKey: ["skills-all", combosKey(combos)],
    queryFn: async (): Promise<Skill[]> => {
      const results = await Promise.all(
        combos.map((c) =>
          invokeCommand<Skill[]>("get_skills", {
            providerId: c.providerId,
            scope: c.scope,
          }).catch(() => [] as Skill[]),
        ),
      );
      return results.flat();
    },
    enabled: enabled && combos.length > 0,
    staleTime: 30_000,
    // A combo list that's momentarily empty should not flash as an error.
    placeholderData: [],
  });
}

/**
 * Provider query hook (plan 05). Skills fetching lives in
 * `features/skills/useAllSkills.ts` (single-matrix hook).
 */
import { useQuery } from "@tanstack/react-query";

import { invokeCommand } from "@/lib/tauri";
import type { ProviderId } from "@/types/skill";

export interface ProviderDetection {
  providerId: ProviderId;
  name: string;
  detected: boolean;
  globalSkillsDir?: string;
  supportsDisable: boolean;
}

export const queryKeys = {
  providers: ["providers"] as const,
};

export function useProviders() {
  return useQuery({
    queryKey: queryKeys.providers,
    queryFn: () =>
      invokeCommand<ProviderDetection[]>("detect_providers"),
    staleTime: 60_000,
  });
}

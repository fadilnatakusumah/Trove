/**
 * Uninstall confirmation for one skill (plan 07 §7.3, §21).
 *
 * Owns the mutation so the list row and the detail view behave identically.
 * Mounted only while a target is selected — that keeps the mutation hook
 * unconditional instead of guarding a hook behind a nullable value.
 */
import { ConfirmDialog } from "@/features/skills/ConfirmDialog";
import { useSkillRemoval } from "@/features/skills/lifecycle";
import type { Skill } from "@/types/skill";

export function UninstallDialog({
  skill,
  onClose,
  onRemoved,
}: {
  skill: Skill;
  /** Dismissed without removing anything. */
  onClose: () => void;
  /** Removed successfully — e.g. navigate away from the skill's page. */
  onRemoved?: () => void;
}) {
  const remove = useSkillRemoval(skill, () => {
    onRemoved?.();
    onClose();
  });

  return (
    <ConfirmDialog
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      title={`Uninstall “${skill.name}”?`}
      description="The skill folder is removed. By default it goes to your system trash, where you can restore it."
      detail={skill.path}
      confirmLabel="Move to trash"
      permanentOption
      busy={remove.isPending}
      onConfirm={(permanent) => remove.mutate(permanent)}
    />
  );
}
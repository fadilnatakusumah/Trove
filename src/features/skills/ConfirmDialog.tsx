/**
 * Destructive-action confirmation (plan 07, §21).
 * Shows the exact path and states plainly what will happen — required before
 * any filesystem mutation (§22 rule 8).
 */
import { useState } from "react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  detail,
  confirmLabel = "Remove",
  permanentOption = false,
  onConfirm,
  busy = false,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  /** Concrete path/identifier shown in mono type so the user sees the target. */
  detail?: string;
  confirmLabel?: string;
  /** Offer the "delete permanently" escalation (uninstall only). */
  permanentOption?: boolean;
  onConfirm: (permanent: boolean) => void;
  busy?: boolean;
}) {
  const [permanent, setPermanent] = useState(false);

  // Reset the escalation on dismiss so the destructive choice never carries
  // into the next confirmation. Callers that unmount the dialog between uses
  // get this for free; this covers the ones that keep it mounted.
  const handleOpenChange = (next: boolean) => {
    if (!next) setPermanent(false);
    onOpenChange(next);
  };

  return (
    <AlertDialog open={open} onOpenChange={handleOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          <AlertDialogDescription>{description}</AlertDialogDescription>
        </AlertDialogHeader>
        {detail ? (
          <p className="rounded-md bg-muted px-3 py-2 font-mono text-xs break-all">
            {detail}
          </p>
        ) : null}
        {permanentOption ? (
          <label className="flex items-start gap-2 text-sm">
            <input
              type="checkbox"
              className="mt-0.5 size-4 accent-destructive"
              checked={permanent}
              onChange={(e) => setPermanent(e.target.checked)}
              disabled={busy}
            />
            <span>
              Delete permanently instead
              <span className="block text-xs text-muted-foreground">
                Skips the trash — this cannot be recovered.
              </span>
            </span>
          </label>
        ) : null}
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            onClick={(e) => {
              // Keep the dialog mounted while the mutation runs so the busy
              // state is visible; the parent closes it on success.
              e.preventDefault();
              onConfirm(permanent);
            }}
            disabled={busy}
          >
            {busy ? "Working…" : permanent ? "Delete permanently" : confirmLabel}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

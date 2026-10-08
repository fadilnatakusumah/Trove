/**
 * Error/empty/loading states shared by list screens (plan 08 sweep applied
 * from day one per plan 05).
 */
import { AlertTriangle, Inbox, Loader2 } from "lucide-react";

export function LoadingState({ label }: { label: string }) {
  return (
    <div className="flex flex-1 items-center justify-center gap-2 p-10 text-sm text-muted-foreground">
      <Loader2 className="size-4 animate-spin" /> {label}
    </div>
  );
}

export function EmptyState({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-1 p-10 text-center">
      <Inbox className="mb-2 size-8 text-muted-foreground/50" />
      <p className="text-sm font-medium">{title}</p>
      {hint ? <p className="text-xs text-muted-foreground">{hint}</p> : null}
    </div>
  );
}

export function ErrorState({ message }: { message: string }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-1 p-10 text-center">
      <AlertTriangle className="mb-2 size-8 text-destructive/70" />
      <p className="text-sm font-medium">Something went wrong</p>
      <p className="max-w-md text-xs text-muted-foreground">{message}</p>
    </div>
  );
}

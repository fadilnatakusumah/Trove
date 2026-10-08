import { CheckCircle2, CircleDashed } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useProviders } from "@/features/providers/queries";
import { ErrorState, LoadingState } from "@/features/states";

export function ProvidersPage() {
  const { data, isPending, isError, error } = useProviders();

  if (isPending) return <LoadingState label="Detecting agents…" />;
  if (isError)
    return <ErrorState message={error?.message ?? "Detection failed"} />;

  const detectedCount = data.filter((p) => p.detected).length;

  return (
    <main className="flex flex-1 flex-col gap-4 p-6">
      <header>
        <h1 className="text-2xl font-semibold tracking-tight">Providers</h1>
        <p className="text-sm text-muted-foreground">
          {detectedCount} of {data.length} supported AI coding agents detected
          on this machine.
        </p>
      </header>

      <div className="grid gap-3">
        {data.map((p) => (
          <Card key={p.providerId}>
            <CardHeader className="pb-2">
              <CardTitle className="flex items-center gap-2 text-base">
                {p.detected ? (
                  <CheckCircle2 className="size-4 text-emerald-500" />
                ) : (
                  <CircleDashed className="size-4 text-muted-foreground/50" />
                )}
                {p.name}
                <Badge variant={p.detected ? "default" : "outline"}>
                  {p.detected ? "Detected" : "Not found"}
                </Badge>
              </CardTitle>
            </CardHeader>
            <CardContent className="text-sm text-muted-foreground">
              <p className="font-mono text-xs">{p.globalSkillsDir}</p>
              <p className="mt-1 text-xs">
                {p.supportsDisable
                  ? "Enable/disable supported"
                  : "Disable is not supported for this provider"}
              </p>
            </CardContent>
          </Card>
        ))}
      </div>
    </main>
  );
}

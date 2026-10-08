/**
 * Error boundary (plan 05/08 polish). A render error in any screen would
 * otherwise unmount the whole tree and leave a blank window with only a
 * console message — this surfaces it and offers a way back.
 */
import { Component, type ErrorInfo, type ReactNode } from "react";
import { AlertTriangle, RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Console only: no telemetry in Phase 1 (§4/§31).
    console.error("Render error:", error, info.componentStack);
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    return (
      <div className="flex h-svh flex-col items-center justify-center gap-3 p-8 text-center">
        <AlertTriangle className="size-10 text-destructive/70" />
        <div>
          <p className="text-base font-semibold">Something went wrong</p>
          <p className="mt-1 max-w-md text-sm text-muted-foreground">
            {error.message}
          </p>
        </div>
        <Button onClick={() => this.setState({ error: null })}>
          <RotateCcw /> Try again
        </Button>
      </div>
    );
  }
}

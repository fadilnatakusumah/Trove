/**
 * Typed Tauri invoke wrapper (plan 02).
 *
 * The only place the frontend talks to the Rust backend. Maps rejected
 * invocations into the structured `AppError` type instead of raw strings.
 */
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { isAppError, type AppError } from "@/types/error";

/**
 * Call a Tauri command. The rejection is normalized: Tauri 2 serializes
 * `AppError` structs into the rejection payload, but older/plain-string
 * errors are wrapped into a generic AppError so callers always get one shape.
 */
export async function invokeCommand<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (raw) {
    throw normalizeError(raw);
  }
}

function normalizeError(raw: unknown): AppError {
  if (isAppError(raw)) return raw;
  // Tauri may hand back a JSON-encoded string payload.
  if (typeof raw === "string") {
    try {
      const parsed: unknown = JSON.parse(raw);
      if (isAppError(parsed)) return parsed;
    } catch {
      // not JSON — fall through
    }
  }
  return {
    code: "INSTALL_FAILED",
    message: "Agent Manager encountered an unexpected error.",
    detail: String(raw),
  };
}

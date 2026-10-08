/**
 * Structured error types (plan 02, tech-plan §25). Mirror of
 * `src-tauri/src/errors/mod.rs`.
 */

export type ErrorCode =
  | "PROVIDER_NOT_FOUND"
  | "SKILL_NOT_FOUND"
  | "INVALID_SKILL"
  | "INSTALL_FAILED"
  | "PERMISSION_DENIED"
  /** Documented addition beyond §25: path-traversal guard (plan 03). */
  | "PATH_ESCAPE"
  | "CONFLICT"
  | "GIT_MISSING"
  | "UNSUPPORTED";

export interface AppError {
  code: ErrorCode;
  message: string;
  detail?: string;
}

export function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.code === "string" && typeof v.message === "string";
}

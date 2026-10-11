/**
 * Path display helpers.
 *
 * Paths are values: they get compared, encoded into routes, and sent back to
 * Rust. Displaying one is a separate concern, and the backend owns it — it is
 * the only side that knows the home directory, so `~`-shortening happens there
 * (see `filesystem_service::display_path`, surfaced as `displayPath` on
 * projects and `path` on skills).
 *
 * This module is the frontend's safety net for the few places a raw path still
 * reaches the UI. It cannot shorten `~`, but it does guarantee the one thing
 * the user actually notices: consistent separators.
 */

/**
 * Render a path for display: forward slashes only, never a mix.
 *
 * Windows paths arrive as `C:\Users\me\.config/opencode/skills\frontend-design`
 * — backslashes from the OS, forward slashes from anywhere we joined segments
 * ourselves. Forward slashes are unambiguous on every platform.
 */
export function formatPath(path: string): string {
  return path.replace(/\\/g, "/");
}

/** Last segment of a path, for a compact label. */
export function pathBasename(path: string): string {
  return formatPath(path).split("/").filter(Boolean).pop() ?? path;
}

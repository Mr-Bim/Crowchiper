/**
 * Keyboard shortcuts for inserting dates in YYYY-MM-DD format.
 *
 * Shortcuts:
 * - Ctrl/Cmd+Shift+D: Insert today's date
 * - Ctrl/Cmd+Shift+Y: Insert yesterday's date
 * - Ctrl/Cmd+Shift+T: Insert tomorrow's date
 */

import { keymap } from "@codemirror/view";

/**
 * Format a date as YYYY-MM-DD.
 */
function formatDate(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/**
 * Get today's date in YYYY-MM-DD format.
 */
export function getToday(): string {
  return formatDate(new Date());
}

/**
 * Get yesterday's date in YYYY-MM-DD format.
 */
export function getYesterday(): string {
  const date = new Date();
  date.setDate(date.getDate() - 1);
  return formatDate(date);
}

/**
 * Get tomorrow's date in YYYY-MM-DD format.
 */
export function getTomorrow(): string {
  const date = new Date();
  date.setDate(date.getDate() + 1);
  return formatDate(date);
}

/**
 * Keyboard shortcuts for inserting dates.
 */
export const dateShortcuts = keymap.of([
  {
    key: "Mod-Shift-d",
    run(view) {
      const date = getToday();
      const { from, to } = view.state.selection.main;
      view.dispatch({
        changes: { from, to, insert: date },
        selection: { anchor: from + date.length },
      });
      return true;
    },
  },
  {
    key: "Mod-Shift-y",
    run(view) {
      const date = getYesterday();
      const { from, to } = view.state.selection.main;
      view.dispatch({
        changes: { from, to, insert: date },
        selection: { anchor: from + date.length },
      });
      return true;
    },
  },
  {
    key: "Mod-Shift-t",
    run(view) {
      const date = getTomorrow();
      const { from, to } = view.state.selection.main;
      view.dispatch({
        changes: { from, to, insert: date },
        selection: { anchor: from + date.length },
      });
      return true;
    },
  },
]);

/**
 * Editor setup and lifecycle management.
 *
 * Handles creating, reusing, and resetting the CodeMirror editor instance.
 * Extracted from selection.ts for clearer separation of concerns.
 */

import { applySpellcheckToEditor } from "../spellcheck.ts";
import { getEditor, getLoadedPost, setEditor } from "./state/index.ts";
import { scheduleEncrypt } from "./save.ts";

// Lazy-load editor chunk - only starts when setupEditor is first called
let editorPromise: Promise<typeof import("../editor/setup.ts")> | null = null;

function getEditorModule() {
  if (!editorPromise) {
    editorPromise = import("../editor/setup.ts");
  }
  return editorPromise;
}

/**
 * Set up the editor with the given content.
 * Reuses existing editor if available, otherwise creates a new one.
 */
export async function setupEditor(
  container: HTMLElement,
  content: string,
): Promise<void> {
  const existingEditor = getEditor();
  const { createEditor, resetEditorContent } = await getEditorModule();

  const onDocChange = () => {
    if (getLoadedPost()) {
      scheduleEncrypt();
    }
  };

  if (existingEditor) {
    // Reuse existing editor - just reset its content and state
    resetEditorContent(existingEditor, content);
  } else {
    // Clear container before creating (handles leftover DOM from destroyed editors)
    container.innerHTML = "";
    // Create the editor
    const newEditor = createEditor(container, content, onDocChange);
    setEditor(newEditor);
  }

  applySpellcheckToEditor();
}

/**
 * Destroy the current editor instance.
 * Used when deleting posts or clearing the editor area.
 */
export function destroyEditor(): void {
  const editor = getEditor();
  if (editor) {
    editor.destroy();
    setEditor(null);
  }
}

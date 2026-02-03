import { completionKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import {
  defaultHighlightStyle,
  syntaxHighlighting,
} from "@codemirror/language";
import { type Extension } from "@codemirror/state";
import { EditorState } from "@codemirror/state";
import { drawSelection, EditorView, keymap } from "@codemirror/view";
import { attachmentPlugin } from "./attachment-widget/index.ts";
import { checkboxPlugin } from "./checkbox-widget.ts";
import { dateShortcuts } from "./date-shortcuts.ts";
import { slashCommands } from "./slash-commands.ts";
import "../../css/cm-editor.css";
import "../../css/cm-slash-commands.css";

declare const __TEST_MODE__: boolean;

/**
 * Build the extensions array for the editor.
 * Extracted so it can be reused when resetting editor state.
 */
function buildExtensions(onDocChange: () => void): Extension[] {
  const extensions: Extension[] = [
    history(),
    drawSelection(),
    syntaxHighlighting(defaultHighlightStyle),
    markdown(),
    attachmentPlugin, // Must come before default keymap to intercept Backspace/Delete
    dateShortcuts, // Date insertion shortcuts (must come before default keymap)
    keymap.of([...completionKeymap, ...defaultKeymap, ...historyKeymap]),
    EditorView.lineWrapping,
    slashCommands,
    checkboxPlugin,
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onDocChange();
      }
    }),
  ];
  if (__TEST_MODE__) {
    extensions.push(
      EditorView.contentAttributes.of({ "data-testid": "test-editor-content" }),
    );
  }
  return extensions;
}

// Store the onDocChange callback so we can rebuild extensions when resetting
let currentOnDocChange: (() => void) | null = null;

export function createEditor(
  container: HTMLElement,
  content: string,
  onDocChange: () => void,
): EditorView {
  currentOnDocChange = onDocChange;

  const state = EditorState.create({
    doc: content,
    extensions: buildExtensions(onDocChange),
  });

  return new EditorView({
    state,
    parent: container,
  });
}

/**
 * Reset the editor with new content.
 * This replaces the entire state (including undo history) without destroying the view.
 * More efficient than destroy + recreate as it reuses DOM elements.
 */
export function resetEditorContent(editor: EditorView, content: string): void {
  if (!currentOnDocChange) {
    throw new Error("Editor was not created with createEditor");
  }

  const newState = EditorState.create({
    doc: content,
    extensions: buildExtensions(currentOnDocChange),
  });

  editor.setState(newState);
}

export { EditorView };

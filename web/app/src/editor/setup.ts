import { completionKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import {
  defaultHighlightStyle,
  syntaxHighlighting,
} from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { drawSelection, EditorView, keymap } from "@codemirror/view";
import { attachmentPlugin } from "./attachment-widget/index.ts";
import { checkboxPlugin } from "./checkbox-widget.ts";
import { dateShortcuts } from "./date-shortcuts.ts";
import { slashCommands } from "./slash-commands.ts";
import "../../editor.css";

export function createEditor(
  container: HTMLElement,
  content: string,
  onDocChange: () => void,
): EditorView {
  const state = EditorState.create({
    doc: content,
    extensions: [
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
    ],
  });

  return new EditorView({
    state,
    parent: container,
  });
}

export { EditorView };

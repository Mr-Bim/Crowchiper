import { completionKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import {
	defaultHighlightStyle,
	syntaxHighlighting,
} from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { drawSelection, EditorView, keymap } from "@codemirror/view";
import { attachmentPlugin } from "./attachment-widget.ts";
import { checkboxPlugin } from "./checkbox-widget.ts";
import { slashCommands } from "./slash-commands.ts";

const editorTheme = EditorView.theme({
	"&": {
		height: "100%",
		fontSize: "16px",
	},
	".cm-scroller": {
		overflow: "auto",
		fontFamily: "inherit",
	},
	".cm-content": {
		padding: "16px",
		caretColor: "var(--accent)",
	},
	".cm-line": {
		padding: "0",
	},
	"&.cm-focused .cm-cursor": {
		borderLeftColor: "var(--accent)",
	},
	"&.cm-focused .cm-selectionBackground, ::selection": {
		backgroundColor: "oklch(0.67 0.16 55 / 0.2)",
	},
	".cm-activeLine": {
		backgroundColor: "transparent",
	},
	".cm-checkbox-widget": {
		width: "1em",
		height: "1em",
		marginRight: "0.5em",
		verticalAlign: "middle",
		cursor: "pointer",
		accentColor: "var(--accent)",
	},
	".cm-attachment-widget": {
		display: "block",
		margin: "8px 0",
	},
	".cm-attachment-picker": {
		display: "inline-block",
	},
	".cm-attachment-picker-btn": {
		padding: "8px 16px",
		background: "var(--accent)",
		color: "white",
		border: "none",
		borderRadius: "4px",
		cursor: "pointer",
		fontSize: "14px",
	},
	".cm-attachment-picker-btn:hover": {
		opacity: "0.9",
	},
	".cm-attachment-picker-btn:disabled": {
		opacity: "0.6",
		cursor: "wait",
	},
	".cm-attachment-thumbnail-wrapper": {
		display: "inline-block",
	},
	".cm-attachment-thumbnail": {
		maxWidth: "100%",
		width: "auto",
		height: "auto",
		borderRadius: "4px",
		cursor: "pointer",
		transition: "opacity 0.15s",
		display: "block",
	},
	".cm-attachment-thumbnail:hover": {
		opacity: "0.85",
	},
	".cm-attachment-loading": {
		padding: "16px",
		color: "var(--text-muted)",
		fontStyle: "italic",
	},
	".cm-attachment-error": {
		padding: "16px",
		color: "var(--error, #dc3545)",
	},
});

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
			keymap.of([...completionKeymap, ...defaultKeymap, ...historyKeymap]),
			EditorView.lineWrapping,
			editorTheme,
			slashCommands,
			checkboxPlugin,
			attachmentPlugin,
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

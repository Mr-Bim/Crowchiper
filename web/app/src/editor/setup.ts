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
	// Gallery styles
	".cm-gallery-start, .cm-gallery-end": {
		display: "none",
	},
	".cm-gallery-image": {
		display: "inline-block",
		position: "relative",
		margin: "4px",
		verticalAlign: "top",
	},
	".cm-gallery-image .cm-attachment-thumbnail-wrapper": {
		display: "inline-block",
	},
	".cm-gallery-image .cm-attachment-thumbnail": {
		maxHeight: "200px",
		width: "auto",
	},
	".cm-gallery-delete-btn": {
		position: "absolute",
		top: "4px",
		right: "4px",
		width: "24px",
		height: "24px",
		padding: "0",
		border: "none",
		borderRadius: "50%",
		background: "rgba(0, 0, 0, 0.6)",
		color: "white",
		cursor: "pointer",
		display: "flex",
		alignItems: "center",
		justifyContent: "center",
		opacity: "0",
		transition: "opacity 0.15s",
	},
	".cm-gallery-image:hover .cm-gallery-delete-btn, .cm-gallery-delete-btn:focus": {
		opacity: "1",
	},
	".cm-gallery-delete-btn:focus": {
		outline: "2px solid var(--accent)",
		outlineOffset: "2px",
	},
	".cm-gallery-delete-btn:hover": {
		background: "rgba(220, 53, 69, 0.9)",
	},
	".cm-gallery-delete-btn svg": {
		width: "14px",
		height: "14px",
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
			attachmentPlugin, // Must come before default keymap to intercept Backspace/Delete
			keymap.of([...completionKeymap, ...defaultKeymap, ...historyKeymap]),
			EditorView.lineWrapping,
			editorTheme,
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

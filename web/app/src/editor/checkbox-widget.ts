/**
 * Checkbox widget decorations for CodeMirror.
 *
 * Replaces markdown checkbox syntax (- [ ] and - [x]) with
 * interactive checkbox inputs that toggle the state on click.
 */

import {
	Decoration,
	type DecorationSet,
	EditorView,
	ViewPlugin,
	type ViewUpdate,
	WidgetType,
} from "@codemirror/view";

class CheckboxWidget extends WidgetType {
	private checkbox: HTMLInputElement | null = null;
	private handler: ((e: MouseEvent) => void) | null = null;

	constructor(
		private checked: boolean,
		private from: number,
		private to: number,
	) {
		super();
	}

	eq(other: CheckboxWidget): boolean {
		return this.checked === other.checked;
	}

	toDOM(view: EditorView): HTMLElement {
		const checkbox = document.createElement("input");
		checkbox.type = "checkbox";
		checkbox.checked = this.checked;
		checkbox.className = "cm-checkbox-widget";
		checkbox.setAttribute("aria-label", this.checked ? "Completed task" : "Incomplete task");

		this.handler = (e: MouseEvent) => {
			e.preventDefault();
			const newText = this.checked ? "- [ ] " : "- [x] ";
			view.dispatch({
				changes: { from: this.from, to: this.to, insert: newText },
			});
		};
		checkbox.addEventListener("mousedown", this.handler);
		this.checkbox = checkbox;

		return checkbox;
	}

	destroy(): void {
		if (this.checkbox && this.handler) {
			this.checkbox.removeEventListener("mousedown", this.handler);
			this.checkbox = null;
			this.handler = null;
		}
	}

	ignoreEvent(): boolean {
		return false;
	}
}

function buildDecorations(view: EditorView): DecorationSet {
	const decorations: Array<{ from: number; to: number; deco: Decoration }> = [];
	const doc = view.state.doc;

	// Pattern matches "- [ ] " or "- [x] " at line start (with optional leading whitespace)
	const checkboxPattern = /^(\s*)- \[([ xX])\] /;

	for (let i = 1; i <= doc.lines; i++) {
		const line = doc.line(i);
		const match = line.text.match(checkboxPattern);

		if (match) {
			const leadingSpaces = match[1].length;
			const isChecked = match[2].toLowerCase() === "x";
			const from = line.from + leadingSpaces;
			const to = line.from + match[0].length;

			decorations.push({
				from,
				to,
				deco: Decoration.replace({
					widget: new CheckboxWidget(isChecked, from, to),
				}),
			});
		}
	}

	return Decoration.set(
		decorations.map((d) => d.deco.range(d.from, d.to)),
	);
}

export const checkboxPlugin = ViewPlugin.fromClass(
	class {
		decorations: DecorationSet;

		constructor(view: EditorView) {
			this.decorations = buildDecorations(view);
		}

		update(update: ViewUpdate) {
			if (update.docChanged || update.viewportChanged) {
				this.decorations = buildDecorations(update.view);
			}
		}
	},
	{
		decorations: (v) => v.decorations,
	},
);

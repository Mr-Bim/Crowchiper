/**
 * Keyboard handlers for gallery navigation and editing.
 */

import { StateField } from "@codemirror/state";
import {
  EditorView,
  keymap,
} from "@codemirror/view";

// Pattern matches ::gallery{}...:: with images inside
const GALLERY_PATTERN =
  /::gallery\{([^}]*)\}((?:!\[[^\]]*\]\(attachment:[a-zA-Z0-9-]+\))+)::/g;

// ============================================================================
// Gallery line tracking
// ============================================================================

function findGalleryLines(doc: {
  lines: number;
  line: (n: number) => { text: string };
}): Set<number> {
  const galleryLines = new Set<number>();

  for (let i = 1; i <= doc.lines; i++) {
    const line = doc.line(i);
    GALLERY_PATTERN.lastIndex = 0;
    if (GALLERY_PATTERN.test(line.text)) {
      galleryLines.add(i);
    }
  }

  return galleryLines;
}

/**
 * State field that caches gallery line numbers.
 * Galleries occupy entire lines, so we just track which lines have them.
 * Rescans on any document change (cheap since we're just checking line patterns).
 */
export const galleryLines = StateField.define<Set<number>>({
  create(state) {
    return findGalleryLines(state.doc);
  },
  update(lines, tr) {
    if (tr.docChanged) {
      return findGalleryLines(tr.newDoc);
    }
    return lines;
  },
});

// ============================================================================
// Keyboard handler helpers
// ============================================================================

/** Context for gallery keyboard handlers */
interface GalleryContext {
  view: EditorView;
  doc: EditorView["state"]["doc"];
  cursorLine: { number: number; from: number; to: number; text: string };
  galleries: Set<number>;
  from: number;
}

/** Get context for gallery handlers, or null if selection is not collapsed */
function getGalleryContext(view: EditorView): GalleryContext | null {
  const { from, to } = view.state.selection.main;
  if (from !== to) return null;

  return {
    view,
    doc: view.state.doc,
    cursorLine: view.state.doc.lineAt(from),
    galleries: view.state.field(galleryLines),
    from,
  };
}

/** Swap current line with gallery line above it */
function swapWithGalleryAbove(ctx: GalleryContext): boolean {
  const { view, doc, cursorLine } = ctx;
  if (cursorLine.number <= 1) return false;

  const prevLineNum = cursorLine.number - 1;
  if (!ctx.galleries.has(prevLineNum)) return false;

  const prevLine = doc.line(prevLineNum);
  view.dispatch({
    changes: {
      from: prevLine.from,
      to: cursorLine.to,
      insert: cursorLine.text + "\n" + prevLine.text,
    },
    selection: { anchor: prevLine.from },
  });
  return true;
}

/** Swap current line with gallery line below it */
function swapWithGalleryBelow(ctx: GalleryContext): boolean {
  const { view, doc, cursorLine } = ctx;
  if (cursorLine.number >= doc.lines) return false;

  const nextLineNum = cursorLine.number + 1;
  if (!ctx.galleries.has(nextLineNum)) return false;

  const nextLine = doc.line(nextLineNum);
  view.dispatch({
    changes: {
      from: cursorLine.from,
      to: nextLine.to,
      insert: nextLine.text + "\n" + cursorLine.text,
    },
    selection: {
      anchor:
        cursorLine.from + nextLine.text.length + 1 + cursorLine.text.length,
    },
  });
  return true;
}

/** Skip over gallery above (for up/left navigation) */
function skipGalleryAbove(ctx: GalleryContext): boolean {
  const { view, doc, cursorLine } = ctx;
  if (cursorLine.number <= 1) return false;

  const prevLineNum = cursorLine.number - 1;
  if (!ctx.galleries.has(prevLineNum)) return false;

  if (prevLineNum > 1) {
    view.dispatch({ selection: { anchor: doc.line(prevLineNum - 1).to } });
  } else {
    view.dispatch({ selection: { anchor: 0 } });
  }
  return true;
}

/** Skip over gallery below (for down/right navigation) */
function skipGalleryBelow(ctx: GalleryContext): boolean {
  const { view, doc, cursorLine } = ctx;
  if (cursorLine.number >= doc.lines) return false;

  const nextLineNum = cursorLine.number + 1;
  if (!ctx.galleries.has(nextLineNum)) return false;

  if (nextLineNum < doc.lines) {
    view.dispatch({ selection: { anchor: doc.line(nextLineNum + 1).from } });
  } else {
    view.dispatch({ selection: { anchor: doc.length } });
  }
  return true;
}

// ============================================================================
// Keyboard handler
// ============================================================================

/**
 * Custom keyboard handler for gallery images.
 * Prevents editing gallery lines and handles navigation around them.
 */
export const galleryKeyHandler = keymap.of([
  {
    key: "Backspace",
    run(view) {
      const ctx = getGalleryContext(view);
      if (!ctx) return false;

      // On gallery line at end: move to start
      if (
        ctx.galleries.has(ctx.cursorLine.number) &&
        ctx.from === ctx.cursorLine.to
      ) {
        view.dispatch({ selection: { anchor: ctx.cursorLine.from } });
        return true;
      }

      // At start of line after gallery: swap lines
      if (ctx.from === ctx.cursorLine.from) {
        return swapWithGalleryAbove(ctx);
      }

      return false;
    },
  },
  {
    key: "Delete",
    run(view) {
      const ctx = getGalleryContext(view);
      if (!ctx) return false;

      // On gallery line at start: move to end
      if (
        ctx.galleries.has(ctx.cursorLine.number) &&
        ctx.from === ctx.cursorLine.from
      ) {
        view.dispatch({ selection: { anchor: ctx.cursorLine.to } });
        return true;
      }

      // At end of line before gallery: swap lines
      if (ctx.from === ctx.cursorLine.to) {
        return swapWithGalleryBelow(ctx);
      }

      return false;
    },
  },
  {
    key: "Mod-Backspace",
    run(view) {
      const ctx = getGalleryContext(view);
      if (!ctx) return false;

      // On gallery line: move to start
      if (ctx.galleries.has(ctx.cursorLine.number)) {
        view.dispatch({ selection: { anchor: ctx.cursorLine.from } });
        return true;
      }

      // At start of line after gallery: swap lines
      if (ctx.from === ctx.cursorLine.from) {
        return swapWithGalleryAbove(ctx);
      }

      return false;
    },
  },
  {
    key: "ArrowUp",
    run(view) {
      const ctx = getGalleryContext(view);
      return ctx ? skipGalleryAbove(ctx) : false;
    },
  },
  {
    key: "ArrowDown",
    run(view) {
      const ctx = getGalleryContext(view);
      return ctx ? skipGalleryBelow(ctx) : false;
    },
  },
  {
    key: "ArrowLeft",
    run(view) {
      const ctx = getGalleryContext(view);
      if (!ctx) return false;
      // Only skip when at start of line
      return ctx.from === ctx.cursorLine.from ? skipGalleryAbove(ctx) : false;
    },
  },
  {
    key: "ArrowRight",
    run(view) {
      const ctx = getGalleryContext(view);
      if (!ctx) return false;
      // Only skip when at end of line
      return ctx.from === ctx.cursorLine.to ? skipGalleryBelow(ctx) : false;
    },
  },
]);

// ============================================================================
// Cursor guard and input handler
// ============================================================================

/**
 * Update listener that ensures cursor doesn't end up inside a gallery.
 * If cursor is on a gallery line, move it to the end (before newline).
 * Only runs when selection changes WITHOUT a doc change (click/arrow navigation).
 */
export const galleryCursorGuard = EditorView.updateListener.of((update) => {
  // Only guard on pure selection changes (clicks, arrow keys)
  // Not when typing (docChanged) - the atomic ranges handle that
  if (!update.selectionSet || update.docChanged) return;

  const { from, to } = update.state.selection.main;

  // Only handle collapsed selections (cursor, not range)
  if (from !== to) return;

  const doc = update.state.doc;
  const cursorLine = doc.lineAt(from);
  const galleries = update.state.field(galleryLines);

  // If cursor is on a gallery line, always move to end of line
  if (galleries.has(cursorLine.number) && from !== cursorLine.to) {
    update.view.dispatch({
      selection: { anchor: cursorLine.to },
    });
  }
});

/**
 * Input handler that redirects typing on gallery lines to the next line.
 * When user types on a gallery line, insert a newline first, then the text.
 */
export const galleryInputHandler = EditorView.inputHandler.of(
  (view, from, _to, text) => {
    // Only handle text input (not deletions)
    if (!text) return false;

    const doc = view.state.doc;
    const cursorLine = doc.lineAt(from);
    const galleries = view.state.field(galleryLines);

    // If typing on a gallery line, redirect to next line
    if (galleries.has(cursorLine.number)) {
      // Insert at end of gallery line with a newline prefix
      view.dispatch({
        changes: {
          from: cursorLine.to,
          to: cursorLine.to,
          insert: "\n" + text,
        },
        selection: { anchor: cursorLine.to + 1 + text.length },
      });
      return true;
    }

    return false;
  },
);

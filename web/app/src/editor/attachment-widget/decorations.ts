/**
 * CodeMirror decorations and plugins for gallery rendering.
 */

import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
} from "@codemirror/view";

import { GalleryContainerWidget, type GalleryImage } from "./widget.ts";

// Pattern matches ::gallery{}...:: with images inside
const GALLERY_PATTERN =
  /::gallery\{([^}]*)\}((?:!\[[^\]]*\]\(attachment:[a-zA-Z0-9-]+\))+)::/g;

// Pattern for extracting individual images from gallery content
const GALLERY_IMAGE_PATTERN = /!\[([^\]]*)\]\(attachment:([a-zA-Z0-9-]+)\)/g;

// ============================================================================
// Build decorations
// ============================================================================

function buildDecorations(view: EditorView): DecorationSet {
  const decorations: Array<{ from: number; to: number; deco: Decoration }> = [];
  const doc = view.state.doc;

  for (let i = 1; i <= doc.lines; i++) {
    const line = doc.line(i);
    let match: RegExpExecArray | null;
    GALLERY_PATTERN.lastIndex = 0;

    while ((match = GALLERY_PATTERN.exec(line.text)) !== null) {
      const galleryStart = line.from + match.index;
      const galleryEnd = galleryStart + match[0].length;
      const configPart = match[1];
      const imagesContent = match[2];

      // Calculate positions
      const prefixEnd =
        galleryStart + "::gallery{".length + configPart.length + "}".length;

      // Find all images
      const imagesStart = prefixEnd;
      let imgMatch: RegExpExecArray | null;
      GALLERY_IMAGE_PATTERN.lastIndex = 0;

      const imagePositions: GalleryImage[] = [];
      while ((imgMatch = GALLERY_IMAGE_PATTERN.exec(imagesContent)) !== null) {
        imagePositions.push({
          from: imagesStart + imgMatch.index,
          to: imagesStart + imgMatch.index + imgMatch[0].length,
          alt: imgMatch[1],
          uuid: imgMatch[2],
        });
      }

      // Replace the entire gallery with a single container widget
      decorations.push({
        from: galleryStart,
        to: galleryEnd,
        deco: Decoration.replace({
          widget: new GalleryContainerWidget(
            imagePositions,
            galleryStart,
            galleryEnd,
          ),
        }),
      });
    }
  }

  // Sort by position for CodeMirror
  decorations.sort((a, b) => a.from - b.from);

  return Decoration.set(decorations.map((d) => d.deco.range(d.from, d.to)));
}

export const galleryViewPlugin = ViewPlugin.fromClass(
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

// ============================================================================
// Atomic ranges
// ============================================================================

function buildAtomicRanges(view: EditorView): DecorationSet {
  const ranges: Array<{ from: number; to: number }> = [];
  const doc = view.state.doc;

  for (let i = 1; i <= doc.lines; i++) {
    const line = doc.line(i);
    let match: RegExpExecArray | null;
    GALLERY_PATTERN.lastIndex = 0;

    while ((match = GALLERY_PATTERN.exec(line.text)) !== null) {
      const galleryStart = line.from + match.index;
      const galleryEnd = galleryStart + match[0].length;

      // Make the entire gallery atomic as one unit
      ranges.push({ from: galleryStart, to: galleryEnd });
    }
  }

  // Sort by position
  ranges.sort((a, b) => a.from - b.from);

  return Decoration.set(
    ranges.map((r) => Decoration.mark({ atomic: true }).range(r.from, r.to)),
  );
}

export const atomicRangesPlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;

    constructor(view: EditorView) {
      this.decorations = buildAtomicRanges(view);
    }

    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = buildAtomicRanges(update.view);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
    provide: (plugin) =>
      EditorView.atomicRanges.of((view) => {
        const value = view.plugin(plugin);
        return value?.decorations ?? Decoration.none;
      }),
  },
);

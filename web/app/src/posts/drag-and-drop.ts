/**
 * Drag and drop functionality for post reordering.
 * Uses @atlaskit/pragmatic-drag-and-drop.
 */

import {
	draggable,
	dropTargetForElements,
	monitorForElements,
} from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachClosestEdge,
	type Edge,
	extractClosestEdge,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/closest-edge";

type ReorderCallback = (fromIndex: number, toIndex: number) => void;

// Track cleanup functions to avoid duplicate listeners
let cleanupFns: (() => void)[] = [];

export function initDragAndDrop(
	container: HTMLElement,
	onReorder: ReorderCallback,
): void {
	// Clean up previous drag and drop setup
	for (const cleanup of cleanupFns) {
		cleanup();
	}
	cleanupFns = [];

	const items = Array.from(
		container.querySelectorAll<HTMLElement>("[data-index]"),
	);

	for (const item of items) {
		// Make each item draggable
		const dragCleanup = draggable({
			element: item,
			getInitialData: () => ({
				index: Number(item.getAttribute("data-index")),
				uuid: item.getAttribute("data-uuid"),
			}),
			onDragStart: () => {
				item.setAttribute("data-dragging", "");
			},
			onDrop: () => {
				item.removeAttribute("data-dragging");
			},
		});
		cleanupFns.push(dragCleanup);

		// Make each item a drop target
		const dropCleanup = dropTargetForElements({
			element: item,
			getData: ({ input, element }) => {
				const data = {
					index: Number(element.getAttribute("data-index")),
				};
				return attachClosestEdge(data, {
					input,
					element,
					allowedEdges: ["top", "bottom"],
				});
			},
			onDragEnter: ({ self }) => {
				const edge = extractClosestEdge(self.data);
				updateDropIndicator(item, edge);
			},
			onDrag: ({ self }) => {
				const edge = extractClosestEdge(self.data);
				updateDropIndicator(item, edge);
			},
			onDragLeave: () => {
				clearDropIndicator(item);
			},
			onDrop: () => {
				clearDropIndicator(item);
			},
		});
		cleanupFns.push(dropCleanup);
	}

	// Monitor for completed drops
	const monitorCleanup = monitorForElements({
		onDrop: ({ source, location }) => {
			const destination = location.current.dropTargets[0];
			if (!destination) return;

			const fromIndex = source.data.index as number;
			const toIndex = destination.data.index as number;
			const edge = extractClosestEdge(destination.data);

			// Calculate final index based on drop edge
			let finalIndex = toIndex;
			if (edge === "bottom") {
				finalIndex = toIndex + 1;
			}

			// Adjust if moving down
			if (fromIndex < finalIndex) {
				finalIndex -= 1;
			}

			if (fromIndex !== finalIndex) {
				onReorder(fromIndex, finalIndex);
			}
		},
	});
	cleanupFns.push(monitorCleanup);
}

function updateDropIndicator(element: HTMLElement, edge: Edge | null): void {
	clearDropIndicator(element);
	if (edge === "top") {
		element.setAttribute("data-drop-top", "");
	} else if (edge === "bottom") {
		element.setAttribute("data-drop-bottom", "");
	}
}

function clearDropIndicator(element: HTMLElement): void {
	element.removeAttribute("data-drop-top");
	element.removeAttribute("data-drop-bottom");
}

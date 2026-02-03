/**
 * Drag and drop functionality for post reordering and reparenting.
 * Uses @atlaskit/pragmatic-drag-and-drop.
 *
 * Supports two drop modes:
 * - Reorder: Drop on top/bottom edge to reorder siblings
 * - Reparent: Drop on center to make child of target post
 */

import {
  draggable,
  dropTargetForElements,
  monitorForElements,
} from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import { attachClosestEdge } from "@atlaskit/pragmatic-drag-and-drop-hitbox/closest-edge";
import { getSiblings } from "./state/index.ts";
import { getOptionalElement } from "../../../shared/dom.ts";

// UUID v4 regex pattern
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Validate that a string is a valid UUID v4 format.
 */
function isValidUuid(value: string): boolean {
  return UUID_PATTERN.test(value);
}

type ReorderCallback = (
  parentId: string | null,
  fromIndex: number,
  toIndex: number,
) => void;
type ReparentCallback = (
  uuid: string,
  newParentId: string | null,
  position: number,
) => void;

// Track cleanup functions to avoid duplicate listeners
let cleanupFns: (() => void)[] = [];

// Threshold for center drop zone (percentage of element height from edges)
const CENTER_ZONE_THRESHOLD = 0.3;

type DropMode = "reorder-top" | "reorder-bottom" | "reparent";

/**
 * Determine drop mode based on cursor position within element.
 */
function getDropMode(element: HTMLElement, clientY: number): DropMode {
  const rect = element.getBoundingClientRect();
  const relativeY = clientY - rect.top;
  const height = rect.height;

  // Top zone
  if (relativeY < height * CENTER_ZONE_THRESHOLD) {
    return "reorder-top";
  }
  // Bottom zone
  if (relativeY > height * (1 - CENTER_ZONE_THRESHOLD)) {
    return "reorder-bottom";
  }
  // Center zone - reparent
  return "reparent";
}

export function initDragAndDrop(
  container: HTMLElement,
  onReorder: ReorderCallback,
  onReparent: ReparentCallback,
): void {
  if (!getOptionalElement(container.id)) return;

  // Clean up previous drag and drop setup
  for (const cleanup of cleanupFns) {
    cleanup();
  }
  cleanupFns = [];

  const items = Array.from(
    container.querySelectorAll<HTMLElement>("[data-post-uuid]"),
  );

  for (const item of items) {
    const uuid = item.getAttribute("data-post-uuid") ?? "";
    const parentId = item.getAttribute("data-parent-id") || null;

    // Validate UUID before using
    if (!isValidUuid(uuid)) {
      console.warn("Invalid UUID in drag-and-drop item:", uuid);
      continue;
    }

    // Validate parent ID if present
    if (parentId && !isValidUuid(parentId)) {
      console.warn("Invalid parent UUID in drag-and-drop item:", parentId);
      continue;
    }

    // Make each item draggable
    const dragCleanup = draggable({
      element: item,
      getInitialData: () => {
        // Get index within siblings
        const siblings = getSiblings(uuid);
        const index = siblings.findIndex((s) => s.uuid === uuid);
        return {
          uuid,
          parentId,
          index,
        };
      },
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
        const targetUuid = element.getAttribute("data-post-uuid") ?? "";
        const targetParentId = element.getAttribute("data-parent-id") || null;

        // Validate UUIDs
        if (!isValidUuid(targetUuid)) {
          return {};
        }
        if (targetParentId && !isValidUuid(targetParentId)) {
          return {};
        }

        // Get index within siblings
        const siblings = getSiblings(targetUuid);
        const index = siblings.findIndex((s) => s.uuid === targetUuid);

        const data = {
          uuid: targetUuid,
          parentId: targetParentId,
          index,
        };
        return attachClosestEdge(data, {
          input,
          element,
          allowedEdges: ["top", "bottom"],
        });
      },
      canDrop: ({ source }) => {
        // Cannot drop on self
        return source.data.uuid !== item.getAttribute("data-post-uuid");
      },
      onDragEnter: ({ location }) => {
        const mode = getDropMode(item, location.current.input.clientY);
        updateDropIndicator(item, mode);
      },
      onDrag: ({ location }) => {
        const mode = getDropMode(item, location.current.input.clientY);
        updateDropIndicator(item, mode);
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

      const fromUuid = source.data.uuid as string;
      const fromParentId = source.data.parentId as string | null;
      const fromIndex = source.data.index as number;

      const toUuid = destination.data.uuid as string;
      const toParentId = destination.data.parentId as string | null;
      const toIndex = destination.data.index as number;

      // Validate all UUIDs before processing
      if (!isValidUuid(fromUuid) || !isValidUuid(toUuid)) {
        console.warn("Invalid UUID in drag-and-drop operation");
        return;
      }
      if (fromParentId && !isValidUuid(fromParentId)) {
        console.warn("Invalid from parent UUID in drag-and-drop operation");
        return;
      }
      if (toParentId && !isValidUuid(toParentId)) {
        console.warn("Invalid to parent UUID in drag-and-drop operation");
        return;
      }

      // Get drop mode based on last input position
      const targetElement = destination.element as HTMLElement;
      const clientY = location.current.input.clientY;
      const mode = getDropMode(targetElement, clientY);

      if (mode === "reparent") {
        // Make the dragged post a child of the target post
        onReparent(fromUuid, toUuid, 0);
      } else {
        // Reordering
        const edge = mode === "reorder-top" ? "top" : "bottom";

        // Check if same parent
        if (fromParentId === toParentId) {
          // Reorder within same parent
          let finalIndex = toIndex;
          if (edge === "bottom") {
            finalIndex = toIndex + 1;
          }

          // Adjust if moving down
          if (fromIndex < finalIndex) {
            finalIndex -= 1;
          }

          if (fromIndex !== finalIndex) {
            onReorder(fromParentId, fromIndex, finalIndex);
          }
        } else {
          // Moving to different parent - use reparent with position
          let position = toIndex;
          if (edge === "bottom") {
            position = toIndex + 1;
          }
          onReparent(fromUuid, toParentId, position);
        }
      }
    },
  });
  cleanupFns.push(monitorCleanup);
}

function updateDropIndicator(element: HTMLElement, mode: DropMode): void {
  clearDropIndicator(element);
  if (mode === "reorder-top") {
    element.setAttribute("data-drop-top", "");
  } else if (mode === "reorder-bottom") {
    element.setAttribute("data-drop-bottom", "");
  } else if (mode === "reparent") {
    element.setAttribute("data-drop-child", "");
  }
}

function clearDropIndicator(element: HTMLElement): void {
  element.removeAttribute("data-drop-top");
  element.removeAttribute("data-drop-bottom");
  element.removeAttribute("data-drop-child");
}

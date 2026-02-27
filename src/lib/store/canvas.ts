/**
 * Canvas viewport state — pan, zoom, selection, hover.
 * Kept separate from model.ts because it changes at 60fps
 * and must never trigger model re-renders.
 */
import { writable, derived } from 'svelte/store';

export interface Viewport {
  x: number;      // pan offset X
  y: number;      // pan offset Y
  scale: number;  // zoom level (1.0 = 100%)
}

export interface SelectionState {
  nodeIds: Set<string>;
  edgeIds: Set<string>;
}

export const viewport = writable<Viewport>({ x: 0, y: 0, scale: 1 });
export const selection = writable<SelectionState>({ nodeIds: new Set(), edgeIds: new Set() });
export const hoveredNodeId = writable<string | null>(null);
export const activeDiagramId = writable<string | null>(null);

// Derived: is anything selected?
export const hasSelection = derived(
  selection,
  ($s) => $s.nodeIds.size > 0 || $s.edgeIds.size > 0
);

// Derived: CSS transform string for the canvas container
export const transformCSS = derived(viewport, ($vp) =>
  `translate(${$vp.x}px, ${$vp.y}px) scale(${$vp.scale})`
);

// ── Actions ───────────────────────────────────────────────────────────────────

export function pan(dx: number, dy: number) {
  viewport.update((vp) => ({ ...vp, x: vp.x + dx, y: vp.y + dy }));
}

export function zoom(delta: number, originX: number, originY: number) {
  viewport.update((vp) => {
    const factor = delta > 0 ? 1.1 : 0.9;
    const newScale = Math.min(2.5, Math.max(0.1, vp.scale * factor));
    // Zoom toward the cursor position
    const scaleRatio = newScale / vp.scale;
    return {
      x: originX - scaleRatio * (originX - vp.x),
      y: originY - scaleRatio * (originY - vp.y),
      scale: newScale,
    };
  });
}

export function resetViewport() {
  viewport.set({ x: 0, y: 0, scale: 1 });
}

export function selectNode(nodeId: string, additive = false) {
  selection.update((s) => {
    if (additive) {
      const next = new Set(s.nodeIds);
      if (next.has(nodeId)) {
        next.delete(nodeId);
      } else {
        next.add(nodeId);
      }
      return { ...s, nodeIds: next };
    }
    return { nodeIds: new Set([nodeId]), edgeIds: new Set() };
  });
}

export function selectEdge(edgeId: string, additive = false) {
  selection.update((s) => {
    if (additive) {
      const next = new Set(s.edgeIds);
      if (next.has(edgeId)) {
        next.delete(edgeId);
      } else {
        next.add(edgeId);
      }
      return { ...s, edgeIds: next };
    }
    return { nodeIds: new Set(), edgeIds: new Set([edgeId]) };
  });
}

export function clearSelection() {
  selection.set({ nodeIds: new Set(), edgeIds: new Set() });
}

export function selectAll(nodeIds: string[]) {
  selection.set({ nodeIds: new Set(nodeIds), edgeIds: new Set() });
}

/** Convert canvas-space coordinates to model-space (accounting for pan+zoom) */
export function canvasToModel(
  canvasX: number,
  canvasY: number,
  vp: Viewport
): { x: number; y: number } {
  return {
    x: (canvasX - vp.x) / vp.scale,
    y: (canvasY - vp.y) / vp.scale,
  };
}

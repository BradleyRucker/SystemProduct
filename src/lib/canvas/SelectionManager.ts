/**
 * SelectionManager — decoupled selection and drag coordination.
 * PixiCanvas owns the instance; NodeRenderer calls into it.
 * The Svelte store (canvas.ts) is the source of truth for selection state.
 */
import {
  selectNode as storeSelectNode,
  selectEdge as storeSelectEdge,
  clearSelection,
} from '$lib/store/canvas';

interface Callbacks {
  onNodeSelected: (nodeId: string, additive: boolean) => void;
  onEdgeSelected: (edgeId: string, additive: boolean) => void;
  onEmptyClick: () => void;
  onNodeMoved: (nodeId: string, x: number, y: number) => void;
}

export class SelectionManager {
  private callbacks: Callbacks;
  private selectedNodes = new Set<string>();
  private selectedEdges = new Set<string>();

  constructor(callbacks: Callbacks) {
    this.callbacks = callbacks;
  }

  selectNode(nodeId: string, additive: boolean) {
    if (!additive) {
      this.selectedNodes.clear();
      this.selectedEdges.clear();
    }
    this.selectedNodes.add(nodeId);
    storeSelectNode(nodeId, additive);
    this.callbacks.onNodeSelected(nodeId, additive);
  }

  selectEdge(edgeId: string, additive: boolean) {
    if (!additive) {
      this.selectedNodes.clear();
      this.selectedEdges.clear();
    }
    this.selectedEdges.add(edgeId);
    storeSelectEdge(edgeId, additive);
    this.callbacks.onEdgeSelected(edgeId, additive);
  }

  clear() {
    this.selectedNodes.clear();
    this.selectedEdges.clear();
    clearSelection();
  }

  isNodeSelected(nodeId: string): boolean {
    return this.selectedNodes.has(nodeId);
  }

  isEdgeSelected(edgeId: string): boolean {
    return this.selectedEdges.has(edgeId);
  }

  notifyNodeMoved(nodeId: string, x: number, y: number) {
    this.callbacks.onNodeMoved(nodeId, x, y);
  }

  get selectedNodeIds(): string[] {
    return [...this.selectedNodes];
  }
}

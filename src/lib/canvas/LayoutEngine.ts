/**
 * ELK layout engine wrapper.
 * elk.js runs in a web worker; this module manages the worker lifecycle
 * and provides a typed promise-based API.
 */
import ELK from 'elkjs/lib/elk.bundled.js';
import type { IRNode, IREdge } from '$lib/types';

const elk = new ELK();

export interface LayoutResult {
  positions: Map<string, { x: number; y: number; width: number; height: number }>;
  waypoints: Map<string, { x: number; y: number }[]>;
}

export async function autoLayout(
  diagramId: string,
  nodes: IRNode[],
  edges: IREdge[],
  options: Record<string, string> = {}
): Promise<LayoutResult> {
  const graph = {
    id: diagramId,
    layoutOptions: {
      'elk.algorithm': 'layered',
      'elk.direction': 'DOWN',
      'elk.spacing.nodeNode': '32',
      'elk.layered.spacing.nodeNodeBetweenLayers': '48',
      'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
      ...options,
    },
    children: nodes.map((n) => ({
      id: n.id,
      width: n.width,
      height: n.height,
    })),
    edges: edges
      .filter((e) => nodes.some((n) => n.id === e.source_id) && nodes.some((n) => n.id === e.target_id))
      .map((e) => ({
        id: e.id,
        sources: [e.source_id],
        targets: [e.target_id],
      })),
  };

  const result = await elk.layout(graph);

  const positions = new Map<string, { x: number; y: number; width: number; height: number }>();
  for (const child of result.children ?? []) {
    positions.set(child.id!, {
      x: child.x ?? 0,
      y: child.y ?? 0,
      width: child.width ?? 120,
      height: child.height ?? 60,
    });
  }

  const waypoints = new Map<string, { x: number; y: number }[]>();
  for (const edge of result.edges ?? []) {
    const pts: { x: number; y: number }[] = [];
    for (const section of (edge as any).sections ?? []) {
      if (section.bendPoints) {
        pts.push(...section.bendPoints);
      }
    }
    waypoints.set(edge.id!, pts);
  }

  return { positions, waypoints };
}

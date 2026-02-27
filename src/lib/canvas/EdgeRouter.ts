import * as PIXI from "pixi.js";
import type { IREdge, IRNode } from "$lib/types";

const EDGE_COLORS: Record<string, number> = {
  satisfies:        0x3b82f6,
  refines:          0x3b82f6,
  allocates:        0x38bdf8,
  realizes:         0xa78bfa,
  traces:           0x64748b,
  verifies:         0x22c55e,
  connects:         0xf59e0b,
  composes:         0x64748b,
  specializes:      0x64748b,
  derives:          0x8b5cf6,
  transition:       0x16a34a,
  binding_connector:0xea580c,
};

type ArrowStyle = "open" | "filled" | "diamond" | "none";
const ARROWHEAD: Record<string, ArrowStyle> = {
  satisfies:        "open",
  refines:          "open",
  allocates:        "open",
  realizes:         "open",
  traces:           "open",
  verifies:         "filled",
  connects:         "none",
  composes:         "diamond",
  specializes:      "filled",
  derives:          "open",
  transition:       "filled",
  binding_connector:"none",
};

type LineStyle = "solid" | "dashed";
const LINE_STYLE: Record<string, LineStyle> = {
  satisfies:        "dashed",
  refines:          "dashed",
  allocates:        "dashed",
  realizes:         "dashed",
  traces:           "dashed",
  verifies:         "dashed",
  connects:         "solid",
  composes:         "solid",
  specializes:      "solid",
  derives:          "dashed",
  transition:       "solid",
  binding_connector:"dashed",
};

export class EdgeRouter {
  private layer: PIXI.Container;
  private graphics = new Map<string, PIXI.Graphics>();
  private labels   = new Map<string, PIXI.Text>();
  private onEdgeSelected?: (edgeId: string) => void;
  private onEdgeContextMenu?: (edgeId: string, x: number, y: number) => void;

  constructor(
    layer: PIXI.Container,
    onEdgeSelected?: (edgeId: string) => void,
    onEdgeContextMenu?: (edgeId: string, x: number, y: number) => void,
  ) {
    this.layer = layer;
    this.onEdgeSelected = onEdgeSelected;
    this.onEdgeContextMenu = onEdgeContextMenu;
  }

  render(edges: IREdge[], nodes: IRNode[]) {
    const nodeMap = new Map(nodes.map((n) => [n.id, n]));
    const seen = new Set<string>();

    for (const edge of edges) {
      seen.add(edge.id);
      let g = this.graphics.get(edge.id);
      if (!g) {
        g = new PIXI.Graphics();
        g.eventMode = "static";
        g.cursor = "pointer";
        const edgeId = edge.id;

        g.on("pointerdown", (e: PIXI.FederatedPointerEvent) => {
          if (e.button !== 0) return;
          e.stopPropagation();
          this.onEdgeSelected?.(edgeId);
        });

        g.on("rightclick", (e: PIXI.FederatedPointerEvent) => {
          e.stopPropagation();
          this.onEdgeSelected?.(edgeId);
          this.onEdgeContextMenu?.(edgeId, e.clientX, e.clientY);
        });

        this.layer.addChild(g);
        this.graphics.set(edge.id, g);
      }

      g.clear();
      const labelPos = this.drawEdge(g, edge, nodeMap);

      // ── Edge label (transition guard/action or generic label) ──────────────
      const labelText = edge.label?.trim() ?? "";
      if (labelText) {
        let t = this.labels.get(edge.id);
        if (!t) {
          t = new PIXI.Text("", {
            fontSize: 11,
            fill: 0xcbd5e1,
            fontFamily: "monospace",
          });
          t.eventMode = "none";
          this.layer.addChild(t);
          this.labels.set(edge.id, t);
        }
        // For transitions format as: event [guard] / action
        // Users type the full label in the edge label field; we just display it.
        t.text = labelText;
        t.x = labelPos.x - t.width / 2;
        t.y = labelPos.y - t.height - 4;
        t.visible = true;
      } else {
        const t = this.labels.get(edge.id);
        if (t) t.visible = false;
      }
    }

    for (const [id, g] of this.graphics) {
      if (!seen.has(id)) {
        this.layer.removeChild(g);
        g.destroy();
        this.graphics.delete(id);
        const t = this.labels.get(id);
        if (t) {
          this.layer.removeChild(t);
          t.destroy();
          this.labels.delete(id);
        }
      }
    }
  }

  private drawEdge(
    g: PIXI.Graphics,
    edge: IREdge,
    nodeMap: Map<string, IRNode>,
  ): { x: number; y: number } {
    const src = nodeMap.get(edge.source_id);
    const tgt = nodeMap.get(edge.target_id);
    const fallback = { x: 0, y: 0 };
    if (!src || !tgt) return fallback;

    const color = EDGE_COLORS[edge.kind] ?? 0x64748b;
    const alpha = edge.has_suggestion ? 0.42 : 0.84;
    const points = this.routePoints(src, tgt, edge.waypoints);

    g.lineStyle(12, 0xffffff, 0.001);
    g.moveTo(points[0].x, points[0].y);
    for (let i = 1; i < points.length; i++) {
      g.lineTo(points[i].x, points[i].y);
    }

    g.lineStyle(4, color, 0.12);
    g.moveTo(points[0].x, points[0].y);
    for (let i = 1; i < points.length; i++) {
      g.lineTo(points[i].x, points[i].y);
    }

    if (LINE_STYLE[edge.kind] === "dashed") {
      this.drawDashed(g, points, color, alpha);
    } else {
      g.lineStyle(2, color, alpha);
      g.moveTo(points[0].x, points[0].y);
      for (let i = 1; i < points.length; i++) {
        g.lineTo(points[i].x, points[i].y);
      }
    }

    const tip = points[points.length - 1];
    const prev = points[points.length - 2];
    const angle = Math.atan2(tip.y - prev.y, tip.x - prev.x);
    this.drawArrowhead(g, tip, angle, ARROWHEAD[edge.kind] ?? "open", color, alpha);

    // Return midpoint of the longest segment for label placement
    const midIdx = Math.floor(points.length / 2);
    const pa = points[midIdx - 1] ?? points[0];
    const pb = points[midIdx]     ?? points[points.length - 1];
    return { x: (pa.x + pb.x) / 2, y: (pa.y + pb.y) / 2 };
  }

  private routePoints(
    src: IRNode,
    tgt: IRNode,
    waypoints: { x: number; y: number }[],
  ): { x: number; y: number }[] {
    const srcCx = src.x + src.width / 2;
    const srcCy = src.y + src.height / 2;
    const tgtCx = tgt.x + tgt.width / 2;
    const tgtCy = tgt.y + tgt.height / 2;

    if (waypoints.length > 0) {
      return [{ x: srcCx, y: srcCy }, ...waypoints, { x: tgtCx, y: tgtCy }];
    }

    const dx = tgtCx - srcCx;
    const dy = tgtCy - srcCy;

    const srcPt = edgePoint(src, dx, dy);
    const tgtPt = edgePoint(tgt, -dx, -dy);

    const points: { x: number; y: number }[] = [srcPt];

    if (Math.abs(srcPt.x - tgtPt.x) >= Math.abs(srcPt.y - tgtPt.y)) {
      const midX = (srcPt.x + tgtPt.x) / 2;
      points.push({ x: midX, y: srcPt.y });
      points.push({ x: midX, y: tgtPt.y });
    } else {
      const midY = (srcPt.y + tgtPt.y) / 2;
      points.push({ x: srcPt.x, y: midY });
      points.push({ x: tgtPt.x, y: midY });
    }

    points.push(tgtPt);
    return compactPoints(points);
  }

  private drawDashed(
    g: PIXI.Graphics,
    points: { x: number; y: number }[],
    color: number,
    alpha: number,
  ) {
    const dashLen = 8;
    const gapLen = 5;

    g.lineStyle(2, color, alpha);

    for (let i = 0; i < points.length - 1; i++) {
      const x0 = points[i].x;
      const y0 = points[i].y;
      const x1 = points[i + 1].x;
      const y1 = points[i + 1].y;
      const segLen = Math.hypot(x1 - x0, y1 - y0);
      if (segLen <= 0) continue;

      const ux = (x1 - x0) / segLen;
      const uy = (y1 - y0) / segLen;

      let d = 0;
      let draw = true;
      while (d < segLen) {
        const run = Math.min(draw ? dashLen : gapLen, segLen - d);
        if (draw) {
          const sx = x0 + ux * d;
          const sy = y0 + uy * d;
          g.moveTo(sx, sy);
          g.lineTo(sx + ux * run, sy + uy * run);
        }
        d += run;
        draw = !draw;
      }
    }
  }

  private drawArrowhead(
    g: PIXI.Graphics,
    tip: { x: number; y: number },
    angle: number,
    style: ArrowStyle,
    color: number,
    alpha: number,
  ) {
    if (style === "none") return;

    const size = 8;
    const spread = Math.PI / 6;

    const lx = tip.x - size * Math.cos(angle - spread);
    const ly = tip.y - size * Math.sin(angle - spread);
    const rx = tip.x - size * Math.cos(angle + spread);
    const ry = tip.y - size * Math.sin(angle + spread);

    g.lineStyle(2, color, alpha);

    if (style === "filled") {
      g.beginFill(color, alpha);
      g.moveTo(tip.x, tip.y);
      g.lineTo(lx, ly);
      g.lineTo(rx, ry);
      g.closePath();
      g.endFill();
      return;
    }

    if (style === "open") {
      g.moveTo(lx, ly);
      g.lineTo(tip.x, tip.y);
      g.lineTo(rx, ry);
      return;
    }

    const back = size * 1.4;
    const bx = tip.x - back * Math.cos(angle);
    const by = tip.y - back * Math.sin(angle);
    g.beginFill(color, alpha);
    g.moveTo(tip.x, tip.y);
    g.lineTo(lx, ly);
    g.lineTo(bx, by);
    g.lineTo(rx, ry);
    g.closePath();
    g.endFill();
  }
}

function edgePoint(node: IRNode, dx: number, dy: number): { x: number; y: number } {
  const cx = node.x + node.width / 2;
  const cy = node.y + node.height / 2;

  if (Math.abs(dx) > Math.abs(dy)) {
    if (dx > 0) return { x: node.x + node.width, y: cy };
    return { x: node.x, y: cy };
  }

  if (dy > 0) return { x: cx, y: node.y + node.height };
  return { x: cx, y: node.y };
}

function compactPoints(points: { x: number; y: number }[]): { x: number; y: number }[] {
  if (points.length <= 2) return points;

  const out: { x: number; y: number }[] = [points[0]];
  const eps = 0.001;

  for (let i = 1; i < points.length - 1; i++) {
    const prev = out[out.length - 1];
    const curr = points[i];
    const next = points[i + 1];

    const sameX = Math.abs(prev.x - curr.x) < eps && Math.abs(curr.x - next.x) < eps;
    const sameY = Math.abs(prev.y - curr.y) < eps && Math.abs(curr.y - next.y) < eps;
    const duplicate = Math.abs(prev.x - curr.x) < eps && Math.abs(prev.y - curr.y) < eps;

    if (!sameX && !sameY && !duplicate) {
      out.push(curr);
    }
  }

  out.push(points[points.length - 1]);
  return out;
}

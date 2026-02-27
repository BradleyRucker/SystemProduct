import * as PIXI from "pixi.js";
import type { IRNode } from "$lib/types";
import type { SelectionManager } from "$lib/canvas/SelectionManager";

type NodePalette = {
  fill: number; // light tinted body fill
  border: number; // full-saturation border color
  accent: number; // header fill (slightly darker tint)
  text: number; // primary text (near-black)
  muted: number; // secondary / stereotype label text
  chip: number; // ID chip background
};

// Light, clean palettes — inspired by Trace.Space / Cameo / Jama style.
// Bodies are near-white with a subtle color wash. Borders carry the full hue.
const KIND_COLORS: Record<string, NodePalette> = {
  requirement: {
    fill: 0xf0f5ff,
    accent: 0xdbeafe,
    border: 0x3b82f6,
    text: 0x0f172a,
    muted: 0x3b82f6,
    chip: 0xdbeafe,
  },
  block: {
    fill: 0xf4f6f9,
    accent: 0xe2e8f0,
    border: 0x64748b,
    text: 0x0f172a,
    muted: 0x64748b,
    chip: 0xe2e8f0,
  },
  interface: {
    fill: 0xfffbf0,
    accent: 0xfef3c7,
    border: 0xf59e0b,
    text: 0x1c1300,
    muted: 0xb45309,
    chip: 0xfef3c7,
  },
  port: {
    fill: 0xfffbf0,
    accent: 0xfef3c7,
    border: 0xf59e0b,
    text: 0x1c1300,
    muted: 0xb45309,
    chip: 0xfef3c7,
  },
  use_case: {
    fill: 0xfaf7ff,
    accent: 0xede9fe,
    border: 0x7c3aed,
    text: 0x0f0820,
    muted: 0x7c3aed,
    chip: 0xede9fe,
  },
  actor: {
    fill: 0xfaf7ff,
    accent: 0xede9fe,
    border: 0x7c3aed,
    text: 0x0f0820,
    muted: 0x7c3aed,
    chip: 0xede9fe,
  },
  test_case: {
    fill: 0xf0fff5,
    accent: 0xdcfce7,
    border: 0x16a34a,
    text: 0x052e16,
    muted: 0x16a34a,
    chip: 0xdcfce7,
  },
  stakeholder: {
    fill: 0xf8f9fb,
    accent: 0xf1f5f9,
    border: 0x94a3b8,
    text: 0x0f172a,
    muted: 0x64748b,
    chip: 0xf1f5f9,
  },
  function: {
    fill: 0xf0faff,
    accent: 0xe0f2fe,
    border: 0x0284c7,
    text: 0x082032,
    muted: 0x0284c7,
    chip: 0xe0f2fe,
  },
  external: {
    fill: 0xfafafa,
    accent: 0xf0f0f0,
    border: 0xaaaaaa,
    text: 0x1a1a1a,
    muted: 0x888888,
    chip: 0xf0f0f0,
  },
  value_type: {
    fill: 0xfff7ed,
    accent: 0xffedd5,
    border: 0xea580c,
    text: 0x1c0a00,
    muted: 0xea580c,
    chip: 0xffedd5,
  },
  constraint_block: {
    fill: 0xf5f3ff,
    accent: 0xede9fe,
    border: 0x7c3aed,
    text: 0x150a2c,
    muted: 0x7c3aed,
    chip: 0xede9fe,
  },
  state: {
    fill: 0xf0fdf4,
    accent: 0xdcfce7,
    border: 0x16a34a,
    text: 0x052e16,
    muted: 0x16a34a,
    chip: 0xdcfce7,
  },
};

const SELECTED_COLOR = 0x3b82f6; // blue ring when selected
const CORNER_RADIUS = 8;
const HEADER_HEIGHT = 26;
const PADDING_X = 10;
const FONT_STEREO = 9; // «stereotype» label
const FONT_NAME = 13;
const FONT_ID = 9; // REQ-xxx chip
const FONT_SUBTEXT = 10;

// SysML-style stereotype strings
const STEREO_LABEL: Record<string, string> = {
  requirement: "«requirement»",
  block: "«block»",
  interface: "«interface»",
  port: "«port»",
  use_case: "«use case»",
  actor: "«actor»",
  test_case: "«test case»",
  stakeholder: "«stakeholder»",
  function: "«function»",
  external: "«external»",
  value_type: "«valueType»",
  constraint_block: "«constraintBlock»",
  state: "«state»",
};

export class NodeRenderer {
  private layer: PIXI.Container;
  private onNodeContextMenu?: (nodeId: string, x: number, y: number) => void;
  private onNodeDoubleClick?: (nodeId: string) => void;
  private sprites = new Map<string, PIXI.Container>();

  constructor(
    layer: PIXI.Container,
    onNodeContextMenu?: (nodeId: string, x: number, y: number) => void,
    onNodeDoubleClick?: (nodeId: string) => void,
  ) {
    this.layer = layer;
    this.onNodeContextMenu = onNodeContextMenu;
    this.onNodeDoubleClick = onNodeDoubleClick;
  }

  render(nodes: IRNode[], selection: SelectionManager) {
    const seen = new Set<string>();

    for (const node of nodes) {
      seen.add(node.id);
      const existing = this.sprites.get(node.id);
      if (existing) {
        this.updateNode(existing, node, selection);
      } else {
        const sprite = this.createNode(node, selection);
        this.layer.addChild(sprite);
        this.sprites.set(node.id, sprite);
      }
    }

    for (const [id, sprite] of this.sprites) {
      if (!seen.has(id)) {
        this.layer.removeChild(sprite);
        sprite.destroy({ children: true });
        this.sprites.delete(id);
      }
    }
  }

  private createNode(
    node: IRNode,
    selection: SelectionManager,
  ): PIXI.Container {
    const c = new PIXI.Container();
    c.x = node.x;
    c.y = node.y;
    c.eventMode = "static";
    c.cursor = "pointer";

    this.drawShape(c, node, selection.isNodeSelected(node.id));
    this.makeDraggable(c, node, selection);

    return c;
  }

  private updateNode(
    c: PIXI.Container,
    node: IRNode,
    selection: SelectionManager,
  ) {
    c.x = node.x;
    c.y = node.y;
    c.removeChildren();
    this.drawShape(c, node, selection.isNodeSelected(node.id));
  }

  private drawShape(c: PIXI.Container, node: IRNode, selected: boolean) {
    const palette = KIND_COLORS[node.kind] ?? KIND_COLORS.block;
    const w = node.width;
    const h = node.height;
    const style = node.style_overrides as Record<string, unknown> | undefined;
    const centerLabel = style?.center_label === true;

    c.alpha = node.has_suggestion ? 0.55 : 1;

    const g = new PIXI.Graphics();

    // ── Drop shadow (very subtle) ─────────────────────────────────────────────
    g.beginFill(0x000000, 0.07);
    g.drawRoundedRect(2, 3, w, h, CORNER_RADIUS);
    g.endFill();

    // ── Body ─────────────────────────────────────────────────────────────────
    g.beginFill(palette.fill);
    g.lineStyle(
      selected ? 2 : 1.5,
      selected ? SELECTED_COLOR : palette.border,
      selected ? 1 : 0.9,
    );
    g.drawRoundedRect(0, 0, w, h, CORNER_RADIUS);
    g.endFill();

    // ── Header tint strip (top portion, slightly deeper fill) ─────────────────
    // Clip to rounded top — draw as a rectangle then clip the overrun with the
    // same border rect drawn on top.  Simplest approach: draw header, then
    // re-draw the bottom edge of the border over it.
    g.lineStyle(0);
    g.beginFill(palette.accent, 0.9);
    // Only round the top corners by drawing a rect that extends below header
    g.drawRoundedRect(
      1,
      1,
      w - 2,
      HEADER_HEIGHT + CORNER_RADIUS,
      CORNER_RADIUS - 1,
    );
    g.endFill();

    // ── Clip the header rect bottom bleed: draw an opaque fill cover ──────────
    g.beginFill(palette.fill);
    g.drawRect(1, HEADER_HEIGHT, w - 2, CORNER_RADIUS + 1);
    g.endFill();

    // ── Thin divider line under header ────────────────────────────────────────
    g.lineStyle(1, palette.border, 0.25);
    g.moveTo(1, HEADER_HEIGHT);
    g.lineTo(w - 1, HEADER_HEIGHT);

    // ── Selection ring ───────────────────────────────────────────────────────
    if (selected) {
      const glow = new PIXI.Graphics();
      glow.lineStyle(2.5, SELECTED_COLOR, 0.35);
      glow.drawRoundedRect(-3, -3, w + 6, h + 6, CORNER_RADIUS + 3);
      c.addChild(glow);
    }

    c.addChild(g);

    // ── Stereotype label  «block» ─────────────────────────────────────────────
    if (!centerLabel) {
      const stereoStr = STEREO_LABEL[node.kind] ?? `«${node.kind}»`;
      const stereoText = new PIXI.Text(stereoStr, {
        fontFamily: "Inter, Segoe UI, sans-serif",
        fontSize: FONT_STEREO,
        fontStyle: "italic",
        fill: palette.muted,
        letterSpacing: 0.2,
      });
      stereoText.x = PADDING_X;
      stereoText.y = 5;
      c.addChild(stereoText);
    }

    // ── ID chip  REQ-001 ──────────────────────────────────────────────────────
    const reqId = getReqId(node);
    if (reqId && !centerLabel) {
      const chipBg = new PIXI.Graphics();
      const chipText = new PIXI.Text(reqId, {
        fontFamily: "Cascadia Code, Fira Mono, monospace",
        fontSize: FONT_ID,
        fontWeight: "600",
        fill: palette.muted,
      });
      chipText.x = PADDING_X + 4;
      chipText.y = 5;

      // Position chip at right side of header
      const chipW = chipText.width + 8;
      const chipH = 14;
      const chipX = w - chipW - PADDING_X;
      const chipY = 6;
      chipBg.beginFill(palette.chip, 0.9);
      chipBg.lineStyle(1, palette.border, 0.3);
      chipBg.drawRoundedRect(chipX, chipY, chipW, chipH, 3);
      chipBg.endFill();
      chipText.x = chipX + 4;
      chipText.y = chipY + 2;
      c.addChild(chipBg);
      c.addChild(chipText);
    }

    // ── Node name ─────────────────────────────────────────────────────────────
    const nameText = new PIXI.Text(node.name || "(unnamed)", {
      fontFamily: "Inter, Segoe UI, sans-serif",
      fontSize: centerLabel ? 16 : FONT_NAME,
      fontWeight: "600",
      fill: palette.text,
      wordWrap: true,
      wordWrapWidth: w - PADDING_X * 2,
      lineHeight: centerLabel ? 20 : 17,
    });

    if (centerLabel) {
      nameText.anchor.set(0.5);
      nameText.x = w / 2;
      nameText.y = h / 2;
    } else {
      nameText.x = PADDING_X;
      nameText.y = HEADER_HEIGHT + 7;
    }
    c.addChild(nameText);

    // ── Summary / description text ────────────────────────────────────────────
    if (!centerLabel) {
      const summary = summarizeNode(node);
      if (summary.length > 0) {
        const subText = new PIXI.Text(summary, {
          fontFamily: "Inter, Segoe UI, sans-serif",
          fontSize: FONT_SUBTEXT,
          fill: 0x6b7280, // neutral gray — readable on any light fill
          wordWrap: true,
          wordWrapWidth: w - PADDING_X * 2,
          lineHeight: 14,
        });
        subText.x = PADDING_X;
        subText.y = Math.min(h - 18, HEADER_HEIGHT + 24);
        c.addChild(subText);
      }
    }
  }

  private makeDraggable(
    c: PIXI.Container,
    node: IRNode,
    selection: SelectionManager,
  ) {
    let dragging = false;
    let startPointer = { x: 0, y: 0 };
    let startPos = { x: 0, y: 0 };
    let lastTapTime = 0;
    const DOUBLE_TAP_MS = 300;

    c.on("pointerdown", (e: PIXI.FederatedPointerEvent) => {
      if (e.button === 2) {
        e.stopPropagation();
        selection.selectNode(node.id, false);
        this.onNodeContextMenu?.(node.id, e.clientX, e.clientY);
        return;
      }
      if (e.button !== 0) return;
      e.stopPropagation();

      const now = Date.now();
      if (now - lastTapTime < DOUBLE_TAP_MS) {
        this.onNodeDoubleClick?.(node.id);
        lastTapTime = 0;
        return;
      }
      lastTapTime = now;

      dragging = true;
      startPointer = { x: e.globalX, y: e.globalY };
      startPos = { x: c.x, y: c.y };
      selection.selectNode(node.id, e.shiftKey);
      c.zIndex = 9999;
    });

    c.on("globalpointermove", (e: PIXI.FederatedPointerEvent) => {
      if (!dragging) return;
      const scale = c.parent?.scale.x ?? 1;
      c.x = startPos.x + (e.globalX - startPointer.x) / scale;
      c.y = startPos.y + (e.globalY - startPointer.y) / scale;
    });

    c.on("pointerup", () => {
      if (!dragging) return;
      dragging = false;
      c.zIndex = 0;
      selection.notifyNodeMoved(node.id, c.x, c.y);
    });

    c.on("pointerupoutside", () => {
      dragging = false;
      c.zIndex = 0;
    });
  }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function getReqId(node: IRNode): string | null {
  if (node.kind !== "requirement") return null;
  const data = node.data as Record<string, unknown> | undefined;
  const id = data?.req_id;
  return typeof id === "string" && id.length > 0 ? id : null;
}

function summarizeNode(node: IRNode): string {
  const data = (node.data ?? {}) as Record<string, unknown>;

  if (node.kind === "requirement") {
    const text = cleanSentence(data.text ?? node.description ?? "");
    return truncate(text, 90);
  }

  if (node.kind === "test_case") {
    const status = cleanSentence(data.status ?? "");
    const procedure = cleanSentence(data.procedure ?? "");
    const joined = [status ? `status: ${status}` : "", procedure]
      .filter(Boolean)
      .join(" · ");
    return truncate(joined, 90);
  }

  return truncate(cleanSentence(node.description ?? ""), 90);
}

function cleanSentence(value: unknown): string {
  if (typeof value !== "string") return "";
  return value.replace(/\s+/g, " ").trim();
}

function truncate(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  return `${text.slice(0, maxLen - 1).trim()}…`;
}

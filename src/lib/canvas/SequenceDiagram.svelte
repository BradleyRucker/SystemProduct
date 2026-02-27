<script lang="ts">
    /**
     * SequenceDiagram.svelte
     *
     * SVG-based sequence diagram renderer.
     *
     * Layout rules:
     *   • Each node in the IR becomes a lifeline (vertical dashed line + header box).
     *   • Lifelines are ordered by their x position in the diagram element data so
     *     the user can reorder them by dragging in the parent (x stored, y ignored).
     *   • Each edge whose both endpoints are lifelines in this diagram becomes a
     *     message arrow, ordered by the edge's index in the IR edge list.
     *   • "self" messages (source === target) draw a small loop on the right side.
     *   • Edge label becomes the message label.
     *   • Edge kind drives the arrow style:
     *       - "connects"    → solid filled arrowhead   (synchronous call)
     *       - "transition"  → dashed open arrowhead    (return / async reply)
     *       - anything else → open arrowhead           (async signal)
     *
     * Interaction:
     *   • Click a lifeline header → onNodeSelected
     *   • Click a message arrow   → onEdgeSelected
     *   • Drag lifeline header    → moves that lifeline (fires onNodeMoved with new x)
     *   • Right-click lifeline    → onNodeContextMenu
     *   • Right-click message     → onEdgeContextMenu
     *   • Delete/Backspace key    → onDeleteSelected
     */

    import type { DiagramIR, IRNode, IREdge } from "$lib/types";
    import { createEventDispatcher, onMount, onDestroy } from "svelte";

    export let ir: DiagramIR;
    export let onNodeSelected:    (id: string, additive: boolean) => void = () => {};
    export let onEdgeSelected:    (id: string, additive: boolean) => void = () => {};
    export let onEmptyClick:      () => void = () => {};
    export let onNodeMoved:       (id: string, x: number, y: number) => void = () => {};
    export let onNodeContextMenu: (id: string, x: number, y: number) => void = () => {};
    export let onEdgeContextMenu: (id: string, x: number, y: number) => void = () => {};
    export let onDeleteSelected:  () => void = () => {};

    // ── Layout constants ──────────────────────────────────────────────────────
    const HEADER_W   = 140;   // lifeline header box width
    const HEADER_H   = 50;    // lifeline header box height
    const LANE_W     = 180;   // horizontal space per lifeline
    const TOP_PAD    = 24;    // space above headers
    const MSG_START  = HEADER_H + TOP_PAD + 16; // y where first message sits
    const MSG_GAP    = 48;    // vertical gap between messages
    const SELF_W     = 48;    // width of self-message loop
    const SELF_H     = 32;    // height of self-message loop

    // ── State ─────────────────────────────────────────────────────────────────
    let selectedNodeId: string | null = null;
    let selectedEdgeId: string | null = null;

    // Drag state
    let dragging: { id: string; startX: number; origLX: number } | null = null;

    // ── Derived layout ────────────────────────────────────────────────────────
    interface Lifeline {
        node: IRNode;
        cx: number;   // center x of the lifeline on screen
        lx: number;   // logical x (from IR, used for ordering)
    }

    interface Message {
        edge: IREdge;
        fromIdx: number;
        toIdx: number;
        y: number;
        isSelf: boolean;
    }

    let lifelines: Lifeline[] = [];
    let messages: Message[] = [];
    let svgH = 400;
    let svgW = 600;

    $: {
        // Sort by x position to get left-to-right order
        const sorted = [...ir.nodes].sort((a, b) => a.x - b.x);
        lifelines = sorted.map((node, i) => ({
            node,
            cx: i * LANE_W + LANE_W / 2,
            lx: node.x,
        }));

        const idToIdx = new Map(lifelines.map((l, i) => [l.node.id, i]));

        messages = ir.edges
            .map((edge, msgIdx) => {
                const fi = idToIdx.get(edge.source_id);
                const ti = idToIdx.get(edge.target_id);
                if (fi === undefined || ti === undefined) return null;
                return {
                    edge,
                    fromIdx: fi,
                    toIdx: ti,
                    y: MSG_START + msgIdx * MSG_GAP,
                    isSelf: fi === ti,
                } satisfies Message;
            })
            .filter(Boolean) as Message[];

        const maxY = messages.length > 0
            ? messages[messages.length - 1].y + MSG_GAP * 2
            : MSG_START + 80;
        svgH = maxY + 40;
        svgW = Math.max(600, lifelines.length * LANE_W + LANE_W / 2);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────
    function arrowStyle(kind: string): { dashed: boolean; filled: boolean } {
        if (kind === "connects")   return { dashed: false, filled: true  };
        if (kind === "transition") return { dashed: true,  filled: false };
        return { dashed: false, filled: false };
    }

    function lifelineColor(kind: string): string {
        const map: Record<string, string> = {
            block:      "#3b82f6",
            actor:      "#8b5cf6",
            external:   "#64748b",
            interface:  "#0ea5e9",
        };
        return map[kind] ?? "#6366f1";
    }

    function truncate(s: string, max = 18): string {
        return s.length > max ? s.slice(0, max - 1) + "…" : s;
    }

    // ── Drag handling ─────────────────────────────────────────────────────────
    function onHeaderMouseDown(e: MouseEvent, ll: Lifeline) {
        if (e.button !== 0) return;
        e.preventDefault();
        dragging = { id: ll.node.id, startX: e.clientX, origLX: ll.lx };
    }

    function onMouseMove(e: MouseEvent) {
        if (!dragging) return;
        const dx = e.clientX - dragging.startX;
        const newLX = dragging.origLX + dx;
        // Re-order immediately for live feedback
        const idx = lifelines.findIndex(l => l.node.id === dragging!.id);
        if (idx >= 0) {
            lifelines[idx] = { ...lifelines[idx], lx: newLX };
            lifelines = [...lifelines].sort((a, b) => a.lx - b.lx)
                .map((l, i) => ({ ...l, cx: i * LANE_W + LANE_W / 2 }));
        }
    }

    function onMouseUp(e: MouseEvent) {
        if (!dragging) return;
        const ll = lifelines.find(l => l.node.id === dragging!.id);
        if (ll) onNodeMoved(ll.node.id, ll.lx, ll.node.y);
        dragging = null;
    }

    // ── Selection ─────────────────────────────────────────────────────────────
    function selectNode(id: string, e: MouseEvent) {
        e.stopPropagation();
        selectedNodeId = id;
        selectedEdgeId = null;
        onNodeSelected(id, e.shiftKey || e.metaKey);
    }

    function selectEdge(id: string, e: MouseEvent) {
        e.stopPropagation();
        selectedEdgeId = id;
        selectedNodeId = null;
        onEdgeSelected(id, e.shiftKey || e.metaKey);
    }

    function clickBackground() {
        selectedNodeId = null;
        selectedEdgeId = null;
        onEmptyClick();
    }

    // ── Keyboard ──────────────────────────────────────────────────────────────
    function onKeyDown(e: KeyboardEvent) {
        const tag = (e.target as HTMLElement).tagName;
        if (tag === "INPUT" || tag === "TEXTAREA") return;
        if (e.key === "Delete" || e.key === "Backspace") {
            e.preventDefault();
            onDeleteSelected();
        }
    }

    onMount(() => window.addEventListener("keydown", onKeyDown));
    onDestroy(() => window.removeEventListener("keydown", onKeyDown));
</script>

<!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
<!-- svelte-ignore a11y-click-events-have-key-events -->
<div
    class="seq-root"
    role="application"
    aria-label="Sequence diagram"
    on:mousemove={onMouseMove}
    on:mouseup={onMouseUp}
    on:mouseleave={onMouseUp}
>
    {#if lifelines.length === 0}
        <div class="seq-empty">
            <div class="seq-empty-title">Empty sequence diagram</div>
            <div class="seq-empty-body">
                Add <strong>Block</strong> or <strong>Actor</strong> nodes from the panel on the left,
                then draw <strong>connects</strong> edges between them to create messages.
            </div>
        </div>
    {:else}
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <svg
            class="seq-svg"
            width={svgW}
            height={svgH}
            on:click={clickBackground}
        >
            <defs>
                <!-- Filled arrowhead (sync call) -->
                <marker id="arr-filled" markerWidth="8" markerHeight="8"
                    refX="7" refY="3" orient="auto">
                    <path d="M0,0 L0,6 L8,3 z" fill="var(--seq-msg-color, #94a3b8)" />
                </marker>
                <!-- Open arrowhead (async) -->
                <marker id="arr-open" markerWidth="10" markerHeight="8"
                    refX="9" refY="3" orient="auto">
                    <polyline points="0,0 9,3 0,6" fill="none"
                        stroke="var(--seq-msg-color, #94a3b8)" stroke-width="1.5" />
                </marker>
                <!-- Open arrowhead for dashed return -->
                <marker id="arr-return" markerWidth="10" markerHeight="8"
                    refX="9" refY="3" orient="auto">
                    <polyline points="0,0 9,3 0,6" fill="none"
                        stroke="#64748b" stroke-width="1.5" />
                </marker>
            </defs>

            <!-- ── Lifeline dashed lines ── -->
            {#each lifelines as ll}
                <line
                    x1={ll.cx} y1={HEADER_H + TOP_PAD}
                    x2={ll.cx} y2={svgH - 24}
                    stroke="#334155"
                    stroke-width="1.5"
                    stroke-dasharray="6 4"
                />
            {/each}

            <!-- ── Messages ── -->
            {#each messages as msg}
                {@const style = arrowStyle(msg.edge.kind)}
                {@const fromCx = lifelines[msg.fromIdx]?.cx ?? 0}
                {@const toCx   = lifelines[msg.toIdx]?.cx ?? 0}
                {@const label  = msg.edge.label || msg.edge.kind}
                {@const isSelected = selectedEdgeId === msg.edge.id}

                {#if msg.isSelf}
                    <!-- Self-message: right-side loop -->
                    <!-- svelte-ignore a11y-no-static-element-interactions -->
                    <g
                        class="seq-msg"
                        class:selected={isSelected}
                        on:click={(e) => selectEdge(msg.edge.id, e)}
                        on:contextmenu|preventDefault={(e) =>
                            onEdgeContextMenu(msg.edge.id, e.clientX, e.clientY)}
                        role="button"
                        tabindex="-1"
                    >
                        <path
                            d={`M${fromCx},${msg.y} L${fromCx + SELF_W},${msg.y} L${fromCx + SELF_W},${msg.y + SELF_H} L${fromCx},${msg.y + SELF_H}`}
                            fill="none"
                            stroke={isSelected ? "#6366f1" : "#94a3b8"}
                            stroke-width={isSelected ? 2 : 1.5}
                            stroke-dasharray={style.dashed ? "5 3" : "none"}
                            marker-end={style.filled ? "url(#arr-filled)" : "url(#arr-open)"}
                        />
                        <text
                            x={fromCx + SELF_W + 6}
                            y={msg.y + SELF_H / 2}
                            class="msg-label"
                            dominant-baseline="middle"
                            fill={isSelected ? "#a5b4fc" : "#94a3b8"}
                        >{truncate(label, 20)}</text>
                    </g>
                {:else}
                    <!-- Regular message -->
                    {@const goRight = toCx > fromCx}
                    {@const arrowMarker = style.dashed
                        ? "url(#arr-return)"
                        : style.filled
                            ? "url(#arr-filled)"
                            : "url(#arr-open)"}
                    <!-- svelte-ignore a11y-no-static-element-interactions -->
                    <g
                        class="seq-msg"
                        class:selected={isSelected}
                        on:click={(e) => selectEdge(msg.edge.id, e)}
                        on:contextmenu|preventDefault={(e) =>
                            onEdgeContextMenu(msg.edge.id, e.clientX, e.clientY)}
                        role="button"
                        tabindex="-1"
                    >
                        <!-- Hit-area line (invisible, wider) -->
                        <line
                            x1={fromCx} y1={msg.y} x2={toCx} y2={msg.y}
                            stroke="transparent" stroke-width="12"
                        />
                        <!-- Visible arrow line -->
                        <line
                            x1={fromCx} y1={msg.y} x2={toCx} y2={msg.y}
                            stroke={isSelected ? "#6366f1" : style.dashed ? "#64748b" : "#94a3b8"}
                            stroke-width={isSelected ? 2 : 1.5}
                            stroke-dasharray={style.dashed ? "5 3" : "none"}
                            marker-end={arrowMarker}
                        />
                        <!-- Label above the arrow -->
                        <text
                            x={(fromCx + toCx) / 2}
                            y={msg.y - 7}
                            class="msg-label"
                            text-anchor="middle"
                            fill={isSelected ? "#a5b4fc" : "#94a3b8"}
                        >{truncate(label)}</text>
                    </g>
                {/if}
            {/each}

            <!-- ── Lifeline headers (on top so dragging works) ── -->
            {#each lifelines as ll}
                {@const isSelected = selectedNodeId === ll.node.id}
                {@const color = lifelineColor(ll.node.kind)}
                {@const hx = ll.cx - HEADER_W / 2}
                {@const hy = TOP_PAD}

                <!-- svelte-ignore a11y-no-static-element-interactions -->
                <g
                    class="lifeline-header"
                    class:selected={isSelected}
                    style="cursor: grab"
                    on:mousedown={(e) => onHeaderMouseDown(e, ll)}
                    on:click={(e) => selectNode(ll.node.id, e)}
                    on:contextmenu|preventDefault={(e) =>
                        onNodeContextMenu(ll.node.id, e.clientX, e.clientY)}
                    role="button"
                    tabindex="0"
                >
                    <!-- Drop shadow -->
                    <rect
                        x={hx + 2} y={hy + 2}
                        width={HEADER_W} height={HEADER_H}
                        rx="6" fill="rgba(0,0,0,0.35)"
                    />
                    <!-- Main box -->
                    <rect
                        x={hx} y={hy}
                        width={HEADER_W} height={HEADER_H}
                        rx="6"
                        fill={isSelected ? "#1e1b4b" : "#0f172a"}
                        stroke={isSelected ? "#6366f1" : color}
                        stroke-width={isSelected ? 2 : 1.5}
                    />
                    <!-- Color accent bar at top -->
                    <rect
                        x={hx + 1} y={hy + 1}
                        width={HEADER_W - 2} height="4"
                        rx="5"
                        fill={color}
                        opacity="0.7"
                    />
                    <!-- Stereotype label -->
                    <text
                        x={ll.cx} y={hy + 16}
                        class="stereo-label"
                        text-anchor="middle"
                        fill={color}
                    >«{ll.node.kind.replace("_", " ")}»</text>
                    <!-- Name -->
                    <text
                        x={ll.cx} y={hy + 34}
                        class="lifeline-name"
                        text-anchor="middle"
                        fill={isSelected ? "#e0e7ff" : "#e2e8f0"}
                    >{truncate(ll.node.name, 17)}</text>
                </g>
            {/each}
        </svg>
    {/if}
</div>

<style>
    .seq-root {
        width: 100%;
        height: 100%;
        overflow: auto;
        background: #0d1117;
        position: relative;
    }

    .seq-svg {
        display: block;
        min-width: 100%;
    }

    .seq-empty {
        position: absolute;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        text-align: center;
        max-width: 380px;
        padding: 32px;
        background: var(--surface-raised, #161b22);
        border: 1px solid var(--surface-border, #30363d);
        border-radius: 12px;
        color: var(--text-muted, #8b949e);
    }

    .seq-empty-title {
        font-size: 15px;
        font-weight: 600;
        color: var(--text-secondary, #c9d1d9);
        margin-bottom: 8px;
    }

    .seq-empty-body {
        font-size: 13px;
        line-height: 1.6;
    }

    .seq-empty-body strong {
        color: var(--text-secondary, #c9d1d9);
    }

    .msg-label {
        font-size: 11px;
        font-family: var(--font-mono, monospace);
        pointer-events: none;
    }

    .stereo-label {
        font-size: 9px;
        font-family: var(--font-sans, sans-serif);
        font-style: italic;
        pointer-events: none;
    }

    .lifeline-name {
        font-size: 12px;
        font-weight: 600;
        font-family: var(--font-sans, sans-serif);
        pointer-events: none;
    }

    .seq-msg {
        cursor: pointer;
    }

    .lifeline-header {
        transition: filter 0.1s;
    }

    .lifeline-header:hover rect:nth-child(2) {
        filter: brightness(1.15);
    }

    .lifeline-header.selected rect:nth-child(2) {
        filter: drop-shadow(0 0 6px #6366f1aa);
    }
</style>

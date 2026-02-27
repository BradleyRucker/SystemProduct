<script lang="ts">
    /**
     * Parametric Diagram renderer (SVG-based).
     *
     * Shows constraint blocks with their parameters and binding connectors
     * linking constraint parameters to value properties on blocks/value-types.
     *
     * Layout: constraint blocks in a centred column; bound blocks orbit them
     * in a grid around the edges.
     */
    import { createEventDispatcher } from "svelte";
    import type { DiagramIR, IRNode, IREdge, ConstraintBlockData } from "$lib/types";

    export let ir: DiagramIR;
    export let onNodeSelected: (id: string) => void = () => {};
    export let onEdgeSelected: (id: string) => void = () => {};
    export let onEmptyClick: () => void = () => {};
    export let onNodeMoved: (id: string, x: number, y: number) => void = () => {};
    export let onNodeContextMenu: (id: string, x: number, y: number) => void = () => {};
    export let onEdgeContextMenu: (id: string, x: number, y: number) => void = () => {};
    // accepted but unused in this renderer (no keyboard-delete in SVG mode)
    export let onDeleteSelected: () => void = () => {};

    // ── Layout constants ───────────────────────────────────────────────────────
    const CB_W   = 200;  // constraint block width
    const CB_H   = 110;  // constraint block height (base; grows with params)
    const BLK_W  = 160;  // value block width
    const BLK_H  = 72;   // value block height
    const PAD_X  = 60;   // horizontal gap between constraint block and value blocks
    const PAD_Y  = 36;   // vertical gap between rows
    const PORT_R = 6;    // port circle radius

    // ── Derived data ──────────────────────────────────────────────────────────

    // Constraint block nodes
    $: cbNodes = ir.nodes.filter(n => n.kind === "constraint_block");

    // All other nodes that appear in binding_connector edges
    $: boundNodeIds = new Set<string>(
        ir.edges
            .filter(e => e.kind === "binding_connector")
            .flatMap(e => [e.source_id, e.target_id])
            .filter(id => !cbNodes.some(cb => cb.id === id))
    );
    $: boundNodes = ir.nodes.filter(n => boundNodeIds.has(n.id));

    // Nodes that are neither CBs nor bound — show as plain "unconnected" list
    $: otherNodes = ir.nodes.filter(
        n => !cbNodes.some(cb => cb.id === n.id) && !boundNodeIds.has(n.id)
    );

    // ── Simple auto-layout ────────────────────────────────────────────────────
    // Use node.x/y if set (non-zero), otherwise fall back to computed positions.

    function cbParams(node: IRNode): string[] {
        const d = node.data as unknown as ConstraintBlockData | null;
        return d?.parameters ?? [];
    }

    function cbExpression(node: IRNode): string {
        const d = node.data as unknown as ConstraintBlockData | null;
        return d?.expression ?? "";
    }

    function cbHeight(node: IRNode): number {
        const params = cbParams(node);
        return Math.max(CB_H, 52 + params.length * 22);
    }

    interface Positioned {
        node: IRNode;
        x: number;
        y: number;
        w: number;
        h: number;
    }

    $: positioned = ((): { cbs: Positioned[]; bounds: Positioned[]; others: Positioned[] } => {
        // Place constraint blocks in a column on the left third
        const cbs: Positioned[] = [];
        let cbY = PAD_Y;
        for (const node of cbNodes) {
            const h = cbHeight(node);
            const hasPos = node.x !== 0 || node.y !== 0;
            cbs.push({
                node,
                x: hasPos ? node.x : PAD_X,
                y: hasPos ? node.y : cbY,
                w: CB_W,
                h,
            });
            cbY += h + PAD_Y;
        }

        // Place bound nodes in a column to the right
        const bounds: Positioned[] = [];
        let bY = PAD_Y;
        for (const node of boundNodes) {
            const hasPos = node.x !== 0 || node.y !== 0;
            bounds.push({
                node,
                x: hasPos ? node.x : PAD_X + CB_W + PAD_X * 2,
                y: hasPos ? node.y : bY,
                w: BLK_W,
                h: BLK_H,
            });
            bY += BLK_H + PAD_Y;
        }

        // Place other nodes further right
        const others: Positioned[] = [];
        let oY = PAD_Y;
        const oX = PAD_X + CB_W + PAD_X * 2 + BLK_W + PAD_X;
        for (const node of otherNodes) {
            const hasPos = node.x !== 0 || node.y !== 0;
            others.push({
                node,
                x: hasPos ? node.x : oX,
                y: hasPos ? node.y : oY,
                w: BLK_W,
                h: BLK_H,
            });
            oY += BLK_H + PAD_Y;
        }

        return { cbs, bounds, others };
    })();

    let allPositioned: Positioned[] = [];
    $: allPositioned = [...positioned.cbs, ...positioned.bounds, ...positioned.others];

    function posOf(id: string): { x: number; y: number; w: number; h: number } | null {
        return allPositioned.find(p => p.node.id === id) ?? null;
    }

    // For a binding_connector edge, find the port position on the constraint block
    // that matches the parameter name stored in edge.label (if any).
    function portPos(
        cbPos: { x: number; y: number; w: number; h: number },
        params: string[],
        paramName: string,
        side: "left" | "right",
    ): { x: number; y: number } {
        const idx = params.indexOf(paramName);
        const row = idx >= 0 ? idx : 0;
        const portY = cbPos.y + 52 + row * 22 + 11;
        return {
            x: side === "right" ? cbPos.x + cbPos.w : cbPos.x,
            y: portY,
        };
    }

    // ── Edge routing ──────────────────────────────────────────────────────────
    interface RoutedEdge {
        edge: IREdge;
        x1: number; y1: number;
        x2: number; y2: number;
        mx: number; my: number; // midpoint for label
    }

    $: routedEdges = ((): RoutedEdge[] => {
        return ir.edges
            .filter(e => e.kind === "binding_connector")
            .map(e => {
                const srcPos = posOf(e.source_id);
                const tgtPos = posOf(e.target_id);
                if (!srcPos || !tgtPos) return null;

                // If source is a CB, use the parameter port on the right side
                const srcIsCb = cbNodes.some(cb => cb.id === e.source_id);
                const tgtIsCb = cbNodes.some(cb => cb.id === e.target_id);

                let x1: number, y1: number, x2: number, y2: number;

                if (srcIsCb) {
                    const params = cbParams(allPositioned.find(p => p.node.id === e.source_id)!.node);
                    const pp = portPos(srcPos, params, e.label ?? "", "right");
                    x1 = pp.x; y1 = pp.y;
                } else {
                    x1 = srcPos.x + srcPos.w;
                    y1 = srcPos.y + srcPos.h / 2;
                }

                if (tgtIsCb) {
                    const params = cbParams(allPositioned.find(p => p.node.id === e.target_id)!.node);
                    const pp = portPos(tgtPos, params, e.label ?? "", "left");
                    x2 = pp.x; y2 = pp.y;
                } else {
                    x2 = tgtPos.x;
                    y2 = tgtPos.y + tgtPos.h / 2;
                }

                return {
                    edge: e,
                    x1, y1, x2, y2,
                    mx: (x1 + x2) / 2,
                    my: (y1 + y2) / 2,
                };
            })
            .filter((r): r is RoutedEdge => r !== null);
    })();

    // ── Non-binding edges (composes, satisfies, etc.) ─────────────────────────
    $: otherEdges = ir.edges.filter(e => e.kind !== "binding_connector");

    // ── SVG viewport ─────────────────────────────────────────────────────────
    $: svgWidth  = Math.max(800, ...allPositioned.map(p => p.x + p.w + PAD_X));
    $: svgHeight = Math.max(500, ...allPositioned.map(p => p.y + p.h + PAD_Y));

    // ── Colours ───────────────────────────────────────────────────────────────
    const COLOR_CB    = "#4c1d95"; // constraint block – deep violet
    const COLOR_BLOCK = "#1e3a5f"; // block/value – dark blue
    const COLOR_OTHER = "#1e3a2f"; // other nodes – dark green
    const EDGE_COLOR  = "#ea580c"; // binding connector – orange

    function nodeColor(kind: string): string {
        if (kind === "constraint_block") return COLOR_CB;
        if (kind === "block" || kind === "value_type") return COLOR_BLOCK;
        return COLOR_OTHER;
    }

    // ── Interaction ───────────────────────────────────────────────────────────
    function handleNodeClick(id: string, e: MouseEvent) {
        e.stopPropagation();
        onNodeSelected(id);
    }
    function handleNodeCtx(id: string, e: MouseEvent) {
        e.preventDefault();
        e.stopPropagation();
        onNodeSelected(id);
        onNodeContextMenu(id, e.clientX, e.clientY);
    }
    function handleEdgeClick(id: string, e: MouseEvent) {
        e.stopPropagation();
        onEdgeSelected(id);
    }
    function handleEdgeCtx(id: string, e: MouseEvent) {
        e.preventDefault();
        e.stopPropagation();
        onEdgeSelected(id);
        onEdgeContextMenu(id, e.clientX, e.clientY);
    }

    // ── Drag to move nodes ────────────────────────────────────────────────────
    let dragState: { id: string; startClientX: number; startClientY: number; startX: number; startY: number } | null = null;

    function onNodePointerDown(id: string, e: PointerEvent) {
        if (e.button !== 0) return;
        e.stopPropagation();
        const p = allPositioned.find(pos => pos.node.id === id);
        if (!p) return;
        dragState = { id, startClientX: e.clientX, startClientY: e.clientY, startX: p.x, startY: p.y };
        (e.currentTarget as Element).setPointerCapture(e.pointerId);
        onNodeSelected(id);
    }

    function onSvgPointerMove(e: PointerEvent) {
        if (!dragState) return;
        const dx = e.clientX - dragState.startClientX;
        const dy = e.clientY - dragState.startClientY;
        const newX = Math.max(0, dragState.startX + dx);
        const newY = Math.max(0, dragState.startY + dy);
        // Optimistically update the positioned list so the SVG redraws immediately
        const idx = allPositioned.findIndex(p => p.node.id === dragState!.id);
        if (idx >= 0) {
            allPositioned[idx] = { ...allPositioned[idx], x: newX, y: newY };
            allPositioned = allPositioned; // trigger reactivity
        }
    }

    function onSvgPointerUp(e: PointerEvent) {
        if (!dragState) return;
        const dx = e.clientX - dragState.startClientX;
        const dy = e.clientY - dragState.startClientY;
        // Only persist if actually moved more than 3px (not a click)
        if (Math.abs(dx) > 3 || Math.abs(dy) > 3) {
            const newX = Math.max(0, dragState.startX + dx);
            const newY = Math.max(0, dragState.startY + dy);
            onNodeMoved(dragState.id, newX, newY);
        }
        dragState = null;
    }
</script>

<!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
<div
    class="para-wrap"
    on:click={onEmptyClick}
    role="presentation"
>
    {#if ir.nodes.length === 0}
        <div class="empty-guide">
            <p class="empty-title">Parametric Diagram</p>
            <p class="empty-sub">
                Add <strong>ConstraintBlock</strong> and <strong>Block / ValueType</strong> nodes,
                then connect their parameters with <strong>binding_connector</strong> edges.
            </p>
        </div>
    {:else}
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <svg
            width={svgWidth}
            height={svgHeight}
            class="para-svg"
            xmlns="http://www.w3.org/2000/svg"
            on:pointermove={onSvgPointerMove}
            on:pointerup={onSvgPointerUp}
            on:pointerleave={onSvgPointerUp}
        >
            <defs>
                <!-- Dashed marker for binding connectors -->
                <marker id="bc-arrow" markerWidth="8" markerHeight="8"
                        refX="6" refY="3" orient="auto">
                    <path d="M0,0 L0,6 L8,3 z" fill={EDGE_COLOR} opacity="0.85" />
                </marker>
            </defs>

            <!-- ── Binding connector edges ── -->
            {#each routedEdges as r (r.edge.id)}
                <!-- Hit area -->
                <!-- svelte-ignore a11y-click-events-have-key-events -->
                <line
                    x1={r.x1} y1={r.y1} x2={r.x2} y2={r.y2}
                    stroke="transparent" stroke-width="12"
                    style="cursor:pointer"
                    on:click|stopPropagation={(e) => handleEdgeClick(r.edge.id, e)}
                    on:contextmenu={(e) => handleEdgeCtx(r.edge.id, e)}
                />
                <!-- Visible dashed line -->
                <line
                    x1={r.x1} y1={r.y1} x2={r.x2} y2={r.y2}
                    stroke={EDGE_COLOR}
                    stroke-width="1.5"
                    stroke-dasharray="6 4"
                    stroke-opacity="0.85"
                    marker-end="url(#bc-arrow)"
                    style="pointer-events:none"
                />
                {#if r.edge.label}
                    <text
                        x={r.mx} y={r.my - 5}
                        text-anchor="middle"
                        fill="#cbd5e1"
                        font-size="10"
                        font-family="monospace"
                        style="pointer-events:none"
                    >{r.edge.label}</text>
                {/if}
            {/each}

            <!-- ── Other edges (composes, satisfies, etc.) ── -->
            {#each otherEdges as edge (edge.id)}
                {@const sp = posOf(edge.source_id)}
                {@const tp = posOf(edge.target_id)}
                {#if sp && tp}
                    {@const x1 = sp.x + sp.w / 2}
                    {@const y1 = sp.y + sp.h}
                    {@const x2 = tp.x + tp.w / 2}
                    {@const y2 = tp.y}
                    <!-- svelte-ignore a11y-click-events-have-key-events -->
                    <line
                        {x1} {y1} {x2} {y2}
                        stroke="#64748b"
                        stroke-width="1.5"
                        stroke-dasharray="4 3"
                        stroke-opacity="0.6"
                        style="cursor:pointer"
                        on:click|stopPropagation={(e) => handleEdgeClick(edge.id, e)}
                        on:contextmenu={(e) => handleEdgeCtx(edge.id, e)}
                    />
                {/if}
            {/each}

            <!-- ── Constraint Block nodes ── -->
            {#each positioned.cbs as p (p.node.id)}
                {@const params = cbParams(p.node)}
                {@const expr   = cbExpression(p.node)}
                <!-- svelte-ignore a11y-click-events-have-key-events -->
                <g
                    style="cursor:grab"
                    on:click={(e) => handleNodeClick(p.node.id, e)}
                    on:contextmenu={(e) => handleNodeCtx(p.node.id, e)}
                    on:pointerdown={(e) => onNodePointerDown(p.node.id, e)}
                >
                    <!-- Outer rounded rect -->
                    <rect
                        x={p.x} y={p.y}
                        width={p.w} height={p.h}
                        rx="6" ry="6"
                        fill="#2e1065"
                        stroke="#7c3aed"
                        stroke-width="1.5"
                    />
                    <!-- Stereotype banner -->
                    <rect
                        x={p.x} y={p.y}
                        width={p.w} height="20"
                        rx="6" ry="6"
                        fill="#4c1d95"
                    />
                    <rect
                        x={p.x} y={p.y + 14}
                        width={p.w} height="6"
                        fill="#4c1d95"
                    />
                    <text
                        x={p.x + p.w / 2} y={p.y + 14}
                        text-anchor="middle"
                        fill="#c4b5fd"
                        font-size="9"
                        font-family="sans-serif"
                        font-style="italic"
                    >«constraintBlock»</text>
                    <!-- Name -->
                    <text
                        x={p.x + p.w / 2} y={p.y + 34}
                        text-anchor="middle"
                        fill="#e2e8f0"
                        font-size="12"
                        font-weight="600"
                        font-family="sans-serif"
                    >{p.node.name}</text>
                    <!-- Constraint expression -->
                    {#if expr}
                        <text
                            x={p.x + p.w / 2} y={p.y + 50}
                            text-anchor="middle"
                            fill="#a78bfa"
                            font-size="10"
                            font-family="monospace"
                        >{expr.length > 28 ? expr.slice(0, 27) + "…" : expr}</text>
                    {/if}
                    <!-- Parameter ports -->
                    {#each params as param, i}
                        {@const py = p.y + 52 + i * 22}
                        <!-- Right-side port circle -->
                        <circle
                            cx={p.x + p.w}
                            cy={py + 11}
                            r={PORT_R}
                            fill="#1e1b4b"
                            stroke="#7c3aed"
                            stroke-width="1.5"
                        />
                        <!-- Parameter label -->
                        <text
                            x={p.x + p.w - PORT_R - 6}
                            y={py + 15}
                            text-anchor="end"
                            fill="#c4b5fd"
                            font-size="10"
                            font-family="monospace"
                        >{param}</text>
                    {/each}
                </g>
            {/each}

            <!-- ── Bound value/block nodes ── -->
            {#each positioned.bounds as p (p.node.id)}
                {@const col = nodeColor(p.node.kind)}
                <!-- svelte-ignore a11y-click-events-have-key-events -->
                <g
                    style="cursor:grab"
                    on:click={(e) => handleNodeClick(p.node.id, e)}
                    on:contextmenu={(e) => handleNodeCtx(p.node.id, e)}
                    on:pointerdown={(e) => onNodePointerDown(p.node.id, e)}
                >
                    <rect
                        x={p.x} y={p.y}
                        width={p.w} height={p.h}
                        rx="4" ry="4"
                        fill={col + "22"}
                        stroke={col.replace("#", "#") + "99"}
                        stroke-width="1.5"
                    />
                    <!-- Left-side connection nub -->
                    <circle
                        cx={p.x}
                        cy={p.y + p.h / 2}
                        r={PORT_R}
                        fill="#0f172a"
                        stroke={EDGE_COLOR}
                        stroke-width="1.5"
                    />
                    <!-- Kind badge -->
                    <text
                        x={p.x + p.w / 2} y={p.y + 16}
                        text-anchor="middle"
                        fill="#94a3b8"
                        font-size="9"
                        font-style="italic"
                        font-family="sans-serif"
                    >«{p.node.kind.replace("_", " ")}»</text>
                    <!-- Name -->
                    <text
                        x={p.x + p.w / 2} y={p.y + 34}
                        text-anchor="middle"
                        fill="#e2e8f0"
                        font-size="12"
                        font-weight="600"
                        font-family="sans-serif"
                    >{p.node.name.length > 18 ? p.node.name.slice(0, 17) + "…" : p.node.name}</text>
                    <!-- Value property hint (description as value) -->
                    {#if p.node.description}
                        <text
                            x={p.x + p.w / 2} y={p.y + 52}
                            text-anchor="middle"
                            fill="#64748b"
                            font-size="10"
                            font-family="monospace"
                        >{p.node.description.length > 22 ? p.node.description.slice(0, 21) + "…" : p.node.description}</text>
                    {/if}
                </g>
            {/each}

            <!-- ── Other unconnected nodes ── -->
            {#each positioned.others as p (p.node.id)}
                <!-- svelte-ignore a11y-click-events-have-key-events -->
                <g
                    style="cursor:grab;opacity:0.65"
                    on:click={(e) => handleNodeClick(p.node.id, e)}
                    on:contextmenu={(e) => handleNodeCtx(p.node.id, e)}
                    on:pointerdown={(e) => onNodePointerDown(p.node.id, e)}
                >
                    <rect
                        x={p.x} y={p.y}
                        width={p.w} height={p.h}
                        rx="4" ry="4"
                        fill="#1e293b"
                        stroke="#334155"
                        stroke-width="1"
                        stroke-dasharray="4 2"
                    />
                    <text
                        x={p.x + p.w / 2} y={p.y + 16}
                        text-anchor="middle"
                        fill="#64748b"
                        font-size="9"
                        font-style="italic"
                        font-family="sans-serif"
                    >«{p.node.kind.replace("_", " ")}»</text>
                    <text
                        x={p.x + p.w / 2} y={p.y + 34}
                        text-anchor="middle"
                        fill="#94a3b8"
                        font-size="11"
                        font-family="sans-serif"
                    >{p.node.name.length > 18 ? p.node.name.slice(0, 17) + "…" : p.node.name}</text>
                </g>
            {/each}
        </svg>
    {/if}
</div>

<style>
    .para-wrap {
        flex: 1;
        overflow: auto;
        background: #0a0e1a;
        display: flex;
        align-items: flex-start;
        justify-content: flex-start;
        min-height: 0;
        padding: 16px;
    }

    .para-svg {
        display: block;
        flex-shrink: 0;
    }

    .empty-guide {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 12px;
        color: var(--text-muted);
        text-align: center;
        padding: 80px 40px;
    }
    .empty-title {
        font-size: var(--text-lg);
        font-weight: var(--weight-semibold);
        color: var(--text-secondary);
        margin: 0;
    }
    .empty-sub {
        font-size: var(--text-sm);
        color: var(--text-muted);
        margin: 0;
        max-width: 380px;
        line-height: 1.6;
    }
</style>

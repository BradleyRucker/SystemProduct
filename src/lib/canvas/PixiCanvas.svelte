<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import * as PIXI from "pixi.js";
    import type { DiagramIR } from "$lib/types";
    import { viewport, zoom, clearSelection } from "$lib/store/canvas";
    import { NodeRenderer } from "../../NodeRenderer";
    import { EdgeRouter } from "./EdgeRouter";
    import { SelectionManager } from "./SelectionManager";

    export let ir: DiagramIR;
    export let onNodeMoved: (
        nodeId: string,
        x: number,
        y: number,
    ) => void = () => {};
    export let onNodeSelected: (
        nodeId: string,
        additive: boolean,
    ) => void = () => {};
    export let onEdgeSelected: (
        edgeId: string,
        additive: boolean,
    ) => void = () => {};
    export let onEmptyClick: () => void = () => {};
    /** Called when user clicks source then target to create an edge */
    export let onConnectNodes: (
        sourceId: string,
        targetId: string,
    ) => void = () => {};
    /** Called on right-click of a node with canvas coordinates */
    export let onNodeContextMenu: (
        nodeId: string,
        x: number,
        y: number,
    ) => void = () => {};
    /** Called on double-click of a node */
    export let onNodeDoubleClick: (nodeId: string) => void = () => {};
    /** Called on right-click of an edge with canvas coordinates */
    export let onEdgeContextMenu: (
        edgeId: string,
        x: number,
        y: number,
    ) => void = () => {};
    /** Called when Delete/Backspace is pressed and focus is not in an input */
    export let onDeleteSelected: () => void = () => {};
    /** Called when Ctrl+Shift+C is pressed to center the view */
    export let onCenterRequested: () => void = () => {};

    let container: HTMLDivElement;
    let app: PIXI.Application;
    let worldContainer: PIXI.Container;
    let edgeLayer: PIXI.Container;
    let nodeLayer: PIXI.Container;
    let ghostLayer: PIXI.Container;
    let connectLine: PIXI.Graphics; // live wire while connecting

    let nodeRenderer: NodeRenderer;
    let edgeRouter: EdgeRouter;
    let selectionManager: SelectionManager;

    let isPanning = false;
    let panStart = { x: 0, y: 0 };
    let vpSnapshot = { x: 0, y: 0, scale: 1 };
    let spaceHeld = false;

    // Connect mode
    // Exposed so the parent can toggle it (e.g. via a toolbar button)
    export let connectMode = false;
    let connectSource: string | null = null;

    let themeObserver: MutationObserver | null = null;

    onMount(() => {
        app = new PIXI.Application({
            resizeTo: container,
            backgroundColor: 0x0d1017,
            antialias: true,
            resolution: window.devicePixelRatio || 1,
            autoDensity: true,
        });

        const canvasEl = app.view as HTMLCanvasElement;
        container.appendChild(canvasEl);

        worldContainer = new PIXI.Container();
        edgeLayer = new PIXI.Container();
        nodeLayer = new PIXI.Container();
        ghostLayer = new PIXI.Container();
        connectLine = new PIXI.Graphics();

        worldContainer.addChild(edgeLayer, nodeLayer, ghostLayer, connectLine);
        app.stage.addChild(worldContainer);

        nodeRenderer = new NodeRenderer(
            nodeLayer,
            onNodeContextMenu,
            onNodeDoubleClick,
        );
        edgeRouter = new EdgeRouter(
            edgeLayer,
            (edgeId) => {
                onEdgeSelected(edgeId, false);
            },
            onEdgeContextMenu,
        );

        selectionManager = new SelectionManager({
            onNodeSelected: (id, additive) => {
                if (connectMode) {
                    handleConnectClick(id);
                } else {
                    onNodeSelected(id, additive);
                }
            },
            onEdgeSelected,
            onEmptyClick: () => {
                if (connectMode) {
                    connectSource = null;
                    connectLine.clear();
                }
                onEmptyClick();
            },
            onNodeMoved,
        });

        updateCanvasTheme();
        drawGrid();
        renderIR(ir);

        viewport.subscribe(($vp) => {
            worldContainer.x = $vp.x;
            worldContainer.y = $vp.y;
            worldContainer.scale.set($vp.scale);
        });

        app.stage.eventMode = "static";
        app.stage.hitArea = new PIXI.Rectangle(
            -100000,
            -100000,
            200000,
            200000,
        );
        app.stage.on("pointerdown", onStagePointerDown);
        app.stage.on("pointermove", onStagePointerMove);
        app.stage.on("pointerup", onStagePointerUp);
        app.stage.on("pointerupoutside", onStagePointerUp);
        app.renderer.on("resize", () => {
            drawGrid(true);
        });

        themeObserver = new MutationObserver(() => {
            updateCanvasTheme();
            drawGrid(true);
        });
        themeObserver.observe(document.documentElement, {
            attributes: true,
            attributeFilter: ["data-theme"],
        });
    });

    onDestroy(() => {
        themeObserver?.disconnect();
        app?.destroy(true, { children: true });
    });

    // IR reactivity

    $: if (nodeRenderer && edgeRouter) renderIR(ir);

    function renderIR(diagram: DiagramIR) {
        nodeRenderer.render(diagram.nodes, selectionManager);
        edgeRouter.render(diagram.edges, diagram.nodes);
        renderGhosts(diagram);
    }

    function renderGhosts(diagram: DiagramIR) {
        ghostLayer.removeChildren();
        for (const n of diagram.nodes.filter((n) => n.has_suggestion)) {
            const g = new PIXI.Graphics();
            g.lineStyle(1.5, 0x818cf8, 0.7, 0.5, true);
            g.drawRoundedRect(n.x, n.y, n.width, n.height, 6);
            ghostLayer.addChild(g);
        }
    }

    // Connect mode logic

    function handleConnectClick(nodeId: string) {
        if (!connectSource) {
            connectSource = nodeId;
        } else if (connectSource === nodeId) {
            // clicked same node - cancel
            connectSource = null;
            connectLine.clear();
        } else {
            onConnectNodes(connectSource, nodeId);
            connectSource = null;
            connectLine.clear();
        }
    }

    // Draw live wire from source node center to mouse
    function drawConnectLine(mouseX: number, mouseY: number) {
        if (!connectSource) return;
        const srcNode = ir.nodes.find((n) => n.id === connectSource);
        if (!srcNode) return;

        const vp = {
            x: worldContainer.x,
            y: worldContainer.y,
            scale: worldContainer.scale.x,
        };
        const srcCx = srcNode.x + srcNode.width / 2;
        const srcCy = srcNode.y + srcNode.height / 2;
        // Convert mouse screen coords to world coords
        const wx = (mouseX - vp.x) / vp.scale;
        const wy = (mouseY - vp.y) / vp.scale;

        connectLine.clear();
        connectLine.lineStyle(1.5, 0x6366f1, 0.8);
        connectLine.moveTo(srcCx, srcCy);
        connectLine.lineTo(wx, wy);
        // Small dot at source
        connectLine.beginFill(0x6366f1, 1);
        connectLine.drawCircle(srcCx, srcCy, 4);
        connectLine.endFill();
    }

    // Grid

    let gridLayer: PIXI.Graphics | null = null;

    function getCssVar(name: string): string {
        return getComputedStyle(document.documentElement)
            .getPropertyValue(name)
            .trim();
    }

    function hexToNumber(hex: string): number {
        const cleaned = hex.replace("#", "");
        const parsed = parseInt(cleaned, 16);
        return Number.isNaN(parsed) ? 0x0a0a0f : parsed;
    }

    function updateCanvasTheme() {
        const bg = getCssVar("--canvas-bg") || "#0d1017";
        app.renderer.background.color = hexToNumber(bg);
    }

    function drawGrid(force = false) {
        if (gridLayer && !force) return;
        if (gridLayer) gridLayer.destroy();
        gridLayer = new PIXI.Graphics();
        const minor = getCssVar("--canvas-grid") || "#1a1a25";
        const majorC = getCssVar("--canvas-grid-major") || "#1f1f30";
        const minorColor = hexToNumber(minor);
        const majorColor = hexToNumber(majorC);
        const size = 20,
            major = 100,
            w = 20000,
            h = 20000;
        const ox = -w / 2,
            oy = -h / 2;
        for (let x = ox; x <= ox + w; x += size) {
            gridLayer.lineStyle(
                1,
                (x - ox) % major === 0 ? majorColor : minorColor,
                1,
            );
            gridLayer.moveTo(x, oy);
            gridLayer.lineTo(x, oy + h);
        }
        for (let y = oy; y <= oy + h; y += size) {
            gridLayer.lineStyle(
                1,
                (y - oy) % major === 0 ? majorColor : minorColor,
                1,
            );
            gridLayer.moveTo(ox, y);
            gridLayer.lineTo(ox + w, y);
        }
        worldContainer.addChildAt(gridLayer, 0);
    }

    // Pan and zoom

    function onStagePointerDown(e: PIXI.FederatedPointerEvent) {
        if (e.button === 1 || (e.button === 0 && spaceHeld)) {
            isPanning = true;
            panStart = { x: e.globalX, y: e.globalY };
            viewport.subscribe((vp) => {
                vpSnapshot = { ...vp };
            })();
        } else if (e.button === 0 && !connectMode) {
            clearSelection();
            onEmptyClick();
        }
    }

    function onStagePointerMove(e: PIXI.FederatedPointerEvent) {
        if (isPanning) {
            const dx = e.globalX - panStart.x;
            const dy = e.globalY - panStart.y;
            viewport.update((vp) => ({
                ...vp,
                x: vpSnapshot.x + dx,
                y: vpSnapshot.y + dy,
            }));
        }
        if (connectMode && connectSource) {
            drawConnectLine(e.globalX, e.globalY);
        }
    }

    function onStagePointerUp() {
        isPanning = false;
    }

    function onWheel(e: WheelEvent) {
        e.preventDefault();
        zoom(-e.deltaY, e.offsetX, e.offsetY);
    }

    function isEditableTarget(e: KeyboardEvent): boolean {
        const el = e.target as HTMLElement | null;
        if (!el) return false;
        if (el.isContentEditable) return true;
        const tag = el.tagName.toLowerCase();
        return tag === "input" || tag === "textarea" || tag === "select";
    }

    function onKeyDown(e: KeyboardEvent) {
        if (isEditableTarget(e)) return;
        if (e.code === "Space" && !e.repeat) {
            spaceHeld = true;
            e.preventDefault();
        }
        if (e.key === "Escape") {
            connectSource = null;
            connectLine?.clear();
        }

        // Ctrl+= or Ctrl++ -> zoom in
        if (e.ctrlKey && (e.key === "=" || e.key === "+")) {
            e.preventDefault();
            const r = container?.getBoundingClientRect();
            zoom(
                1,
                r ? r.width / 2 : window.innerWidth / 2,
                r ? r.height / 2 : window.innerHeight / 2,
            );
        }
        // Ctrl+- -> zoom out
        if (e.ctrlKey && e.key === "-") {
            e.preventDefault();
            const r = container?.getBoundingClientRect();
            zoom(
                -1,
                r ? r.width / 2 : window.innerWidth / 2,
                r ? r.height / 2 : window.innerHeight / 2,
            );
        }
        // Ctrl+Shift+C -> center
        if (e.ctrlKey && e.shiftKey && (e.key === "C" || e.key === "c")) {
            e.preventDefault();
            onCenterRequested();
        }

        if (e.key === "Delete" || e.key === "Backspace") {
            const tag = (document.activeElement?.tagName ?? "").toLowerCase();
            if (tag !== "input" && tag !== "textarea" && tag !== "select") {
                onDeleteSelected();
            }
        }
    }
    function onKeyUp(e: KeyboardEvent) {
        if (e.code === "Space") spaceHeld = false;
    }

    // Cursor changes based on mode
    $: cursorStyle = connectMode
        ? connectSource
            ? "crosshair"
            : "cell"
        : "default";
</script>

<svelte:window on:keydown={onKeyDown} on:keyup={onKeyUp} />

<div
    bind:this={container}
    class="canvas-host"
    style="cursor: {cursorStyle}"
    on:wheel={onWheel}
    on:contextmenu|preventDefault
    role="application"
    aria-label="Diagram canvas"
></div>

<style>
    .canvas-host {
        width: 100%;
        height: 100%;
        overflow: hidden;
        outline: none;
        border-radius: 14px;
        background:
            radial-gradient(
                900px 320px at 20% -10%,
                #5b6ef518 0%,
                transparent 62%
            ),
            linear-gradient(180deg, #0a101a 0%, #090e15 100%);
    }

    .canvas-host :global(canvas) {
        display: block;
        width: 100%;
        height: 100%;
        border-radius: 14px;
    }
</style>

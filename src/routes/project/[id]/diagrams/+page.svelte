<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { get } from "svelte/store";
    import { page } from "$app/stores";
    import { invoke } from "@tauri-apps/api/core";
    import PixiCanvas from "$lib/canvas/PixiCanvas.svelte";
    import SequenceDiagram from "$lib/canvas/SequenceDiagram.svelte";
    import ParametricDiagram from "$lib/canvas/ParametricDiagram.svelte";
    import ModelTree from "$lib/panels/ModelTree.svelte";
    import Properties from "$lib/panels/Properties.svelte";
    import ContextMenu from "$lib/canvas/ContextMenu.svelte";
    import type { MenuItem } from "$lib/canvas/ContextMenu.svelte";
    import {
        loadProject,
        diagrams,
        nodes,
        edges,
        saveNode,
        saveEdge,
        removeNode,
        removeEdge,
        canCreateSubsystem,
        canEditNode,
        canEditEdge,
        readOnly,
    } from "$lib/store/model";
    import {
        activeDiagramId,
        viewport,
        zoom as zoomStore,
        canvasToModel,
    } from "$lib/store/canvas";
    import {
        Plus,
        ZoomIn,
        ZoomOut,
        Maximize2,
        Share2,
        Layout,
    } from "lucide-svelte";
    import { autoLayout } from "$lib/canvas/LayoutEngine";
    import type {
        Diagram,
        DiagramIR,
        IRNode,
        IREdge,
        DiagramElement,
        NodeKind,
        DiagramKind,
        EdgeKind,
        Node,
        Edge,
    } from "$lib/types";
    import { v4 as uuidv4 } from "uuid";
    import { downloadBlob, slugify } from "$lib/utils/csv";
    import { fade, fly } from "svelte/transition";
    import { cubicOut } from "svelte/easing";

    $: projectId = $page.params.id;

    let activeDiagram: Diagram | null = null;
    let ir: DiagramIR | null = null;
    let elements: DiagramElement[] = [];
    let canvasHost: HTMLElement | null = null;

    // ── Selection → properties panel ─────────────────────────────────────────
    let selectedNode: Node | null = null;
    let selectedEdge: Edge | null = null;

    // ── Connect mode ──────────────────────────────────────────────────────────
    let connectMode = false;
    let pendingEdgeKind: EdgeKind = "composes";

    // ── Context menu ──────────────────────────────────────────────────────────
    let contextMenu: { x: number; y: number; items: MenuItem[] } | null = null;

    // ── Modals ────────────────────────────────────────────────────────────────
    let showNewDiagram = false;
    let newDiagramName = "";
    let newDiagramKind: DiagramKind = "bdd";

    let showAddNode = false;
    let newNodeName = "";
    let newNodeKind: NodeKind = "block";
    let addNodeError = "";

    let showAddExisting = false;
    $: existingNotInDiagram = $nodes.filter(
        (n) => !elements.some((el) => el.node_id === n.id),
    );

    // ── Live traceability data ─────────────────────────────────────────────────
    let commentCounts: Record<string, number> = {};
    let suspectNodeIds = new Set<string>(); // node IDs that are suspect link targets

    async function loadLiveTraceabilityData() {
        const [counts, suspects] = await Promise.all([
            invoke<Record<string, number>>('get_comment_counts', { projectId }),
            invoke<Array<{ target_node_id: string }>>('get_suspect_links', { projectId }),
        ]);
        commentCounts = counts;
        suspectNodeIds = new Set(suspects.map(s => s.target_node_id));
        // Re-run IR with fresh data so properties panel updates immediately
        rebuildIR();
    }

    // Re-run IR whenever the model store changes (nodes/edges added, removed, or
    // edited anywhere in the app — e.g. from the requirements page or AI flows).
    $: $nodes, $edges, rebuildIR();

    // Re-run IR whenever live data changes (comments/suspects updated externally)
    $: if (commentCounts || suspectNodeIds) rebuildIR();

    // Computed irNode for Properties panel — avoids non-null assertions in template
    $: selectedIrNode = (selectedNode && ir)
        ? (ir.nodes.find(n => n.id === selectedNode.id) ?? null)
        : null;

    const ZOOM_MIN = 0.2;
    const ZOOM_MAX = 3.5;
    const FIT_PADDING = 80;

    const DIAGRAM_KINDS: { value: DiagramKind; label: string }[] = [
        { value: "bdd", label: "Block Definition Diagram" },
        { value: "ibd", label: "Internal Block Diagram" },
        { value: "usecase", label: "Use Case Diagram" },
        { value: "sequence", label: "Sequence Diagram" },
        { value: "statemachine", label: "State Machine Diagram" },
        { value: "parametric", label: "Parametric Diagram" },
    ];

    const NODE_KINDS: { value: NodeKind; label: string }[] = [
        { value: "block", label: "Block" },
        { value: "requirement", label: "Requirement" },
        { value: "interface", label: "Interface" },
        { value: "port", label: "Port" },
        { value: "use_case", label: "Use Case" },
        { value: "actor", label: "Actor" },
        { value: "test_case", label: "Test Case" },
        { value: "function", label: "Function" },
        { value: "stakeholder", label: "Stakeholder" },
        { value: "external", label: "External" },
        { value: "value_type", label: "Value Type" },
        { value: "constraint_block", label: "Constraint Block" },
        { value: "state", label: "State" },
    ];

    const EDGE_KINDS: { value: EdgeKind; label: string }[] = [
        { value: "composes", label: "<<composes>>" },
        { value: "specializes", label: "<<specializes>>" },
        { value: "satisfies", label: "<<satisfies>>" },
        { value: "realizes", label: "<<realizes>>" },
        { value: "traces", label: "<<traces>>" },
        { value: "verifies", label: "<<verifies>>" },
        { value: "allocates", label: "<<allocates>>" },
        { value: "refines", label: "<<refines>>" },
        { value: "connects", label: "<<connects>>" },
        { value: "transition", label: "<<transition>>" },
        { value: "binding_connector", label: "<<bindingConnector>>" },
    ];

    // Typed pill list for the connect-mode edge kind bar — avoids `as EdgeKind` in template
    const PILL_EDGE_KINDS: { kind: EdgeKind; label: string; color: string }[] = [
        { kind: "satisfies",        label: "Satisfies",        color: "#6366f1" },
        { kind: "verifies",         label: "Verifies",         color: "#22c55e" },
        { kind: "refines",          label: "Refines",          color: "#3b82f6" },
        { kind: "derives",          label: "Derives",          color: "#8b5cf6" },
        { kind: "traces",           label: "Traces",           color: "#f59e0b" },
        { kind: "allocates",        label: "Allocates",        color: "#ec4899" },
        { kind: "composes",         label: "Composes",         color: "#14b8a6" },
        { kind: "realizes",         label: "Realizes",         color: "#64748b" },
        { kind: "connects",         label: "Connects",         color: "#38bdf8" },
        { kind: "transition",       label: "Transition",       color: "#16a34a" },
        { kind: "binding_connector",label: "Binding",          color: "#ea580c" },
    ];

    const NODE_SIZES: Record<NodeKind, [number, number]> = {
        block:            [180, 90],
        requirement:      [220, 100],
        interface:        [170, 85],
        port:             [20, 20],
        use_case:         [160, 72],
        actor:            [70, 90],
        test_case:        [180, 80],
        function:         [160, 72],
        stakeholder:      [140, 72],
        external:         [190, 80],
        value_type:       [160, 72],
        constraint_block: [200, 90],
        state:            [160, 72],
    };

    onMount(async () => {
        await loadProject(projectId);
        if ($diagrams.length > 0) await selectDiagram($diagrams[0]);
        await loadLiveTraceabilityData();

    });

    // ── Diagram CRUD ──────────────────────────────────────────────────────────

    async function createDiagram() {
        if ($readOnly || !canCreateSubsystem()) return;
        if (!newDiagramName.trim()) return;
        const now = new Date().toISOString();
        const diagram: Diagram = {
            id: uuidv4(),
            project_id: projectId,
            kind: newDiagramKind,
            name: newDiagramName.trim(),
            description: "",
            layout_options: {},
            created_at: now,
            modified_at: now,
        };
        await invoke("upsert_diagram", { diagram });
        diagrams.update((ds) => [...ds, diagram]);
        showNewDiagram = false;
        newDiagramName = "";
        await selectDiagram(diagram);
    }

    async function deleteDiagram(d: Diagram) {
        if ($readOnly || !canCreateSubsystem()) return;
        if (!confirm(`Delete diagram "${d.name}"? This cannot be undone.`))
            return;
        await invoke("delete_diagram", { diagramId: d.id });
        diagrams.update((ds) => ds.filter((x) => x.id !== d.id));
        if (activeDiagram?.id === d.id) {
            const remaining = $diagrams;
            if (remaining.length > 0) await selectDiagram(remaining[0]);
            else {
                activeDiagram = null;
                elements = [];
                ir = null;
            }
        }
    }

    async function selectDiagram(diagram: Diagram) {
        activeDiagram = diagram;
        activeDiagramId.set(diagram.id);
        // Reset viewport immediately so old zoom doesn't carry over between diagrams
        viewport.set({ x: 0, y: 0, scale: 1 });
        elements = await invoke<DiagramElement[]>("diagram_elements", {
            diagramId: diagram.id,
        });
        selectedNode = null;
        selectedEdge = null;
        rebuildIR();
        // Center after Svelte has rendered the new IR into the canvas.
        // Use two nested requestAnimationFrames to ensure PixiJS has also flushed.
        requestAnimationFrame(() => requestAnimationFrame(() => centerOnAll()));
    }

    // ── Node CRUD ─────────────────────────────────────────────────────────────

    async function addNode() {
        if ($readOnly || !canCreateSubsystem()) return;
        if (!newNodeName.trim() || !activeDiagram) return;
        addNodeError = "";
        const now = new Date().toISOString();
        const id = uuidv4();
        const data = defaultNodeData(newNodeKind);
        const node = {
            id,
            project_id: projectId,
            kind: newNodeKind,
            name: newNodeName.trim(),
            description: "",
            data,
            meta: {},
            created_at: now,
            modified_at: now,
        };

        try {
            await saveNode(node);
        } catch (e) {
            addNodeError = `${e}`;
            return;
        }

        const [w, h] = NODE_SIZES[newNodeKind];
        const offset = elements.length * 24;
        const el: DiagramElement = {
            id: uuidv4(),
            diagram_id: activeDiagram.id,
            node_id: id,
            x: 80 + offset,
            y: 80 + offset,
            width: w,
            height: h,
            collapsed: false,
            style_overrides: {},
        };

        try {
            await invoke("upsert_diagram_element", { element: el });
        } catch (e) {
            addNodeError = `${e}`;
            return;
        }

        elements = [...elements, el];
        showAddNode = false;
        newNodeName = "";
        rebuildIR();
    }

    async function quickAddNode(kind: NodeKind, name: string) {
        if ($readOnly || !canCreateSubsystem()) return;
        if (!activeDiagram) return;
        const canvasRect = document.querySelector('.canvas-area')?.getBoundingClientRect();
        const cx = canvasRect ? canvasRect.width / 2 : 400;
        const cy = canvasRect ? canvasRect.height / 2 : 300;
        const modelX = cx + (Math.random() - 0.5) * 200;
        const modelY = cy + (Math.random() - 0.5) * 150;
        addNodeError = "";
        const now = new Date().toISOString();
        const id = uuidv4();
        const data = defaultNodeData(kind);
        const node = {
            id,
            project_id: projectId,
            kind,
            name,
            description: "",
            data,
            meta: {},
            created_at: now,
            modified_at: now,
        };
        try {
            await saveNode(node);
        } catch (e) {
            addNodeError = `${e}`;
            return;
        }
        const [w, h] = NODE_SIZES[kind];
        const el: DiagramElement = {
            id: uuidv4(),
            diagram_id: activeDiagram.id,
            node_id: id,
            x: modelX - w / 2,
            y: modelY - h / 2,
            width: w,
            height: h,
            collapsed: false,
            style_overrides: {},
        };
        try {
            await invoke("upsert_diagram_element", { element: el });
        } catch (e) {
            addNodeError = `${e}`;
            return;
        }
        elements = [...elements, el];
        rebuildIR();
    }

    async function addExistingToDiagram(node: Node) {
        if ($readOnly || !canCreateSubsystem()) return;
        if (!activeDiagram) return;
        const [w, h] = NODE_SIZES[node.kind as NodeKind] ?? [160, 80];
        const offset = elements.length * 24;
        const el: DiagramElement = {
            id: uuidv4(),
            diagram_id: activeDiagram.id,
            node_id: node.id,
            x: 80 + offset,
            y: 80 + offset,
            width: w,
            height: h,
            collapsed: false,
            style_overrides: {},
        };
        await invoke("upsert_diagram_element", { element: el });
        elements = [...elements, el];
        showAddExisting = false;
        rebuildIR();
    }

    function defaultNodeData(kind: NodeKind) {
        switch (kind) {
            case "requirement":
                return {
                    kind,
                    req_id: "",
                    text: "",
                    rationale: "",
                    priority: "shall",
                    status: "draft",
                    allocations: [],
                };
            case "block":
                return { kind, is_abstract: false };
            case "port":
                return { kind, direction: "inout" };
            case "use_case":
                return { kind, level: "user" };
            case "test_case":
                return { kind, status: "not_run" };
            case "value_type":
                return { kind, base_type: "Real" };
            case "constraint_block":
                return { kind, expression: "", parameters: [] };
            case "state":
                return { kind };
            case "external":
                return { kind };
            default:
                return { kind };
        }
    }

    async function placeExistingAt(
        nodeId: string,
        modelX: number,
        modelY: number,
    ) {
        if ($readOnly) return;
        if (!activeDiagram) return;
        const node = $nodes.find((n) => n.id === nodeId);
        if (!node) return;
        const [w, h] = NODE_SIZES[node.kind as NodeKind] ?? [160, 80];
        const x = modelX - w / 2;
        const y = modelY - h / 2;

        const existing = elements.find((el) => el.node_id === nodeId);
        const el: DiagramElement = {
            id: existing?.id ?? uuidv4(),
            diagram_id: activeDiagram.id,
            node_id: nodeId,
            x,
            y,
            width: existing?.width ?? w,
            height: existing?.height ?? h,
            collapsed: existing?.collapsed ?? false,
            style_overrides: existing?.style_overrides ?? {},
        };
        await invoke("upsert_diagram_element", { element: el });
        if (existing) {
            const idx = elements.findIndex((e) => e.node_id === nodeId);
            if (idx >= 0) elements[idx] = el;
        } else {
            elements = [...elements, el];
        }
        rebuildIR();
    }

    // Called by ModelTree via pointer-event drag (not HTML5 drag API, which
    // Tauri's WebView intercepts at the OS level before the DOM sees it).
    async function onNodeDroppedOnCanvas(nodeId: string, clientX: number, clientY: number) {
        if (!activeDiagram || !canvasHost) return;
        const canvasEl = canvasHost.querySelector('canvas');
        const rect = (canvasEl ?? canvasHost).getBoundingClientRect();
        const canvasX = clientX - rect.left;
        const canvasY = clientY - rect.top;
        const model = canvasToModel(canvasX, canvasY, get(viewport));

        // On a BDD diagram, dropping an unassigned requirement onto a block
        // automatically assigns it to that block.
        if (activeDiagram.kind === 'bdd') {
            const droppedNode = $nodes.find((n) => n.id === nodeId);
            if (droppedNode?.kind === 'requirement') {
                const reqData = droppedNode.data as { kind: string; allocations?: string[] };
                const currentAllocs = (reqData.allocations ?? []).filter((a) => a.trim());
                if (currentAllocs.length === 0) {
                    // Find a block in the diagram that the drop point lands inside
                    const hitBlock = ir?.nodes.find(
                        (n) => n.kind === 'block'
                            && model.x >= n.x && model.x <= n.x + n.width
                            && model.y >= n.y && model.y <= n.y + n.height,
                    );
                    if (hitBlock) {
                        const blockNode = $nodes.find((n) => n.id === hitBlock.id);
                        if (blockNode) {
                            const updated: Node = {
                                ...droppedNode,
                                data: { ...reqData, allocations: [blockNode.name] },
                            };
                            await saveNode(updated);
                        }
                    }
                }
            }
        }

        await placeExistingAt(nodeId, model.x, model.y);
    }

    // ── Edge creation (connect mode) ──────────────────────────────────────────

    async function onConnectNodes(sourceId: string, targetId: string) {
        if ($readOnly || !canCreateSubsystem()) return;
        if (!activeDiagram) return;
        const now = new Date().toISOString();
        const edge: Edge = {
            id: uuidv4(),
            project_id: projectId,
            kind: pendingEdgeKind,
            source_id: sourceId,
            target_id: targetId,
            label: "",
            meta: {},
            created_at: now,
            modified_at: now,
        };
        await saveEdge(edge);
        rebuildIR();
    }

    // ── Deletion ──────────────────────────────────────────────────────────────

    async function handleDeleteNode(nodeId: string) {
        if ($readOnly || !canCreateSubsystem()) return;
        const node = $nodes.find((n) => n.id === nodeId) ?? null;
        if (!node || !canEditNode(node)) return;
        await removeNode(nodeId);
        elements = elements.filter((el) => el.node_id !== nodeId);
        if (selectedNode?.id === nodeId) selectedNode = null;
        rebuildIR();
    }

    async function handleDeleteEdge(edgeId: string) {
        if ($readOnly || !canCreateSubsystem()) return;
        const edge = $edges.find((e) => e.id === edgeId) ?? null;
        if (!edge || !canEditEdge(edge)) return;
        await removeEdge(edgeId);
        if (selectedEdge?.id === edgeId) selectedEdge = null;
        rebuildIR();
    }

    function onDeleteSelected() {
        if (selectedNode) handleDeleteNode(selectedNode.id);
        else if (selectedEdge) handleDeleteEdge(selectedEdge.id);
    }

    async function handleDuplicateNode(nodeId: string) {
        if ($readOnly || !canCreateSubsystem()) return;
        const original = $nodes.find((n) => n.id === nodeId);
        const originalEl = elements.find((el) => el.node_id === nodeId);
        if (!original || !originalEl || !activeDiagram || !canEditNode(original))
            return;
        const now = new Date().toISOString();
        const newId = uuidv4();
        const copy: Node = {
            ...original,
            id: newId,
            name: original.name + " (copy)",
            created_at: now,
            modified_at: now,
        };
        await saveNode(copy);
        const newEl: DiagramElement = {
            ...originalEl,
            id: uuidv4(),
            node_id: newId,
            x: originalEl.x + 24,
            y: originalEl.y + 24,
        };
        await invoke("upsert_diagram_element", { element: newEl });
        elements = [...elements, newEl];
        rebuildIR();
    }

    // ── Context menu handlers ─────────────────────────────────────────────────

    function onNodeContextMenu(nodeId: string, x: number, y: number) {
        selectedNode = $nodes.find((n) => n.id === nodeId) ?? null;
        selectedEdge = null;

        const connectEdges = $edges.filter(
            (e) =>
                e.kind === "connects" &&
                (e.source_id === nodeId || e.target_id === nodeId),
        );
        const disconnectItems = connectEdges.map((e) => {
            const otherId = e.source_id === nodeId ? e.target_id : e.source_id;
            const otherName =
                $nodes.find((n) => n.id === otherId)?.name ?? otherId.slice(0, 8);
            return {
                label: `Disconnect from ${otherName}`,
                action: `delete-edge:${e.id}`,
                danger: true,
            };
        });

        contextMenu = {
            x,
            y,
            items: [
                {
                    label: "Delete element",
                    action: `delete-node:${nodeId}`,
                    danger: true,
                },
                { label: "Duplicate", action: `duplicate-node:${nodeId}` },
                {
                    label: "Connect from here",
                    action: `connect-from:${nodeId}`,
                },
                ...disconnectItems,
                { label: "", action: "", separator: true },
                { label: "Copy ID", action: `copy-id:${nodeId}` },
            ],
        };
    }

    function onEdgeContextMenu(edgeId: string, x: number, y: number) {
        selectedEdge = $edges.find((e) => e.id === edgeId) ?? null;
        selectedNode = null;
        contextMenu = {
            x,
            y,
            items: [
                {
                    label: "Delete relationship",
                    action: `delete-edge:${edgeId}`,
                    danger: true,
                },
            ],
        };
    }

    async function onContextMenuSelect(action: string) {
        contextMenu = null;
        if (action.startsWith("delete-node:")) {
            await handleDeleteNode(action.slice("delete-node:".length));
        } else if (action.startsWith("duplicate-node:")) {
            await handleDuplicateNode(action.slice("duplicate-node:".length));
        } else if (action.startsWith("connect-from:")) {
            connectMode = true;
        } else if (action.startsWith("copy-id:")) {
            const id = action.slice("copy-id:".length);
            await navigator.clipboard.writeText(id);
        } else if (action.startsWith("delete-edge:")) {
            await handleDeleteEdge(action.slice("delete-edge:".length));
        }
    }

    // ── Selection ─────────────────────────────────────────────────────────────

    function onNodeSelected(nodeId: string, _additive: boolean) {
        selectedNode = $nodes.find((n) => n.id === nodeId) ?? null;
        selectedEdge = null;
    }

    function onEdgeSelected(edgeId: string, _additive: boolean) {
        selectedEdge = $edges.find((e) => e.id === edgeId) ?? null;
        selectedNode = null;
    }

    function onEmptyClick() {
        selectedNode = null;
        selectedEdge = null;
    }

    async function onUpdateEdge(e: CustomEvent<Edge>) {
        if ($readOnly || !canCreateSubsystem() || !canEditEdge(e.detail)) return;
        await saveEdge(e.detail);
        selectedEdge = e.detail;
        rebuildIR();
    }

    // ── IR rebuild ────────────────────────────────────────────────────────────

    function rebuildIR() {
        if (!activeDiagram) return;

        // Build edge lookup maps for live traceability
        // satisfies: req → block (source=req, target=block) — find blocks that reqs satisfy
        // verifies:  test → req  (source=test, target=req)
        const satisfiesTargets = new Map<string, string[]>(); // blockId → reqIds that satisfy it
        const verifiesSources  = new Map<string, string[]>(); // testId → reqIds it verifies
        for (const e of $edges) {
            if (e.kind === 'satisfies') {
                const list = satisfiesTargets.get(e.target_id) ?? [];
                list.push(e.source_id);
                satisfiesTargets.set(e.target_id, list);
            } else if (e.kind === 'verifies') {
                const list = verifiesSources.get(e.source_id) ?? [];
                list.push(e.target_id);
                verifiesSources.set(e.source_id, list);
            }
        }

        const irNodes: IRNode[] = elements
            .map((el) => {
                const node = $nodes.find((n) => n.id === el.node_id);
                if (!node) return null;

                // Compute live traceability fields
                const linkedReqIds = satisfiesTargets.get(node.id) ?? [];
                const satisfies_count = linkedReqIds.length;

                const verifies_count = (verifiesSources.get(node.id) ?? []).length;
                const open_comments  = commentCounts[node.id] ?? 0;
                const has_suspect    = suspectNodeIds.has(node.id);

                // Coverage: for blocks, check if all linked requirements are approved
                let coverage_status: IRNode['coverage_status'] = 'n/a';
                if (node.kind === 'block') {
                    if (linkedReqIds.length === 0) {
                        coverage_status = 'none';
                    } else {
                        const linkedReqs = linkedReqIds.map(rid => $nodes.find(n => n.id === rid)).filter(Boolean);
                        const allApproved = linkedReqs.every(r => {
                            const d = r!.data as { status?: string };
                            return d.status === 'approved';
                        });
                        coverage_status = allApproved ? 'full' : 'partial';
                    }
                } else if (node.kind === 'requirement') {
                    // For requirements: are they satisfied by any block?
                    const isSatisfied = $edges.some(e => e.kind === 'satisfies' && e.source_id === node.id);
                    coverage_status = isSatisfied ? 'full' : 'none';
                }

                return {
                    id: node.id,
                    kind: node.kind,
                    name: node.name,
                    description: node.description,
                    data: node.data,
                    x: el.x,
                    y: el.y,
                    width: el.width,
                    height: el.height,
                    collapsed: el.collapsed,
                    style_overrides: el.style_overrides,
                    has_suggestion: false,
                    satisfies_count,
                    verifies_count,
                    open_comments,
                    has_suspect,
                    coverage_status,
                    linked_req_ids: linkedReqIds,
                } satisfies IRNode;
            })
            .filter(Boolean) as IRNode[];

        const nodeIds = new Set(irNodes.map((n) => n.id));
        const irEdges: IREdge[] = $edges
            .filter((e) => nodeIds.has(e.source_id) && nodeIds.has(e.target_id))
            .map((e) => ({
                id: e.id,
                kind: e.kind,
                source_id: e.source_id,
                target_id: e.target_id,
                label: e.label,
                waypoints: [],
                has_suggestion: false,
            }));

        ir = {
            diagram_id: activeDiagram.id,
            kind: activeDiagram.kind,
            name: activeDiagram.name,
            nodes: irNodes,
            edges: irEdges,
        };
    }

    function canvasSize(): { w: number; h: number } {
        // Try the bound div first, then fall back to the canvas element inside it
        if (canvasHost) {
            const w = canvasHost.offsetWidth || canvasHost.clientWidth;
            const h = canvasHost.offsetHeight || canvasHost.clientHeight;
            if (w > 0 && h > 0) return { w, h };
            // Try the child canvas element (PixiJS appends it)
            const cv = canvasHost.querySelector("canvas");
            if (cv)
                return {
                    w: cv.offsetWidth || cv.clientWidth,
                    h: cv.offsetHeight || cv.clientHeight,
                };
        }
        return { w: window.innerWidth, h: window.innerHeight };
    }

    function exportDiagramPng() {
        if (!canvasHost || !activeDiagram) return;
        const canvasEl = canvasHost.querySelector(
            "canvas",
        ) as HTMLCanvasElement | null;
        if (!canvasEl) return;
        canvasEl.toBlob((blob) => {
            if (!blob) return;
            const name = slugify(activeDiagram.name || "diagram") || "diagram";
            downloadBlob(blob, `${name}.png`);
        }, "image/png");
    }

    function zoomBy(step: number) {
        const { w, h } = canvasSize();
        zoomStore(step, w / 2, h / 2);
    }

    function centerOnAll() {
        if (elements.length === 0) return;
        const { w, h } = canvasSize();
        if (w === 0 || h === 0) return;

        const bounds = elements.reduce(
            (acc, el) => ({
                minX: Math.min(acc.minX, el.x),
                minY: Math.min(acc.minY, el.y),
                maxX: Math.max(acc.maxX, el.x + el.width),
                maxY: Math.max(acc.maxY, el.y + el.height),
            }),
            {
                minX: Infinity,
                minY: Infinity,
                maxX: -Infinity,
                maxY: -Infinity,
            },
        );

        const contentW = Math.max(1, bounds.maxX - bounds.minX);
        const contentH = Math.max(1, bounds.maxY - bounds.minY);
        const scaleX = (w - FIT_PADDING * 2) / contentW;
        const scaleY = (h - FIT_PADDING * 2) / contentH;
        // Cap fit-to-view at 1.0 so a single small diagram doesn't zoom in past 100%
        const FIT_MAX = 1.0;
        const nextScale = Math.min(
            FIT_MAX,
            Math.max(ZOOM_MIN, Math.min(scaleX, scaleY)),
        );
        const contentCx = bounds.minX + contentW / 2;
        const contentCy = bounds.minY + contentH / 2;

        viewport.set({
            x: w / 2 - contentCx * nextScale,
            y: h / 2 - contentCy * nextScale,
            scale: nextScale,
        });
    }

    // ── AI diagram generation ─────────────────────────────────────────────────

    let aiGenerating = false;
    let aiGenerateError = '';

    async function aiGenerateDiagram() {
        if (!activeDiagram || $readOnly) return;
        aiGenerating = true;
        aiGenerateError = '';
        try {
            const nodeInputs = $nodes.map((n) => ({
                id: n.id,
                kind: n.kind,
                name: n.name,
                description: n.description ?? null,
            }));
            const edgeInputs = $edges.map((e) => ({
                source_id: e.source_id,
                target_id: e.target_id,
                kind: e.kind,
            }));
            const raw = await invoke<string>('ai_generate_diagram', {
                diagramKind: activeDiagram.kind,
                diagramName: activeDiagram.name,
                nodes: nodeInputs,
                edges: edgeInputs,
            });
            const parsed = JSON.parse(raw) as {
                placements: { node_id: string; x: number; y: number; width: number; height: number }[];
            };
            if (!parsed.placements?.length) {
                aiGenerateError = 'AI returned no placements. Try again.';
                return;
            }
            // Upsert each placement as a diagram element
            for (const p of parsed.placements) {
                const node = $nodes.find((n) => n.id === p.node_id);
                if (!node) continue;
                const [defW, defH] = NODE_SIZES[node.kind as NodeKind] ?? [180, 90];
                const existing = elements.find((el) => el.node_id === p.node_id);
                const el: DiagramElement = {
                    id: existing?.id ?? uuidv4(),
                    diagram_id: activeDiagram!.id,
                    node_id: p.node_id,
                    x: p.x,
                    y: p.y,
                    width: p.width ?? defW,
                    height: p.height ?? defH,
                    collapsed: existing?.collapsed ?? false,
                    style_overrides: existing?.style_overrides ?? {},
                };
                await invoke('upsert_diagram_element', { element: el });
                const idx = elements.findIndex((e) => e.node_id === p.node_id);
                if (idx >= 0) elements[idx] = el;
                else elements.push(el);
            }
            elements = [...elements];
            rebuildIR();
            requestAnimationFrame(() => requestAnimationFrame(() => centerOnAll()));
        } catch (e) {
            aiGenerateError = String(e).includes('no_api_key')
                ? 'No AI key configured. Add one in Settings.'
                : `AI error: ${String(e)}`;
        } finally {
            aiGenerating = false;
        }
    }

    // ── Auto-layout ───────────────────────────────────────────────────────────

    async function runLayout() {
        if ($readOnly || !canCreateSubsystem()) return;
        if (!activeDiagram || !ir || ir.nodes.length === 0) return;
        const result = await autoLayout(activeDiagram.id, ir.nodes, ir.edges);
        for (const [nodeId, pos] of result.positions) {
            const existing = elements.find((el) => el.node_id === nodeId);
            const el: DiagramElement = {
                id: existing?.id ?? uuidv4(),
                diagram_id: activeDiagram!.id,
                node_id: nodeId,
                x: pos.x,
                y: pos.y,
                width: pos.width,
                height: pos.height,
                collapsed: existing?.collapsed ?? false,
                style_overrides: existing?.style_overrides ?? {},
            };
            await invoke("upsert_diagram_element", { element: el });
            const idx = elements.findIndex((e) => e.node_id === nodeId);
            if (idx >= 0) elements[idx] = el;
            else elements.push(el);
        }
        rebuildIR();
    }

    async function onNodeMoved(nodeId: string, x: number, y: number) {
        if ($readOnly || !canCreateSubsystem()) return;
        const node = $nodes.find((n) => n.id === nodeId) ?? null;
        if (!node || !canEditNode(node)) return;
        const existing = elements.find((el) => el.node_id === nodeId);
        if (!existing || !activeDiagram) return;
        const updated = { ...existing, x, y };
        await invoke("upsert_diagram_element", { element: updated });
        const idx = elements.findIndex((el) => el.node_id === nodeId);
        if (idx >= 0) elements[idx] = updated;
        rebuildIR();
    }

    // Keep selectedNode in sync when store updates (e.g. after Properties saves)
    $: if (selectedNode) {
        const fresh = $nodes.find((n) => n.id === selectedNode!.id);
        if (fresh) selectedNode = fresh;
    }
</script>

<div class="layout page-body no-scroll">
    <!-- Left sidebar -->
    <aside class="sidebar">
        <ModelTree enableDrag defaultExpanded={['block']} onNodeDropped={onNodeDroppedOnCanvas} />
        {#if activeDiagram && !$readOnly && canCreateSubsystem()}
        <div class="quick-add-section">
            <div class="quick-add-label">Quick Add</div>
            <div class="quick-add-grid">
                <button class="qa-btn block" on:click={() => quickAddNode('block', 'New Block')}>
                    <span class="qa-icon">▣</span>Block
                </button>
                <button class="qa-btn requirement" on:click={() => quickAddNode('requirement', 'New Requirement')}>
                    <span class="qa-icon">◈</span>Req
                </button>
                <button class="qa-btn interface" on:click={() => quickAddNode('interface', 'New Interface')}>
                    <span class="qa-icon">⬡</span>Interface
                </button>
                <button class="qa-btn actor" on:click={() => quickAddNode('actor', 'New Actor')}>
                    <span class="qa-icon">◉</span>Actor
                </button>
                <button class="qa-btn use_case" on:click={() => quickAddNode('use_case', 'New Use Case')}>
                    <span class="qa-icon">◎</span>UseCase
                </button>
                <button class="qa-btn test_case" on:click={() => quickAddNode('test_case', 'New Test Case')}>
                    <span class="qa-icon">◧</span>Test
                </button>
            </div>
        </div>
        {/if}
        <div class="sidebar-footer">
            {#if activeDiagram}
                <button
                    class="btn-add-node"
                    on:click|stopPropagation={() => (showAddNode = true)}
                    disabled={$readOnly || !canCreateSubsystem()}
                >
                    + New Element
                </button>
                {#if existingNotInDiagram.length > 0}
                    <button
                        class="btn-add-existing"
                        on:click|stopPropagation={() =>
                            (showAddExisting = true)}
                        disabled={$readOnly || !canCreateSubsystem()}
                    >
                        + Place Existing
                    </button>
                {/if}
            {:else}
                <button
                    class="btn-add-node"
                    on:click|stopPropagation={() => (showNewDiagram = true)}
                    disabled={$readOnly || !canCreateSubsystem()}
                >
                    + New Diagram
                </button>
            {/if}
        </div>
    </aside>

    <!-- Tab bar + toolbar -->
    <div class="diagram-tabs">
        <div class="diagram-tab-strip">
        {#each $diagrams as d (d.id)}
            <div
                class="diagram-tab-wrap"
                class:active={activeDiagram?.id === d.id}
            >
                <button
                    class="diagram-tab"
                    class:active={activeDiagram?.id === d.id}
                    on:click={() => selectDiagram(d)}>{d.name}</button
                >
                {#if !$readOnly && canCreateSubsystem()}
                    <button
                        class="tab-delete"
                        on:click|stopPropagation={() => deleteDiagram(d)}
                        title="Delete diagram">x</button
                    >
                {/if}
            </div>
        {/each}
        <button
            class="diagram-tab new-tab"
            on:click={() => (showNewDiagram = true)}
            disabled={$readOnly || !canCreateSubsystem()}
        >
            <Plus size={12} /> New
        </button
        >
        </div>

        <div class="toolbar-cluster left">
            {#if activeDiagram}
                <span class="diagram-chip">{activeDiagram.kind.toUpperCase()}</span>
                <input
                    class="diagram-desc-input"
                    placeholder="Add description..."
                    value={activeDiagram.description ?? ''}
                    on:change={async (e) => {
                        if (!activeDiagram) return;
                        const desc = e.currentTarget.value;
                        const updated = { ...activeDiagram, description: desc };
                        await invoke('upsert_diagram', { diagram: updated });
                        activeDiagram = updated;
                    }}
                />
            {/if}
            {#if ir}
                <span class="diagram-stat">{ir.nodes.length} nodes</span>
                <span class="diagram-stat">{ir.edges.length} links</span>
            {/if}
        </div>

        <div class="tab-spacer"></div>

        <div class="toolbar-cluster">
        <!-- Edge kind picker (shown in connect mode) -->
        {#if connectMode}
        <div class="edge-kind-bar">
            <span class="edge-kind-label">Edge:</span>
            {#each PILL_EDGE_KINDS as ek}
            <button
                class="edge-kind-pill"
                class:selected={pendingEdgeKind === ek.kind}
                style="--pill-color: {ek.color}"
                on:click={() => (pendingEdgeKind = ek.kind)}
            >{ek.label}</button>
            {/each}
        </div>
        {/if}

        {#if activeDiagram?.kind !== 'sequence' && activeDiagram?.kind !== 'parametric'}
        <button
            class="tab-action icon-only"
            on:click={() => zoomBy(1)}
            title="Zoom in"><ZoomIn size={14} /></button
        >
        <button
            class="tab-action icon-only"
            on:click={() => zoomBy(-1)}
            title="Zoom out"><ZoomOut size={14} /></button
        >
        <span class="zoom-readout">{Math.round($viewport.scale * 100)}%</span>
        <button
            class="tab-action icon-only"
            on:click={centerOnAll}
            title="Fit to canvas"><Maximize2 size={14} /></button
        >
        {/if}
        {#if activeDiagram}
            <button
                class="tab-action"
                on:click={exportDiagramPng}
                title="Export diagram PNG">Export PNG</button
            >
        {/if}

        <!-- Connect toggle -->
        <button
            class="tab-action"
            class:active={connectMode}
            on:click={() => {
                connectMode = !connectMode;
            }}
            title="Connect mode - click source then target"
            disabled={$readOnly || !canCreateSubsystem()}
        >
            {#if connectMode}<Share2 size={13} /> Cancel{:else}<Share2
                    size={13}
                /> Connect{/if}
        </button>

        {#if ir && ir.nodes.length > 0}
            <button
                class="tab-action"
                on:click={runLayout}
                title="Auto layout"
                disabled={$readOnly || !canCreateSubsystem()}><Layout
                    size={13}
                /> Layout</button
            >
        {/if}
        {#if activeDiagram}
            <button
                class="tab-action ai-gen-btn"
                on:click={aiGenerateDiagram}
                title="Use AI to populate and lay out this diagram"
                disabled={aiGenerating || $readOnly}
            >{aiGenerating ? 'Generating…' : '✦ AI Generate'}</button>
        {/if}
        {#if aiGenerateError}
            <span class="ai-gen-error">{aiGenerateError}</span>
        {/if}
        </div>
    </div>

    <!-- Canvas -->
    <main
        class="canvas-area"
        bind:this={canvasHost}
    >
        {#if ir}
            <div in:fade={{ duration: 200 }} style="height:100%;width:100%;display:flex;flex-direction:column;min-height:0;">
                {#if ir.kind === 'sequence'}
                    <SequenceDiagram
                        {ir}
                        {onNodeSelected}
                        {onEdgeSelected}
                        {onEmptyClick}
                        {onNodeMoved}
                        {onNodeContextMenu}
                        {onEdgeContextMenu}
                        {onDeleteSelected}
                    />
                {:else if ir.kind === 'parametric'}
                    <ParametricDiagram
                        {ir}
                        {onNodeSelected}
                        {onEdgeSelected}
                        {onEmptyClick}
                        {onNodeMoved}
                        {onNodeContextMenu}
                        {onEdgeContextMenu}
                        {onDeleteSelected}
                    />
                {:else}
                    <PixiCanvas
                        {ir}
                        {connectMode}
                        {onNodeMoved}
                        {onNodeSelected}
                        {onEdgeSelected}
                        {onEmptyClick}
                        {onConnectNodes}
                        {onNodeContextMenu}
                        {onEdgeContextMenu}
                        {onDeleteSelected}
                        onCenterRequested={centerOnAll}
                    />
                {/if}
            </div>
            <!-- Minimap (hidden for sequence/parametric diagrams — they render natively) -->
            <div class="minimap" aria-hidden="true" class:hidden={ir.kind === 'sequence' || ir.kind === 'parametric'}>
                <div class="minimap-viewport" style="
                    left: {Math.max(0, Math.min(70, (-$viewport.x / (ir.nodes.length > 0 ? 2000 : 1000)) * 100))}%;
                    top: {Math.max(0, Math.min(70, (-$viewport.y / (ir.nodes.length > 0 ? 2000 : 1000)) * 100))}%;
                "></div>
                {#each ir.nodes as mmnode (mmnode.id)}
                <div class="minimap-node" style="
                    left: {Math.max(0, Math.min(96, (mmnode.x / 2000) * 100))}%;
                    top: {Math.max(0, Math.min(96, (mmnode.y / 2000) * 100))}%;
                    background: {mmnode.kind === 'requirement' ? '#3b82f6' : mmnode.kind === 'block' ? '#6366f1' : '#64748b'};
                "></div>
                {/each}
            </div>
        {:else if $diagrams.length === 0}
            <div class="canvas-empty">
                <div class="canvas-empty-icon" aria-hidden="true">
                    <svg width="52" height="52" viewBox="0 0 52 52" fill="none">
                        <rect
                            x="4"
                            y="4"
                            width="20"
                            height="20"
                            rx="4"
                            stroke="var(--surface-border-bright)"
                            stroke-width="1.5"
                        />
                        <rect
                            x="28"
                            y="4"
                            width="20"
                            height="20"
                            rx="4"
                            stroke="var(--surface-border)"
                            stroke-width="1.5"
                            stroke-dasharray="3 2"
                        />
                        <rect
                            x="4"
                            y="28"
                            width="20"
                            height="20"
                            rx="4"
                            stroke="var(--surface-border)"
                            stroke-width="1.5"
                            stroke-dasharray="3 2"
                        />
                        <line
                            x1="24"
                            y1="14"
                            x2="28"
                            y2="14"
                            stroke="var(--accent)"
                            stroke-width="1.5"
                        />
                        <line
                            x1="14"
                            y1="24"
                            x2="14"
                            y2="28"
                            stroke="var(--accent)"
                            stroke-width="1.5"
                        />
                    </svg>
                </div>
                <div class="canvas-empty-title">No diagrams yet</div>
                <div class="canvas-empty-body">
                    Create your first diagram - BDD, IBD, Use Case, Sequence, or
                    State Machine.
                </div>
                <button
                    class="btn-primary"
                    on:click={() => (showNewDiagram = true)}
                    disabled={$readOnly || !canCreateSubsystem()}
                >
                    <Plus size={14} /> Create a diagram
                </button>
            </div>
        {:else}
            <div class="canvas-empty">
                <div
                    class="canvas-empty-title"
                    style="font-size:var(--text-sm)"
                >
                    Select a diagram above
                </div>
            </div>
        {/if}
    </main>

    <!-- Right properties panel -->
    <Properties
        node={selectedNode}
        edge={selectedEdge}
        readOnly={$readOnly || !canCreateSubsystem()}
        irNode={selectedIrNode}
        on:updateEdge={onUpdateEdge}
        on:deleteNode={(e) => handleDeleteNode(e.detail)}
        on:deleteEdge={(e) => handleDeleteEdge(e.detail)}
    />
</div>

<!-- Context menu (outside .layout so it's not clipped) -->
{#if contextMenu}
    <ContextMenu
        x={contextMenu.x}
        y={contextMenu.y}
        items={contextMenu.items}
        on:select={(e) => onContextMenuSelect(e.detail)}
        on:close={() => (contextMenu = null)}
    />
{/if}

<!-- ── New Diagram modal ───────────────────────────────────────────────────── -->
{#if showNewDiagram}
    <div
        class="modal-backdrop"
        role="button"
        tabindex="0"
        aria-label="Close new diagram modal"
        on:click={() => (showNewDiagram = false)}
        on:keydown={(e) =>
            (e.key === "Enter" || e.key === " ") && (showNewDiagram = false)}
        transition:fade={{ duration: 150 }}
    >
        <div
            class="modal"
            on:click|stopPropagation
            role="dialog"
            aria-label="New diagram"
            transition:fly={{ y: 10, duration: 180, easing: cubicOut }}
        >
            <h2>New Diagram</h2>
            <label class="field-label">
                Name
                <input
                    class="field"
                    placeholder="e.g. System BDD"
                    bind:value={newDiagramName}
                    on:keydown={(e) => e.key === "Enter" && createDiagram()}
                />
            </label>
            <label class="field-label">
                Type
                <select class="field" bind:value={newDiagramKind}>
                    {#each DIAGRAM_KINDS as dk}
                        <option value={dk.value}>{dk.label}</option>
                    {/each}
                </select>
            </label>
            <div class="modal-actions">
                <button
                    class="btn-ghost"
                    on:click={() => (showNewDiagram = false)}>Cancel</button
                >
                <button
                    class="btn-primary"
                    on:click={createDiagram}
                    disabled={!newDiagramName.trim() || !canCreateSubsystem()}
                    >Create</button
                >
            </div>
        </div>
    </div>
{/if}

<!-- ── Place Existing modal ──────────────────────────────────────────────────── -->
{#if showAddExisting}
    <div
        class="modal-backdrop"
        role="button"
        tabindex="0"
        aria-label="Close"
        on:click={() => (showAddExisting = false)}
        on:keydown={(e) => e.key === "Escape" && (showAddExisting = false)}
        transition:fade={{ duration: 150 }}
    >
        <div
            class="modal modal-wide"
            on:click|stopPropagation
            role="dialog"
            aria-label="Place existing element"
            transition:fly={{ y: 10, duration: 180, easing: cubicOut }}
        >
            <h2>Place Existing Element</h2>
            <p class="modal-hint">
                Select a model element to add to this diagram.
            </p>
            <div class="existing-list">
                {#each existingNotInDiagram as n (n.id)}
                    <button
                        class="existing-item"
                        on:click={() => addExistingToDiagram(n)}
                    >
                        <span class="existing-kind">{n.kind}</span>
                        <span class="existing-name">{n.name}</span>
                    </button>
                {/each}
            </div>
            <div class="modal-actions">
                <button
                    class="btn-ghost"
                    on:click={() => (showAddExisting = false)}>Cancel</button
                >
            </div>
        </div>
    </div>
{/if}

<!-- ── Add Element modal ───────────────────────────────────────────────────── -->
{#if showAddNode}
    <div
        class="modal-backdrop"
        role="button"
        tabindex="0"
        aria-label="Close add element modal"
        on:click={() => (showAddNode = false)}
        on:keydown={(e) =>
            (e.key === "Enter" || e.key === " ") && (showAddNode = false)}
        transition:fade={{ duration: 150 }}
    >
        <div
            class="modal"
            on:click|stopPropagation
            role="dialog"
            aria-label="Add element"
            transition:fly={{ y: 10, duration: 180, easing: cubicOut }}
        >
            <h2>Add Element</h2>
            <label class="field-label">
                Type
                <select class="field" bind:value={newNodeKind}>
                    {#each NODE_KINDS as nk}
                        <option value={nk.value}>{nk.label}</option>
                    {/each}
                </select>
            </label>
            <label class="field-label">
                Name
                <input
                    class="field"
                    placeholder="Element name"
                    bind:value={newNodeName}
                    on:keydown={(e) => e.key === "Enter" && addNode()}
                />
            </label>
            {#if addNodeError}
                <div class="modal-error">{addNodeError}</div>
            {/if}
            <div class="modal-actions">
                <button class="btn-ghost" on:click={() => (showAddNode = false)}
                    >Cancel</button
                >
                <button
                    class="btn-primary"
                    on:click={addNode}
                    disabled={!newNodeName.trim()}>Add</button
                >
            </div>
        </div>
    </div>
{/if}

<style>
    .layout {
        display: grid;
        grid-template-columns: 240px 1fr 300px;
        grid-template-rows: 44px 1fr;
        grid-template-areas:
            "sidebar tabs    tabs"
            "sidebar canvas  props";
        height: 100%;
        overflow: hidden;
        background: linear-gradient(180deg, #090d15 0%, var(--surface-base) 100%);
        min-height: 0;
        flex: 1;
    }

    .sidebar {
        grid-area: sidebar;
        border-right: 1px solid var(--surface-border);
        background: linear-gradient(180deg, #101723 0%, var(--surface-raised) 100%);
        overflow: hidden;
        display: flex;
        flex-direction: column;
        min-height: 0;
        box-shadow: inset -1px 0 0 #ffffff08;
    }

    .sidebar-footer {
        padding: var(--space-3);
        border-top: 1px solid var(--surface-border);
        flex-shrink: 0;
        background: #0f1522;
    }

    .btn-add-node {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: var(--space-1);
        width: 100%;
        padding: 9px var(--space-2);
        background: linear-gradient(135deg, #5b6ef536 0%, #5b6ef520 100%);
        border: 1px solid var(--accent);
        border-radius: var(--radius-md);
        color: var(--accent-hover);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-add-node:hover {
        background: var(--accent);
        color: white;
    }
    .btn-add-node:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .btn-add-existing {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: var(--space-1);
        width: 100%;
        margin-top: var(--space-1);
        padding: 9px var(--space-2);
        background: #ffffff04;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-add-existing:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }
    .btn-add-existing:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .modal-wide {
        width: 460px;
        max-height: 70vh;
    }

    .modal-hint {
        font-size: var(--text-sm);
        color: var(--text-muted);
        margin: 0;
    }

    .existing-list {
        display: flex;
        flex-direction: column;
        gap: 2px;
        overflow-y: auto;
        max-height: 340px;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
    }

    .existing-item {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        padding: var(--space-2) var(--space-3);
        background: none;
        border: none;
        text-align: left;
        cursor: pointer;
        transition: background var(--transition-fast);
    }
    .existing-item:hover {
        background: var(--surface-hover);
    }

    .existing-kind {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
        min-width: 80px;
        font-family: var(--font-mono);
    }

    .existing-name {
        font-size: var(--text-sm);
        color: var(--text-primary);
    }

    .diagram-tabs {
        grid-area: tabs;
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-1) var(--space-3);
        border-bottom: 1px solid var(--surface-border);
        background: linear-gradient(180deg, var(--surface-raised), #0f1623);
        overflow-x: auto;
        overflow-y: hidden;
        flex-shrink: 0;
        min-height: 44px;
        box-shadow: inset 0 -1px 0 #00000040;
    }

    .diagram-tab-strip {
        display: flex;
        align-items: center;
        gap: 2px;
        min-width: max-content;
    }

    .diagram-tab-wrap {
        display: flex;
        align-items: center;
        border: 1px solid transparent;
        border-radius: var(--radius-md);
        height: 30px;
        padding-right: 2px;
        transition:
            background var(--transition-fast),
            border-color var(--transition-fast),
            box-shadow var(--transition-fast);
    }
    .diagram-tab-wrap.active {
        background: var(--surface-overlay);
        border-color: var(--surface-border-bright);
        box-shadow: 0 1px 6px #00000030;
    }

    .diagram-tab {
        display: inline-flex;
        align-items: center;
        gap: var(--space-1);
        height: 100%;
        padding: 0 var(--space-2) 0 var(--space-3);
        background: none;
        border: none;
        color: var(--text-secondary);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        cursor: pointer;
        white-space: nowrap;
        transition: color var(--transition-fast);
    }
    .diagram-tab:hover {
        color: var(--text-primary);
    }
    .diagram-tab.active {
        color: #f5f8ff;
    }
    .diagram-tab.new-tab {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        height: 30px;
        background: #ffffff05;
        border: 1px dashed var(--surface-border-bright);
        border-radius: var(--radius-md);
        color: var(--text-muted);
        padding: 0 var(--space-3);
    }
    .diagram-tab.new-tab:hover {
        color: var(--accent-hover);
        border-color: var(--accent);
        background: var(--accent-dim);
    }

    .tab-delete {
        width: 18px;
        height: 18px;
        background: transparent;
        border: 1px solid transparent;
        color: var(--text-muted);
        font-size: 11px;
        cursor: pointer;
        padding: 0;
        border-radius: var(--radius-sm);
        opacity: 0;
        transition: all var(--transition-fast);
        line-height: 1;
    }
    .diagram-tab-wrap:hover .tab-delete {
        opacity: 1;
    }
    .tab-delete:hover {
        color: var(--color-error);
        background: #ef444420;
    }
    .diagram-tab:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .tab-spacer {
        flex: 1;
        min-width: var(--space-2);
    }

    .toolbar-cluster {
        display: inline-flex;
        align-items: center;
        gap: var(--space-1);
        min-width: max-content;
    }

    .toolbar-cluster.left {
        margin-left: var(--space-2);
    }

    .diagram-chip {
        display: inline-flex;
        align-items: center;
        padding: 2px 8px;
        border-radius: 999px;
        border: 1px solid var(--surface-border-bright);
        background: #ffffff08;
        color: var(--text-secondary);
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        font-family: var(--font-mono);
    }

    .diagram-stat {
        display: inline-flex;
        align-items: center;
        padding: 2px 8px;
        border-radius: 999px;
        border: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        color: var(--text-muted);
        font-size: 10px;
        letter-spacing: 0.04em;
    }

    .edge-kind-select {
        height: 30px;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border-bright);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-xs);
        padding: 0 var(--space-2);
        cursor: pointer;
        transition: border-color var(--transition-fast);
    }
    .edge-kind-select:focus {
        outline: none;
        border-color: var(--accent);
    }

    .tab-action {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        height: 30px;
        padding: 0 var(--space-2);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        color: var(--text-muted);
        cursor: pointer;
        font-size: var(--text-sm);
        border-radius: var(--radius-md);
        transition: all var(--transition-fast);
        white-space: nowrap;
    }
    .tab-action:hover {
        color: var(--text-primary);
        background: var(--surface-hover);
        border-color: var(--surface-border-bright);
    }
    .tab-action.icon-only {
        width: 30px;
        padding: 0;
        justify-content: center;
    }
    .tab-action:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }
    .tab-action.active {
        color: var(--accent-hover);
        background: var(--accent-dim);
        border: 1px solid var(--accent);
    }
    .tab-action.ai-gen-btn {
        color: #a78bfa;
        border-color: #7c3aed44;
        background: #7c3aed12;
    }
    .tab-action.ai-gen-btn:hover:not(:disabled) {
        color: #c4b5fd;
        background: #7c3aed22;
        border-color: #7c3aed88;
    }
    .ai-gen-error {
        font-size: var(--text-xs);
        color: var(--color-warning);
        max-width: 200px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .zoom-readout {
        min-width: 48px;
        text-align: center;
        font-family: var(--font-mono);
        font-size: 11px;
        color: var(--text-secondary);
        background: #ffffff06;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        padding: 6px var(--space-2) 5px;
    }

    .canvas-area {
        grid-area: canvas;
        overflow: hidden;
        position: relative;
        min-height: 0;
        height: 100%;
        background:
            radial-gradient(1200px 420px at 20% -5%, #5b6ef514 0%, transparent 62%),
            linear-gradient(180deg, #0b1019 0%, #090d15 100%);
    }

    .canvas-area::before {
        content: "";
        position: absolute;
        inset: 8px;
        border-radius: 14px;
        border: 1px solid var(--surface-border-subtle);
        box-shadow: inset 0 1px 0 #ffffff0a;
        pointer-events: none;
        z-index: 2;
    }

    .canvas-area :global(.canvas-host) {
        position: absolute;
        inset: 8px;
        border-radius: 14px;
        overflow: hidden;
    }

    .canvas-area.drag-over {
        outline: none;
    }

    .canvas-area.drag-over::before {
        border-color: var(--accent);
        box-shadow:
            inset 0 1px 0 #ffffff0a,
            0 0 0 2px var(--accent-glow);
    }

    .canvas-empty-icon {
        margin-bottom: var(--space-2);
    }
    .canvas-empty-title {
        font-size: var(--text-lg);
        font-weight: var(--weight-semibold);
        color: var(--text-secondary);
        letter-spacing: var(--tracking-tight);
    }
    .canvas-empty-body {
        font-size: var(--text-sm);
        color: var(--text-muted);
        max-width: 300px;
        text-align: center;
        line-height: var(--leading-relaxed);
        margin-bottom: var(--space-2);
    }

    .canvas-empty {
        position: relative;
        z-index: 3;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: var(--space-4);
        height: 100%;
        color: var(--text-muted);
        font-size: var(--text-sm);
    }

    .empty-hint {
        font-size: var(--text-lg);
        color: var(--text-secondary);
    }

    /* ── Modals ── */
    .modal-backdrop {
        position: fixed;
        inset: 0;
        background: #00000080;
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: var(--z-modal);
    }

    .modal {
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-xl);
        padding: var(--space-6);
        width: 360px;
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        box-shadow: var(--shadow-lg);
    }

    .modal h2 {
        font-size: var(--text-lg);
        font-weight: var(--weight-semibold);
    }

    .field-label {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
        font-size: var(--text-sm);
        color: var(--text-secondary);
    }

    .field {
        width: 100%;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-family: var(--font-sans);
        font-size: var(--text-base);
        padding: var(--space-2) var(--space-3);
        transition: border-color var(--transition-fast);
    }
    .field:focus {
        outline: none;
        border-color: var(--accent);
    }

    .modal-actions {
        display: flex;
        gap: var(--space-2);
        justify-content: flex-end;
    }

    .modal-error {
        font-size: var(--text-sm);
        color: var(--color-error);
        background: #ef444420;
        border: 1px solid #ef444440;
        border-radius: var(--radius-md);
        padding: var(--space-2) var(--space-3);
    }

    .btn-primary {
        padding: var(--space-2) var(--space-4);
        background: var(--accent);
        color: white;
        border: none;
        border-radius: var(--radius-md);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        cursor: pointer;
        transition: background var(--transition-fast);
    }
    .btn-primary:hover:not(:disabled) {
        background: var(--accent-hover);
    }
    .btn-primary:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .btn-ghost {
        padding: var(--space-2) var(--space-4);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-ghost:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }

    @media (max-width: 1100px) {
        .layout {
            grid-template-columns: 220px 1fr 260px;
        }
    }

    @media (max-width: 900px) {
        .layout {
            grid-template-columns: 1fr;
            grid-template-rows: 38px auto 1fr auto;
            grid-template-areas:
                "tabs"
                "sidebar"
                "canvas"
                "props";
        }
        .sidebar {
            border-right: none;
            border-bottom: 1px solid var(--surface-border);
        }
    }

    /* ── Quick Add panel ── */
    .quick-add-section {
        padding: 0.5rem 0.6rem;
        border-bottom: 1px solid var(--surface-border);
        flex-shrink: 0;
    }
    .quick-add-label {
        font-size: 0.7rem;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
        margin-bottom: 0.4rem;
    }
    .quick-add-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.3rem;
    }
    .qa-btn {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        padding: 0.3rem 0.4rem;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: 4px;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 0.75rem;
        transition: all 0.15s;
    }
    .qa-btn:hover { background: var(--surface-hover); color: var(--text-primary); }
    .qa-btn.block:hover { border-color: #6366f1; color: #818cf8; }
    .qa-btn.requirement:hover { border-color: #3b82f6; color: #60a5fa; }
    .qa-btn.interface:hover { border-color: #8b5cf6; color: #a78bfa; }
    .qa-btn.actor:hover { border-color: #f59e0b; color: #fbbf24; }
    .qa-btn.use_case:hover { border-color: #10b981; color: #34d399; }
    .qa-btn.test_case:hover { border-color: #ef4444; color: #f87171; }
    .qa-icon { font-size: 0.85rem; }

    /* ── Edge kind pill bar ── */
    .edge-kind-bar {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        flex-wrap: wrap;
    }
    .edge-kind-label {
        font-size: 0.7rem;
        color: var(--text-muted);
        font-weight: 600;
        white-space: nowrap;
    }
    .edge-kind-pill {
        padding: 0.2rem 0.55rem;
        border-radius: 999px;
        border: 1px solid rgba(128, 128, 128, 0.3);
        background: transparent;
        color: var(--text-muted);
        cursor: pointer;
        font-size: 0.72rem;
        transition: all 0.15s;
        white-space: nowrap;
        height: 24px;
        display: inline-flex;
        align-items: center;
    }
    .edge-kind-pill:hover {
        background: rgba(128, 128, 128, 0.12);
        color: var(--text-secondary);
    }
    .edge-kind-pill.selected {
        background: rgba(99, 102, 241, 0.18);
        color: var(--pill-color, #6366f1);
        border-color: var(--pill-color, #6366f1);
        font-weight: 600;
    }

    /* ── Minimap ── */
    .minimap.hidden {
        display: none;
    }
    .minimap {
        position: absolute;
        bottom: 20px;
        right: 20px;
        width: 120px;
        height: 80px;
        background: rgba(10, 10, 20, 0.82);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 5px;
        overflow: hidden;
        pointer-events: none;
        z-index: 10;
    }
    .minimap-viewport {
        position: absolute;
        width: 28%;
        height: 28%;
        border: 1px solid rgba(255, 255, 255, 0.3);
        background: rgba(255, 255, 255, 0.05);
        border-radius: 2px;
    }
    .minimap-node {
        position: absolute;
        width: 4px;
        height: 3px;
        border-radius: 1px;
        opacity: 0.8;
    }

    /* ── Diagram description input ── */
    .diagram-desc-input {
        background: transparent;
        border: none;
        border-bottom: 1px solid transparent;
        color: var(--text-muted);
        font-size: 0.78rem;
        padding: 0.1rem 0.25rem;
        outline: none;
        transition: border-color 0.15s, color 0.15s;
        max-width: 200px;
        min-width: 80px;
        font-family: var(--font-sans);
    }
    .diagram-desc-input:hover,
    .diagram-desc-input:focus {
        border-bottom-color: var(--surface-border-bright);
        color: var(--text-secondary);
    }
    .diagram-desc-input::placeholder { color: var(--text-muted); opacity: 0.55; }
</style>

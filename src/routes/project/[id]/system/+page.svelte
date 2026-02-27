<script lang="ts">
    import { onMount } from "svelte";
    import { get } from "svelte/store";
    import { page } from "$app/stores";
    import { invoke } from "@tauri-apps/api/core";
    import { goto } from "$app/navigation";
    import PixiCanvas from "$lib/canvas/PixiCanvas.svelte";
    import ContextMenu from "$lib/canvas/ContextMenu.svelte";
    import type { MenuItem } from "$lib/canvas/ContextMenu.svelte";
    import Properties from "$lib/panels/Properties.svelte";
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
        currentProject,
        readOnly,
    } from "$lib/store/model";
    import { activeDiagramId, viewport } from "$lib/store/canvas";
    import type {
        Diagram,
        DiagramElement,
        DiagramIR,
        IRNode,
        IREdge,
        Node,
        Edge,
        EdgeKind,
    } from "$lib/types";
    import { v4 as uuidv4 } from "uuid";
    import { parseCsv, toCsv, downloadBlob, slugify } from "$lib/utils/csv";
    import { fade, fly } from "svelte/transition";
    import { cubicOut } from "svelte/easing";
    import {
        Plus,
        ZoomIn,
        ZoomOut,
        Maximize2,
        Share2,
        Download,
        Upload,
        Layers,
        Cpu,
    } from "lucide-svelte";

    $: projectId = $page.params.id;

    let activeDiagram: Diagram | null = null;
    let elements: DiagramElement[] = [];
    let ir: DiagramIR | null = null;

    let selectedNode: Node | null = null;
    let selectedEdge: Edge | null = null;

    let connectMode = false;
    let pendingEdgeKind: EdgeKind = "connects";

    let contextMenu: { x: number; y: number; items: MenuItem[] } | null = null;

    let showAddSubsystem = false;
    let newSubsystemName = "";
    let showSeed = false;
    let seedSubsystems = "";
    let seedDescription = "";
    let seeding = false;
    let canvasHost: HTMLDivElement | null = null;
    let importSubsystemInput: HTMLInputElement | null = null;
    let importingSubsystems = false;
    let importSubsystemError = "";

    const EDGE_KINDS: { value: EdgeKind; label: string }[] = [
        { value: "connects", label: "�connects�" },
        { value: "composes", label: "�composes�" },
    ];

    const SYSTEM_NODE_NAME = "System";
    const SYSTEM_SIZE = { width: 260, height: 140 };
    const SUBSYSTEM_SIZE = { width: 220, height: 120 };
    const CENTER_STYLE = { center_label: true, hide_stereo: true };
    const ZOOM_MIN = 0.2;
    const ZOOM_MAX = 3.5;
    const FIT_PADDING = 80;

    onMount(async () => {
        await loadProject(projectId);
        await ensureSystemDiagram();
    });

    function isSystemRoot(node: Node | null): boolean {
        if (!node) return false;
        return (node.meta as Record<string, unknown>)?.system_root === true;
    }

    function getSystemRootNode(): Node | null {
        return $nodes.find((n) => isSystemRoot(n)) ?? null;
    }

    $: subsystemNodes = $nodes.filter(
        (n) =>
            n.kind === "block" &&
            !(n.meta as Record<string, unknown>)?.system_root,
    );
    $: subsystemCount = subsystemNodes.length;

    async function ensureSystemDiagram() {
        const existing = $diagrams.find(
            (d) => d.name.toLowerCase() === "system overview",
        );
        if (existing) {
            await selectDiagram(existing);
            return;
        }
        if (!canCreateSubsystem()) return;

        const now = new Date().toISOString();
        const diagram: Diagram = {
            id: uuidv4(),
            project_id: projectId,
            kind: "bdd",
            name: "System Overview",
            description: "Top-level subsystem architecture",
            layout_options: {},
            created_at: now,
            modified_at: now,
        };
        await invoke("upsert_diagram", { diagram });
        diagrams.update((ds) => [...ds, diagram]);
        await selectDiagram(diagram);
    }

    async function selectDiagram(diagram: Diagram) {
        activeDiagram = diagram;
        activeDiagramId.set(diagram.id);
        elements = await invoke<DiagramElement[]>("diagram_elements", {
            diagramId: diagram.id,
        });
        selectedNode = null;
        selectedEdge = null;
        await ensureSystemRoot(diagram);
        await normalizeSystemElements();
        rebuildIR();
        // Auto-center after DOM has rendered
        setTimeout(centerOnAll, 100);
    }

    async function ensureSystemRoot(diagram: Diagram) {
        let systemNode = getSystemRootNode();

        if (!systemNode) {
            if (!canCreateSubsystem()) return;
            const now = new Date().toISOString();
            systemNode = {
                id: uuidv4(),
                project_id: projectId,
                kind: "block",
                name: SYSTEM_NODE_NAME,
                description: "Top-level system boundary",
                data: { kind: "block", is_abstract: true },
                meta: { system_root: true },
                created_at: now,
                modified_at: now,
            };
            await saveNode(systemNode);
        }

        const exists = elements.find((el) => el.node_id === systemNode!.id);
        if (!exists && canEditNode(systemNode)) {
            const el: DiagramElement = {
                id: uuidv4(),
                diagram_id: diagram.id,
                node_id: systemNode!.id,
                x: 360,
                y: 200,
                width: SYSTEM_SIZE.width,
                height: SYSTEM_SIZE.height,
                collapsed: false,
                style_overrides: { ...CENTER_STYLE },
            };
            await invoke("upsert_diagram_element", { element: el });
            elements = [...elements, el];
        }
    }

    async function normalizeSystemElements() {
        if (!activeDiagram) return;
        const updates: DiagramElement[] = [];

        for (const el of elements) {
            const node = $nodes.find((n) => n.id === el.node_id);
            if (!node) continue;
            if (!canEditNode(node)) continue;
            const isRoot = isSystemRoot(node);
            const target = isRoot ? SYSTEM_SIZE : SUBSYSTEM_SIZE;
            const needsResize =
                el.width < target.width || el.height < target.height;
            const existingStyle = el.style_overrides ?? {};
            const needsStyle =
                existingStyle.center_label !== true ||
                existingStyle.hide_stereo !== true;
            const style = needsStyle
                ? { ...existingStyle, ...CENTER_STYLE }
                : existingStyle;

            if (needsResize || needsStyle) {
                const updated: DiagramElement = {
                    ...el,
                    width: needsResize ? target.width : el.width,
                    height: needsResize ? target.height : el.height,
                    style_overrides: style,
                };
                updates.push(updated);
            }
        }

        if (updates.length === 0) return;
        for (const el of updates) {
            await invoke("upsert_diagram_element", { element: el });
            const idx = elements.findIndex((e) => e.id === el.id);
            if (idx >= 0) elements[idx] = el;
        }
    }

    function rebuildIR() {
        if (!activeDiagram) return;
        const irNodes: IRNode[] = elements
            .map((el) => {
                const node = $nodes.find((n) => n.id === el.node_id);
                if (!node) return null;
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

    let addingSubsystem = false;
    async function addSubsystem() {
        if ($readOnly || !canCreateSubsystem() || addingSubsystem) return;
        if (!newSubsystemName.trim() || !activeDiagram) return;
        addingSubsystem = true;
        try {
            await createSubsystem(newSubsystemName.trim());
            newSubsystemName = "";
            showAddSubsystem = false;
            rebuildIR();
            setTimeout(centerOnAll, 50);
        } finally {
            addingSubsystem = false;
        }
    }

    async function createSubsystem(name: string) {
        if ($readOnly || !canCreateSubsystem()) return;
        if (!activeDiagram) return;
        const now = new Date().toISOString();
        const nodeId = uuidv4();
        const node: Node = {
            id: nodeId,
            project_id: projectId,
            kind: "block",
            name,
            description: "",
            data: { kind: "block", is_abstract: false },
            meta: {},
            created_at: now,
            modified_at: now,
        };
        await saveNode(node);

        const offset = elements.length * 34;
        const el: DiagramElement = {
            id: uuidv4(),
            diagram_id: activeDiagram.id,
            node_id: nodeId,
            x: 140 + offset,
            y: 140 + offset,
            width: SUBSYSTEM_SIZE.width,
            height: SUBSYSTEM_SIZE.height,
            collapsed: false,
            style_overrides: { ...CENTER_STYLE },
        };
        await invoke("upsert_diagram_element", { element: el });
        elements = [...elements, el];

        const systemRoot = getSystemRootNode();
        if (systemRoot) {
            const exists = $edges.find(
                (e) =>
                    (e.source_id === systemRoot.id && e.target_id === nodeId) ||
                    (e.source_id === nodeId && e.target_id === systemRoot.id),
            );
            if (!exists) {
                const edge: Edge = {
                    id: uuidv4(),
                    project_id: projectId,
                    kind: "composes",
                    source_id: systemRoot.id,
                    target_id: nodeId,
                    label: "",
                    meta: { system_link: true },
                    created_at: now,
                    modified_at: now,
                };
                await saveEdge(edge);
            }
        }
    }

    async function seedSystem() {
        if ($readOnly || !canCreateSubsystem()) return;
        if (seeding || !activeDiagram) return;
        seeding = true;
        try {
            const names = seedSubsystems
                .split(/[,\\n]/)
                .map((n) => n.trim())
                .filter(Boolean);
            if (seedDescription.trim()) {
                const root = getSystemRootNode();
                if (root) {
                    await saveNode({
                        ...root,
                        description: seedDescription.trim(),
                        modified_at: new Date().toISOString(),
                    });
                }
            }

            const existingNames = new Set(
                $nodes
                    .filter((n) => n.kind === "block")
                    .map((n) => n.name.trim().toLowerCase()),
            );

            for (const name of names) {
                if (existingNames.has(name.toLowerCase())) continue;
                await createSubsystem(name);
            }

            seedSubsystems = "";
            seedDescription = "";
            showSeed = false;
            rebuildIR();
            setTimeout(centerOnAll, 50);
        } finally {
            seeding = false;
        }
    }

    function projectSlug(): string {
        return slugify($currentProject?.name ?? projectId) || projectId;
    }

    function exportSubsystemsCsv() {
        const header = ["name", "description"];
        const rows = subsystemNodes.map((n) => [
            n.name ?? "",
            n.description ?? "",
        ]);
        const csv = toCsv([header, ...rows]);
        downloadBlob(
            new Blob([csv], { type: "text/csv;charset=utf-8" }),
            `subsystems-${projectSlug()}.csv`,
        );
    }

    function exportSubsystemsJson() {
        const payload = subsystemNodes.map((n) => ({
            name: n.name ?? "",
            description: n.description ?? "",
        }));
        const json = JSON.stringify(payload, null, 2);
        downloadBlob(
            new Blob([json], { type: "application/json" }),
            `subsystems-${projectSlug()}.json`,
        );
    }

    async function importSubsystemsFile(file: File) {
        if ($readOnly || !canCreateSubsystem()) return;
        importingSubsystems = true;
        importSubsystemError = "";
        try {
            if (!activeDiagram) {
                importSubsystemError = "System diagram not ready yet.";
                return;
            }
            const text = await file.text();
            const lowerName = file.name.toLowerCase();

            let rows: { name: string; description: string }[] = [];
            if (lowerName.endsWith(".json")) {
                const parsed = JSON.parse(text);
                const list = Array.isArray(parsed)
                    ? parsed
                    : (parsed?.subsystems ?? []);
                rows = list.map((item: Record<string, unknown>) => ({
                    name: String(item.name ?? ""),
                    description: String(item.description ?? ""),
                }));
            } else {
                const table = parseCsv(text);
                if (table.length === 0) return;
                const header = table[0].map((h) => h.trim().toLowerCase());
                const nameIdx = header.findIndex((h) =>
                    ["name", "subsystem"].includes(h),
                );
                const descIdx = header.findIndex((h) =>
                    ["description", "desc"].includes(h),
                );
                rows = table.slice(1).map((row) => ({
                    name: nameIdx >= 0 ? row[nameIdx] : "",
                    description: descIdx >= 0 ? row[descIdx] : "",
                }));
            }

            const existingByName = new Map<string, Node>();
            for (const n of subsystemNodes) {
                if (n.name) existingByName.set(n.name.toLowerCase(), n);
            }

            for (const row of rows) {
                const name = (row.name ?? "").trim();
                if (!name) continue;
                const existing = existingByName.get(name.toLowerCase());
                if (existing) {
                    await saveNode({
                        ...existing,
                        description: (row.description ?? "").trim(),
                        modified_at: new Date().toISOString(),
                    });
                } else {
                    await createSubsystem(name);
                    const created = $nodes.find(
                        (n) => n.kind === "block" && n.name === name,
                    );
                    if (created && row.description?.trim()) {
                        await saveNode({
                            ...created,
                            description: row.description.trim(),
                            modified_at: new Date().toISOString(),
                        });
                    }
                }
            }
        } catch (err) {
            importSubsystemError = String(err);
        } finally {
            importingSubsystems = false;
            if (importSubsystemInput) importSubsystemInput.value = "";
        }
    }

    function onSubsystemFileChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement | null;
        const file = input?.files?.[0];
        if (file) importSubsystemsFile(file);
    }

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

    async function onNodeMoved(nodeId: string, x: number, y: number) {
        if ($readOnly) return;
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

    function onNodeSelected(nodeId: string) {
        selectedNode = $nodes.find((n) => n.id === nodeId) ?? null;
        selectedEdge = null;
    }

    function onEdgeSelected(edgeId: string) {
        selectedEdge = $edges.find((e) => e.id === edgeId) ?? null;
        selectedNode = null;
    }

    function onEmptyClick() {
        selectedNode = null;
        selectedEdge = null;
    }

    function onNodeDoubleClick(nodeId: string) {
        const node = $nodes.find((n) => n.id === nodeId) ?? null;
        if (!node || isSystemRoot(node)) return;
        goto(`/project/${projectId}/subsystem/${nodeId}`);
    }

    async function handleDeleteNode(nodeId: string) {
        if ($readOnly) return;
        const node = $nodes.find((n) => n.id === nodeId) ?? null;
        if (!node || isSystemRoot(node) || !canEditNode(node)) return;
        await removeNode(nodeId);
        elements = elements.filter((el) => el.node_id !== nodeId);
        if (selectedNode?.id === nodeId) selectedNode = null;
        rebuildIR();
    }

    async function handleDeleteEdge(edgeId: string) {
        if ($readOnly) return;
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

    function onNodeContextMenu(nodeId: string, x: number, y: number) {
        selectedNode = $nodes.find((n) => n.id === nodeId) ?? null;
        selectedEdge = null;
        const isRoot = isSystemRoot(selectedNode);

        // Build disconnect items for every connects edge touching this node
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
            items: isRoot
                ? [
                      { label: "Connect from here", action: `connect-from:${nodeId}` },
                      ...disconnectItems,
                  ]
                : [
                      {
                          label: "Delete subsystem",
                          action: `delete-node:${nodeId}`,
                          danger: true,
                      },
                      { label: "Connect from here", action: `connect-from:${nodeId}` },
                      ...disconnectItems,
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
                    label: "Delete connection",
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
        } else if (action.startsWith("connect-from:")) {
            connectMode = true;
        } else if (action.startsWith("delete-edge:")) {
            await handleDeleteEdge(action.slice("delete-edge:".length));
        }
    }

    $: if (selectedNode) {
        const fresh = $nodes.find((n) => n.id === selectedNode!.id);
        if (fresh) selectedNode = fresh;
    }

    function zoomBy(step: number) {
        if (!canvasHost) return;
        const rect = canvasHost.getBoundingClientRect();
        const centerX = rect.width / 2;
        const centerY = rect.height / 2;
        const vp = get(viewport);
        const factor = step > 0 ? 1.1 : 0.9;
        const nextScale = Math.min(
            ZOOM_MAX,
            Math.max(ZOOM_MIN, vp.scale * factor),
        );
        const scaleRatio = nextScale / vp.scale;
        viewport.set({
            x: centerX - scaleRatio * (centerX - vp.x),
            y: centerY - scaleRatio * (centerY - vp.y),
            scale: nextScale,
        });
    }

    function centerOnAll() {
        if (!canvasHost || elements.length === 0) return;
        const rect = canvasHost.getBoundingClientRect();
        const w = rect.width || canvasHost.offsetWidth;
        const h = rect.height || canvasHost.offsetHeight;
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
        const nextScale = Math.min(
            ZOOM_MAX,
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
</script>

<div class="system-editor page-frame">
    <header class="system-header page-header">
        <div class="header-left">
            <div class="page-eyebrow">System</div>
            <h1 class="page-title">System Overview</h1>
            <p class="page-subtitle">
                Define subsystems, connect them, and model your architecture.
            </p>
        </div>
        <div class="stat-bar">
            <div class="stat">
                <div class="stat-value">{subsystemCount}</div>
                <div class="stat-label">Subsystems</div>
            </div>
        </div>
        <div class="header-actions">
            {#if connectMode}
                <select class="edge-kind-select" bind:value={pendingEdgeKind}>
                    {#each EDGE_KINDS as ek}
                        <option value={ek.value}>{ek.label}</option>
                    {/each}
                </select>
            {/if}
            <button
                class="btn-ghost icon-only"
                on:click={() => zoomBy(1)}
                title="Zoom in"><ZoomIn size={14} /></button
            >
            <button
                class="btn-ghost icon-only"
                on:click={() => zoomBy(-1)}
                title="Zoom out"><ZoomOut size={14} /></button
            >
            <button
                class="btn-ghost icon-only"
                on:click={centerOnAll}
                title="Center"><Maximize2 size={14} /></button
            >
            <div class="btn-divider"></div>
            <button
                class="btn-ghost"
                class:active={connectMode}
                on:click={() => (connectMode = !connectMode)}
                disabled={$readOnly || !canCreateSubsystem()}
            >
                <Share2 size={13} />
                {connectMode ? "Cancel" : "Connect"}
            </button>
            <button class="btn-ghost" on:click={exportSubsystemsCsv}
                ><Download size={13} /> CSV</button
            >
            <button class="btn-ghost" on:click={exportSubsystemsJson}
                ><Download size={13} /> JSON</button
            >
            <button
                class="btn-ghost"
                on:click={() => importSubsystemInput?.click()}
                disabled={importingSubsystems || $readOnly || !canCreateSubsystem()}
            >
                <Upload size={13} />
                {importingSubsystems ? "Importing�" : "Import"}
            </button>
            <button
                class="btn-primary"
                on:click={() => (showAddSubsystem = true)}
                disabled={$readOnly || !canCreateSubsystem()}
            >
                <Plus size={13} /> Add Subsystem
            </button>
        </div>
    </header>

    <input
        bind:this={importSubsystemInput}
        type="file"
        accept=".csv,.json"
        on:change={onSubsystemFileChange}
        style="display:none"
    />

    {#if importSubsystemError}
        <div class="import-error" transition:fly={{ y: -8, duration: 200 }}>
            Import failed: {importSubsystemError}
            <button class="import-error-dismiss" on:click={() => (importSubsystemError = "")} aria-label="Dismiss">�</button>
        </div>
    {/if}

    <div class="system-body page-body no-scroll">
        {#if subsystemCount === 0}
            <div class="seed-banner">
                <div class="seed-copy">
                    <div class="seed-title">
                        Start fast: seed your subsystems
                    </div>
                    <div class="seed-subtitle">
                        Add a quick list of top-level subsystems and a system
                        description.
                    </div>
                </div>
                <button
                    class="btn-primary"
                    on:click={() => (showSeed = true)}
                    disabled={$readOnly || !canCreateSubsystem()}
                >
                    Quick Seed
                </button>
            </div>
        {/if}

        <main class="canvas-area" bind:this={canvasHost}>
            {#if ir}
                <div in:fade={{ duration: 200 }} style="height:100%;width:100%;display:flex;flex-direction:column;min-height:0;">
                    <PixiCanvas
                        {ir}
                        {connectMode}
                        {onNodeMoved}
                        onNodeSelected={(id) => onNodeSelected(id)}
                        onEdgeSelected={(id) => onEdgeSelected(id)}
                        {onEmptyClick}
                        {onNodeDoubleClick}
                        {onConnectNodes}
                        {onNodeContextMenu}
                        {onEdgeContextMenu}
                        {onDeleteSelected}
                        onCenterRequested={centerOnAll}
                    />
                </div>
            {:else}
                <div class="canvas-loading" in:fade={{ duration: 150 }}>
                    <div class="spinner spinner-lg" aria-hidden="true"></div>
                    <span>Loading diagram�</span>
                </div>
            {/if}
        </main>

        <Properties
            node={selectedNode}
            edge={selectedEdge}
            readOnly={$readOnly || !canCreateSubsystem()}
            on:deleteNode={(e) => handleDeleteNode(e.detail)}
            on:deleteEdge={(e) => handleDeleteEdge(e.detail)}
            on:updateEdge={(e) => saveEdge(e.detail)}
        />
    </div>
</div>

{#if contextMenu}
    <ContextMenu
        x={contextMenu.x}
        y={contextMenu.y}
        items={contextMenu.items}
        on:select={(e) => onContextMenuSelect(e.detail)}
        on:close={() => (contextMenu = null)}
    />
{/if}

{#if showAddSubsystem}
    <div
        class="modal-backdrop"
        on:click={() => !addingSubsystem && (showAddSubsystem = false)}
        role="presentation"
        transition:fade={{ duration: 150 }}
    >
        <div
            class="modal"
            on:click|stopPropagation
            role="dialog"
            aria-label="Add subsystem"
            transition:fly={{ y: 10, duration: 180, easing: cubicOut }}
        >
            <h2>Add Subsystem</h2>
            <label class="field-label">
                Name
                <input
                    class="field"
                    placeholder="Subsystem name"
                    bind:value={newSubsystemName}
                    on:keydown={(e) => e.key === "Enter" && addSubsystem()}
                    disabled={addingSubsystem}
                />
            </label>
            <div class="modal-actions">
                <button
                    class="btn-ghost"
                    on:click={() => (showAddSubsystem = false)}
                    disabled={addingSubsystem}
                >Cancel</button>
                <button
                    class="btn-primary"
                    on:click={addSubsystem}
                    disabled={!newSubsystemName.trim() || $readOnly || !canCreateSubsystem() || addingSubsystem}
                >
                    {#if addingSubsystem}
                        <span class="spinner spinner-sm" aria-hidden="true"></span>
                        Adding�
                    {:else}
                        Add
                    {/if}
                </button>
            </div>
        </div>
    </div>
{/if}

{#if showSeed}
    <div
        class="modal-backdrop"
        on:click={() => !seeding && (showSeed = false)}
        role="presentation"
        transition:fade={{ duration: 150 }}
    >
        <div
            class="modal"
            on:click|stopPropagation
            role="dialog"
            aria-label="Seed system"
            transition:fly={{ y: 10, duration: 180, easing: cubicOut }}
        >
            <h2>Seed System</h2>
            <label class="field-label">
                System description
                <textarea
                    class="field"
                    rows="3"
                    placeholder="One sentence describing the system boundary"
                    bind:value={seedDescription}
                    disabled={seeding}
                ></textarea>
            </label>
            <label class="field-label">
                Subsystems (comma or new line separated)
                <textarea
                    class="field"
                    rows="4"
                    placeholder="Sensors, Power, Flight Control, Ground Station"
                    bind:value={seedSubsystems}
                    disabled={seeding}
                ></textarea>
            </label>
            <div class="modal-actions">
                <button
                    class="btn-ghost"
                    on:click={() => (showSeed = false)}
                    disabled={seeding}
                >Cancel</button>
                <button
                    class="btn-primary"
                    on:click={seedSystem}
                    disabled={!seedSubsystems.trim() || seeding || $readOnly || !canCreateSubsystem()}
                >
                    {#if seeding}
                        <span class="spinner spinner-sm" aria-hidden="true"></span>
                        Seeding�
                    {:else}
                        Seed
                    {/if}
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    .system-editor {
        background: var(--surface-base);
    }

    .system-header {
        display: flex;
        align-items: center;
        gap: var(--space-4);
        padding: var(--space-4) var(--space-6) var(--space-3);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
        flex-shrink: 0;
    }

    .header-left {
        flex: 1;
        min-width: 0;
    }

    .stat-bar {
        display: flex;
        gap: var(--space-3);
        align-items: center;
    }

    .stat {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        min-width: 60px;
    }

    .stat-value {
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
    }

    .stat-label {
        font-size: 10px;
        color: var(--text-muted);
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }

    .header-actions {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        flex-shrink: 0;
    }

    .btn-divider {
        width: 1px;
        height: 18px;
        background: var(--surface-border);
        margin: 0 var(--space-1);
        flex-shrink: 0;
    }

    .btn-ghost.icon-only {
        padding: 4px 7px;
    }

    .system-body {
        display: grid;
        grid-template-columns: 1fr 260px;
        grid-template-rows: 1fr;
        height: 100%;
        overflow: hidden;
        min-height: 0;
        flex: 1;
    }

    .seed-banner {
        grid-column: 1 / -1;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-4);
        padding: var(--space-3) var(--space-5);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
    }

    .seed-title {
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
    }

    .seed-subtitle {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .canvas-area {
        position: relative;
        overflow: hidden;
        min-height: 0;
        height: 100%;
    }

    .canvas-empty {
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        color: var(--text-muted);
        font-size: var(--text-sm);
    }

    .canvas-loading {
        height: 100%;
        width: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: var(--space-3);
        color: var(--text-muted);
        font-size: var(--text-sm);
    }

    .edge-kind-select {
        background: var(--surface-overlay);
        border: 1px solid var(--accent);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-xs);
        padding: 2px var(--space-2);
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
        transition: background var(--transition-fast), transform var(--transition-fast);
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
    }
    .btn-primary:hover:not(:disabled) {
        background: var(--accent-hover);
    }
    .btn-primary:active:not(:disabled) {
        transform: scale(0.97);
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
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
    }
    .btn-ghost:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }
    .btn-ghost:active:not(:disabled) {
        transform: scale(0.97);
        background: var(--surface-border);
    }
    .btn-ghost.active {
        border-color: var(--accent);
        color: var(--accent-hover);
    }
    .btn-ghost:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .import-error {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: var(--space-2) var(--space-6);
        color: var(--color-error);
        background: #ef444420;
        border-bottom: 1px solid #ef444440;
        font-size: var(--text-xs);
        gap: var(--space-2);
    }
    .import-error-dismiss {
        background: none;
        border: none;
        color: var(--color-error);
        cursor: pointer;
        font-size: var(--text-base);
        line-height: 1;
        opacity: 0.7;
        padding: 0 var(--space-1);
        transition: opacity var(--transition-fast);
        flex-shrink: 0;
    }
    .import-error-dismiss:hover {
        opacity: 1;
    }

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

    @media (max-width: 1100px) {
        .system-body {
            grid-template-columns: 1fr;
            grid-template-rows: 1fr auto;
        }
    }
</style>

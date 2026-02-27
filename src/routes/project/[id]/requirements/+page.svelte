<script lang="ts">
    import { onMount } from "svelte";
    import { page } from "$app/stores";
    import {
        loadProject,
        nodes,
        edges,
        saveNode,
        removeNode,
        currentProject,
        readOnly,
        getRequirementHistory,
        canViewNode,
        canEditNode,
        canCreateRequirementForAllocations,
    } from "$lib/store/model";
    import { invoke } from "@tauri-apps/api/core";
    import { v4 as uuidv4 } from "uuid";
    import type {
        Node,
        RequirementData,
        DocumentSection,
        SuspectLink,
        ReviewSession,
        ReviewItem,
    } from "$lib/types";
    import { parseCsv, toCsv, downloadBlob, slugify } from "$lib/utils/csv";
    import { slide, fade } from "svelte/transition";
    import {
        ClipboardList,
        Search,
        Plus,
        Download,
        Upload,
    } from "lucide-svelte";
    import CommentThread from "$lib/CommentThread.svelte";

    $: projectId = $page.params.id;

    // -- Comment state ---------------------------------------------------------
    let commentCounts: Record<string, number> = {};
    let expandedCommentNodeId: string | null = null;

    async function loadCommentCounts() {
        const counts = await invoke<Record<string, number>>(
            "get_comment_counts",
            { projectId },
        );
        commentCounts = counts;
    }

    function toggleComments(nodeId: string) {
        expandedCommentNodeId =
            expandedCommentNodeId === nodeId ? null : nodeId;
    }

    // ── Filtering & sorting ───────────────────────────────────────────────────
    let filterText = "";
    let sortField: keyof SortableRow = "req_id";
    let sortDir: "asc" | "desc" = "asc";

    type SortableRow = {
        req_id: string;
        name: string;
        priority: string;
        status: string;
        verification_method: string;
        source: string;
        allocations: string;
        text: string;
    };

    $: requirements = $nodes.filter(
        (n) => n.kind === "requirement" && canViewNode(n),
    );

    // All subsystem blocks (non-system-root blocks)
    $: subsystems = $nodes.filter(
        (n) =>
            n.kind === "block" &&
            !(n.meta as Record<string, unknown>)?.system_root,
    );

    // ── View tabs ─────────────────────────────────────────────────────────────
    let activeView: "table" | "allocation" = "table";

    // ── Allocation matrix ─────────────────────────────────────────────────────
    // For each block, find all requirements that either:
    //   (a) have the block's name in their allocations[] field, OR
    //   (b) are connected by a satisfies edge from the block to the requirement
    $: allocationMatrix = subsystems.map((block) => {
        const byAlloc = requirements.filter((req) => {
            const d = req.data as RequirementData;
            return (d.allocations ?? []).some(
                (a) => a.trim().toLowerCase() === block.name.trim().toLowerCase(),
            );
        });
        const bySatisfies = requirements.filter((req) =>
            $edges.some(
                (e) =>
                    e.kind === "satisfies" &&
                    e.source_id === block.id &&
                    e.target_id === req.id,
            ),
        );
        const combined = [
            ...new Map(
                [...byAlloc, ...bySatisfies].map((r) => [r.id, r]),
            ).values(),
        ];
        return { block, reqs: combined };
    }).filter((row) => row.reqs.length > 0 || subsystems.length <= 12);

    $: filtered = requirements.filter((n) => {
        if (!filterText) return true;
        const q = filterText.toLowerCase();
        const d = n.data as RequirementData;
        return (
            (d.req_id ?? "").toLowerCase().includes(q) ||
            n.name.toLowerCase().includes(q) ||
            (d.text ?? "").toLowerCase().includes(q) ||
            (d.source ?? "").toLowerCase().includes(q) ||
            allocationsLabel(d).toLowerCase().includes(q)
        );
    });

    $: sorted = [...filtered].sort((a, b) => {
        const da = a.data as RequirementData;
        const db = b.data as RequirementData;
        const valA = fieldVal(a, da, sortField);
        const valB = fieldVal(b, db, sortField);
        const cmp = valA.localeCompare(valB, undefined, { numeric: true });
        return sortDir === "asc" ? cmp : -cmp;
    });

    function fieldVal(
        node: Node,
        data: RequirementData,
        field: keyof SortableRow,
    ): string {
        switch (field) {
            case "req_id":
                return data.req_id ?? "";
            case "name":
                return node.name ?? "";
            case "priority":
                return data.priority ?? "";
            case "status":
                return data.status ?? "";
            case "verification_method":
                return data.verification_method ?? "";
            case "source":
                return data.source ?? "";
            case "allocations":
                return allocationsLabel(data);
            case "text":
                return data.text ?? "";
            default:
                return "";
        }
    }

    function allocationsLabel(data: RequirementData): string {
        return (data.allocations ?? []).join(", ");
    }

    function parseAllocations(raw: string): string[] | undefined {
        const list = raw
            .split(",")
            .map((entry) => entry.trim())
            .filter(Boolean);
        return list.length ? list : undefined;
    }

    function allocationsValue(req: RequirementData | null): string {
        if (!req) return "";
        return allocationsLabel(req);
    }

    function fmtDateTime(iso: string): string {
        const d = new Date(iso);
        return d.toLocaleString(undefined, {
            month: "short",
            day: "numeric",
            year: "numeric",
            hour: "numeric",
            minute: "2-digit",
        });
    }

    type HistoryChange = { field: string; prev: string; next: string };

    function diffHistory(entry: {
        prev: Record<string, unknown>;
        next: Record<string, unknown>;
    }): HistoryChange[] {
        const fields = [
            "req_id",
            "name",
            "text",
            "rationale",
            "priority",
            "status",
            "verification_method",
            "source",
            "allocations",
            "description",
        ];
        const changes: HistoryChange[] = [];
        for (const field of fields) {
            const prev = entry.prev[field];
            const next = entry.next[field];
            const prevVal = Array.isArray(prev)
                ? prev.join(", ")
                : String(prev ?? "");
            const nextVal = Array.isArray(next)
                ? next.join(", ")
                : String(next ?? "");
            if (prevVal !== nextVal) {
                changes.push({ field, prev: prevVal, next: nextVal });
            }
        }
        return changes;
    }

    function toggleSort(field: keyof SortableRow) {
        if (sortField === field) {
            sortDir = sortDir === "asc" ? "desc" : "asc";
        } else {
            sortField = field;
            sortDir = "asc";
        }
    }

    // ── Row expansion ─────────────────────────────────────────────────────────
    let expandedId: string | null = null;
    function toggleExpand(id: string) {
        expandedId = expandedId === id ? null : id;
    }

    // ── Add requirement ───────────────────────────────────────────────────────
    let showAddForm = false;
    let newReqId = "";
    let newReqName = "";
    let importing = false;
    let importError = "";
    let importInput: HTMLInputElement | null = null;

    async function addRequirement() {
        if ($readOnly || !canCreateRequirementForAllocations()) return;
        if (!newReqName.trim()) return;
        const now = new Date().toISOString();
        const node: Node = {
            id: uuidv4(),
            project_id: projectId,
            kind: "requirement",
            name: newReqName.trim(),
            description: "",
            data: {
                kind: "requirement",
                req_id: newReqId.trim() || undefined,
                text: "",
                rationale: "",
                priority: "shall",
                status: "draft",
                allocations: [],
            },
            meta: {},
            created_at: now,
            modified_at: now,
        };
        await saveNode(node);
        newReqId = "";
        newReqName = "";
        showAddForm = false;
    }

    // ── Inline update helpers ─────────────────────────────────────────────────
    async function updateField(node: Node, field: string, value: unknown) {
        if ($readOnly || !canEditNode(node)) return;
        const updated: Node = {
            ...node,
            [field]: value,
            modified_at: new Date().toISOString(),
        };
        await saveNode(updated);
    }

    async function updateDataField(node: Node, field: string, value: unknown) {
        if ($readOnly || !canEditNode(node)) return;
        const updated: Node = {
            ...node,
            data: { ...node.data, [field]: value },
            modified_at: new Date().toISOString(),
        };
        await saveNode(updated);
    }

    async function deleteReq(nodeId: string) {
        const node = requirements.find((n) => n.id === nodeId) ?? null;
        if ($readOnly || !node || !canEditNode(node)) return;
        await removeNode(nodeId);
        if (expandedId === nodeId) expandedId = null;
    }

    // ── Subsystem allocation dropdown ─────────────────────────────────────────
    // Track which row has the dropdown open
    let allocationDropdownId: string | null = null;

    function toggleAllocationDropdown(nodeId: string) {
        allocationDropdownId = allocationDropdownId === nodeId ? null : nodeId;
    }

    function isAllocated(
        req: RequirementData | null,
        subsystemName: string,
    ): boolean {
        return (req?.allocations ?? []).some(
            (a) =>
                a.trim().toLowerCase() === subsystemName.trim().toLowerCase(),
        );
    }

    async function selectAllocation(node: Node, subsystemName: string) {
        if ($readOnly || !canEditNode(node)) return;
        const req = node.data as RequirementData;
        const key = subsystemName.trim().toLowerCase();
        const already = (req.allocations ?? []).some(
            (a) => a.trim().toLowerCase() === key,
        );
        // Single-select: clicking the current one deselects it, clicking another replaces it
        await updateDataField(
            node,
            "allocations",
            already ? undefined : [subsystemName.trim()],
        );
        allocationDropdownId = null;
    }

    async function setSystemLevel(node: Node) {
        if ($readOnly || !canEditNode(node)) return;
        await updateDataField(node, "allocations", undefined);
        allocationDropdownId = null;
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────
    // -- Suspect links ---------------------------------------------------------
    let suspectLinks: SuspectLink[] = [];
    let suspectNodeIds = new Set<string>();
    let showSuspectPanel = false;

    async function loadSuspectLinks() {
        suspectLinks = await invoke<SuspectLink[]>("get_suspect_links", {
            projectId,
        });
        suspectNodeIds = new Set(suspectLinks.map((s) => s.target_node_id));
    }

    async function resolveSuspectLink(linkId: string) {
        await invoke("resolve_suspect_link", {
            id: linkId,
            resolvedBy: "user",
        });
        await loadSuspectLinks();
    }

    // -- Review workflow -------------------------------------------------------
    let reviews: ReviewSession[] = [];
    let showReviewPanel = false;
    let creatingReview = false;
    let reviewTitle = "";
    let selectedForReview = new Set<string>(); // node IDs
    let activeReview: ReviewSession | null = null;

    async function loadReviews() {
        reviews = await invoke<ReviewSession[]>("list_review_sessions", {
            projectId,
        });
    }

    async function createReview() {
        if (!reviewTitle.trim() || selectedForReview.size === 0) return;
        creatingReview = true;
        try {
            const session = await invoke<ReviewSession>(
                "create_review_session",
                {
                    projectId,
                    title: reviewTitle.trim(),
                    description: null,
                    nodeIds: Array.from(selectedForReview),
                },
            );
            reviews = [session, ...reviews];
            activeReview = session;
            reviewTitle = "";
            selectedForReview = new Set();
            showReviewPanel = true;
        } finally {
            creatingReview = false;
        }
    }

    async function setVerdict(itemId: string, verdict: string) {
        await invoke("set_review_verdict", {
            itemId,
            verdict,
            verdictBy: "User",
            note: null,
        });
        await loadReviews();
        if (activeReview) {
            activeReview =
                reviews.find((r) => r.id === activeReview!.id) ?? null;
        }
    }

    async function closeReview(sessionId: string, status: string) {
        await invoke("close_review_session", { sessionId, status });
        await loadReviews();
        if (activeReview?.id === sessionId) activeReview = null;
    }

    function toggleSelectForReview(nodeId: string) {
        const next = new Set(selectedForReview);
        if (next.has(nodeId)) next.delete(nodeId);
        else next.add(nodeId);
        selectedForReview = next;
    }

    onMount(async () => {
        await loadProject(projectId);
        await loadSuspectLinks();
        await loadCommentCounts();
        await loadReviews();
    });

    function projectSlug(): string {
        return slugify($currentProject?.name ?? projectId) || projectId;
    }

    function exportRequirementsCsv() {
        const header = [
            "req_id",
            "name",
            "text",
            "rationale",
            "priority",
            "status",
            "verification_method",
            "source",
            "allocations",
            "description",
        ];
        const rows = requirements.map((n) => {
            const d = n.data as RequirementData;
            return [
                d.req_id ?? "",
                n.name ?? "",
                d.text ?? "",
                d.rationale ?? "",
                d.priority ?? "",
                d.status ?? "",
                d.verification_method ?? "",
                d.source ?? "",
                (d.allocations ?? []).join(", "),
                n.description ?? "",
            ];
        });
        const csv = toCsv([header, ...rows]);
        downloadBlob(
            new Blob([csv], { type: "text/csv;charset=utf-8" }),
            `requirements-${projectSlug()}.csv`,
        );
    }

    function exportRequirementsJson() {
        const payload = requirements.map((n) => {
            const d = n.data as RequirementData;
            return {
                req_id: d.req_id ?? "",
                name: n.name ?? "",
                text: d.text ?? "",
                rationale: d.rationale ?? "",
                priority: d.priority ?? "",
                status: d.status ?? "",
                verification_method: d.verification_method ?? "",
                source: d.source ?? "",
                allocations: d.allocations ?? [],
                description: n.description ?? "",
            };
        });
        const json = JSON.stringify(payload, null, 2);
        downloadBlob(
            new Blob([json], { type: "application/json" }),
            `requirements-${projectSlug()}.json`,
        );
    }

    function normalizePriority(
        raw: string | undefined,
    ): RequirementData["priority"] {
        const val = (raw ?? "").toLowerCase();
        if (val === "should" || val === "may") return val;
        return "shall";
    }

    function normalizeStatus(
        raw: string | undefined,
    ): RequirementData["status"] {
        const val = (raw ?? "").toLowerCase();
        if (val === "approved" || val === "obsolete") return val;
        return "draft";
    }

    function normalizeVerif(
        raw: string | undefined,
    ): RequirementData["verification_method"] {
        const val = (raw ?? "").toLowerCase();
        if (
            val === "analysis" ||
            val === "test" ||
            val === "inspection" ||
            val === "demonstration"
        )
            return val;
        return undefined;
    }

    function parseAllocationsList(
        raw: string | undefined,
    ): string[] | undefined {
        if (!raw) return undefined;
        const list = raw
            .split(/[;,]/)
            .map((entry) => entry.trim())
            .filter(Boolean);
        return list.length ? list : undefined;
    }

    async function importRequirementsFile(file: File) {
        if ($readOnly) return;
        importing = true;
        importError = "";
        try {
            const text = await file.text();
            const lowerName = file.name.toLowerCase();

            let rows: Record<string, string>[] = [];
            if (lowerName.endsWith(".json")) {
                const parsed = JSON.parse(text);
                const list = Array.isArray(parsed)
                    ? parsed
                    : (parsed?.requirements ?? []);
                rows = list.map((item: Record<string, unknown>) => ({
                    req_id: String(item.req_id ?? ""),
                    name: String(item.name ?? ""),
                    text: String(item.text ?? ""),
                    rationale: String(item.rationale ?? ""),
                    priority: String(item.priority ?? ""),
                    status: String(item.status ?? ""),
                    verification_method: String(item.verification_method ?? ""),
                    source: String(item.source ?? ""),
                    allocations: Array.isArray(item.allocations)
                        ? item.allocations.join(", ")
                        : String(item.allocations ?? ""),
                    description: String(item.description ?? ""),
                }));
            } else {
                const table = parseCsv(text);
                if (table.length === 0) return;
                const header = table[0].map((h) => h.trim().toLowerCase());
                const indexOf = (keys: string[]) =>
                    header.findIndex((h) => keys.includes(h));
                const idx = {
                    req_id: indexOf(["req_id", "id", "reqid"]),
                    name: indexOf(["name", "title"]),
                    text: indexOf(["text", "requirement"]),
                    rationale: indexOf(["rationale"]),
                    priority: indexOf(["priority"]),
                    status: indexOf(["status"]),
                    verification_method: indexOf([
                        "verification_method",
                        "verification",
                    ]),
                    source: indexOf(["source"]),
                    allocations: indexOf([
                        "allocations",
                        "subsystems",
                        "allocation",
                    ]),
                    description: indexOf(["description"]),
                };
                rows = table.slice(1).map((row) => ({
                    req_id: idx.req_id >= 0 ? row[idx.req_id] : "",
                    name: idx.name >= 0 ? row[idx.name] : "",
                    text: idx.text >= 0 ? row[idx.text] : "",
                    rationale: idx.rationale >= 0 ? row[idx.rationale] : "",
                    priority: idx.priority >= 0 ? row[idx.priority] : "",
                    status: idx.status >= 0 ? row[idx.status] : "",
                    verification_method:
                        idx.verification_method >= 0
                            ? row[idx.verification_method]
                            : "",
                    source: idx.source >= 0 ? row[idx.source] : "",
                    allocations:
                        idx.allocations >= 0 ? row[idx.allocations] : "",
                    description:
                        idx.description >= 0 ? row[idx.description] : "",
                }));
            }

            const existingByReqId = new Map<string, Node>();
            const existingByName = new Map<string, Node>();
            for (const n of requirements) {
                const d = n.data as RequirementData;
                if (d.req_id) existingByReqId.set(d.req_id.toLowerCase(), n);
                if (n.name) existingByName.set(n.name.toLowerCase(), n);
            }

            let skippedByAccess = 0;
            for (const row of rows) {
                const name = (row.name ?? "").trim();
                const reqId = (row.req_id ?? "").trim();
                if (!name && !reqId) continue;

                const allocations = parseAllocationsList(row.allocations);
                const data: RequirementData = {
                    kind: "requirement",
                    req_id: reqId || undefined,
                    text: (row.text ?? "").trim(),
                    rationale: (row.rationale ?? "").trim(),
                    priority: normalizePriority(row.priority),
                    status: normalizeStatus(row.status),
                    verification_method: normalizeVerif(
                        row.verification_method,
                    ),
                    source: (row.source ?? "").trim() || undefined,
                    allocations,
                };

                const match =
                    (reqId && existingByReqId.get(reqId.toLowerCase())) ||
                    (name && existingByName.get(name.toLowerCase()));

                if (match) {
                    if (!canEditNode(match)) {
                        skippedByAccess += 1;
                        continue;
                    }
                    await saveNode({
                        ...match,
                        name: name || match.name,
                        description: (row.description ?? "").trim(),
                        data,
                        modified_at: new Date().toISOString(),
                    });
                } else {
                    if (!canCreateRequirementForAllocations(allocations)) {
                        skippedByAccess += 1;
                        continue;
                    }
                    const now = new Date().toISOString();
                    const node: Node = {
                        id: uuidv4(),
                        project_id: projectId,
                        kind: "requirement",
                        name: name || reqId || "Requirement",
                        description: (row.description ?? "").trim(),
                        data,
                        meta: {},
                        created_at: now,
                        modified_at: now,
                    };
                    await saveNode(node);
                }
            }
            if (skippedByAccess > 0) {
                importError = `Skipped ${skippedByAccess} row${skippedByAccess === 1 ? "" : "s"} due to access scope.`;
            }
        } catch (err) {
            importError = String(err);
        } finally {
            importing = false;
            if (importInput) importInput.value = "";
        }
    }

    function onRequirementsFileChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement | null;
        const file = input?.files?.[0];
        if (file) importRequirementsFile(file);
    }

    function reqData(node: { data: unknown }): RequirementData {
        return node.data as RequirementData;
    }

    function countStatus(status: RequirementData["status"]): number {
        return requirements.filter(
            (n) => (n.data as RequirementData).status === status,
        ).length;
    }

    // ── Source section badges ──────────────────────────────────────────────────
    // derives edges: source_id = document_section id, target_id = requirement node id
    $: derivesByReqId = $edges.reduce((acc, e) => {
        if (e.kind === "derives") {
            const list = acc.get(e.target_id) ?? [];
            list.push(e.source_id);
            acc.set(e.target_id, list);
        }
        return acc;
    }, new Map<string, string[]>());

    // Cache of fetched DocumentSections by id
    let sectionCache = new Map<string, DocumentSection>();

    // When a row expands and has derives edges, fetch linked sections if not cached
    $: if (expandedId) {
        const sectionIds = derivesByReqId.get(expandedId) ?? [];
        const missing = sectionIds.filter((id) => !sectionCache.has(id));
        if (missing.length > 0) {
            invoke<DocumentSection[]>("list_project_document_sections", {
                projectId,
            })
                .catch(() => [] as DocumentSection[])
                .then((secs) => {
                    for (const s of secs) sectionCache.set(s.id, s);
                    sectionCache = new Map(sectionCache);
                });
        }
    }

    const PRIORITY_OPTIONS = ["shall", "should", "may"] as const;
    const STATUS_OPTIONS = ["draft", "approved", "obsolete"] as const;
    const VERIF_OPTIONS = [
        "",
        "analysis",
        "test",
        "inspection",
        "demonstration",
    ] as const;
</script>

<svelte:window on:click={() => (allocationDropdownId = null)} />

<div class="req-page page-frame">
    <!-- Header -->
    <header class="req-header page-header">
        <div class="header-left">
            <div class="page-eyebrow">Requirements</div>
            <h1 class="page-title">Requirements</h1>
            <p class="page-subtitle">
                Capture, allocate, and track system requirements.
            </p>
        </div>
        <div class="stat-bar">
            <div class="stat">
                <div class="stat-value">{requirements.length}</div>
                <div class="stat-label">Total</div>
            </div>
            <div class="stat">
                <div class="stat-value">{countStatus("approved")}</div>
                <div class="stat-label">Approved</div>
            </div>
            <div class="stat">
                <div class="stat-value">{countStatus("draft")}</div>
                <div class="stat-label">Draft</div>
            </div>
        </div>
        <div class="header-actions">
            <input
                class="search"
                type="search"
                placeholder="Search..."
                bind:value={filterText}
            />
            <button class="btn-ghost" on:click={exportRequirementsCsv}
                ><Download size={13} /> CSV</button
            >
            <button class="btn-ghost" on:click={exportRequirementsJson}
                ><Download size={13} /> JSON</button
            >
            <button
                class="btn-ghost"
                on:click={() => importInput?.click()}
                disabled={importing || $readOnly}
            >
                <Upload size={13} />
                {importing ? "Importing..." : "Import"}
            </button>
            <button
                class="btn-review"
                on:click={() => (showReviewPanel = !showReviewPanel)}
            >
                &#x1F4CB; Reviews {#if reviews.filter((r) => r.status === "open" || r.status === "in_progress").length > 0}
                    <span class="review-badge"
                        >{reviews.filter(
                            (r) =>
                                r.status === "open" ||
                                r.status === "in_progress",
                        ).length}</span
                    >
                {/if}
            </button>
            <button
                class="btn-primary"
                on:click={() => (showAddForm = !showAddForm)}
                disabled={$readOnly || !canCreateRequirementForAllocations()}
            >
                <Plus size={13} /> Add Requirement
            </button>
        </div>
    </header>

    <input
        bind:this={importInput}
        type="file"
        accept=".csv,.json"
        on:change={onRequirementsFileChange}
        style="display:none"
    />

    {#if importError}
        <div class="import-error">Import failed: {importError}</div>
    {/if}

    <!-- Suspect links banner -->
    {#if suspectLinks.length > 0}
        <div class="suspect-banner">
            <span
                >&#9888; {suspectLinks.length} suspect link{suspectLinks.length ===
                1
                    ? ""
                    : "s"} - upstream requirements have changed</span
            >
            <button on:click={() => (showSuspectPanel = !showSuspectPanel)}
                >Review</button
            >
        </div>
    {/if}

    <!-- Suspect links panel -->
    {#if showSuspectPanel && suspectLinks.length > 0}
        <div class="suspect-panel">
            <h3>&#9888; Suspect Links</h3>
            {#each suspectLinks as link (link.id)}
                <div class="suspect-row">
                    <div class="suspect-info">
                        <strong>&#9888;</strong> Upstream requirement changed: {link.flagged_reason}
                        <span class="suspect-date"
                            >{new Date(
                                link.flagged_at,
                            ).toLocaleDateString()}</span
                        >
                    </div>
                    <button
                        class="resolve-btn"
                        on:click={() => resolveSuspectLink(link.id)}
                        >Mark Reviewed</button
                    >
                </div>
            {/each}
        </div>
    {/if}

    <!-- Review panel -->
    {#if showReviewPanel}
        <div class="review-panel">
            <div class="review-panel-header">
                <h3>Review Sessions</h3>
                <button
                    class="panel-close"
                    on:click={() => (showReviewPanel = false)}>&#x2715;</button
                >
            </div>

            <!-- Create new review -->
            <div class="review-create-section">
                <h4>Start New Review</h4>
                <p class="review-help">
                    Select requirements from the table below, then create a
                    review session.
                </p>
                <div class="review-create-row">
                    <input
                        bind:value={reviewTitle}
                        placeholder="Review title (e.g. Sprint 3 Baseline)"
                        class="review-title-input"
                    />
                    <button
                        class="btn-primary"
                        on:click={createReview}
                        disabled={creatingReview ||
                            !reviewTitle.trim() ||
                            selectedForReview.size === 0}
                    >
                        {creatingReview
                            ? "Creating..."
                            : `Create Review (${selectedForReview.size} selected)`}
                    </button>
                </div>
            </div>

            <!-- Active and past reviews -->
            {#if reviews.length > 0}
                <div class="review-list">
                    {#each reviews as session (session.id)}
                        <div
                            class="review-session-card"
                            class:active={activeReview?.id === session.id}
                        >
                            <div
                                class="review-session-header"
                                on:click={() =>
                                    (activeReview =
                                        activeReview?.id === session.id
                                            ? null
                                            : session)}
                                role="button"
                                tabindex="0"
                                on:keydown={(e) =>
                                    e.key === "Enter" &&
                                    (activeReview =
                                        activeReview?.id === session.id
                                            ? null
                                            : session)}
                            >
                                <div class="review-session-title">
                                    <span
                                        class="review-status-dot status-{session.status}"
                                    ></span>
                                    <strong>{session.title}</strong>
                                </div>
                                <div class="review-session-meta">
                                    <span class="review-status-label"
                                        >{session.status.replace(
                                            "_",
                                            " ",
                                        )}</span
                                    >
                                    <span class="review-item-count"
                                        >{session.items.length} reqs</span
                                    >
                                    <span class="review-date"
                                        >{new Date(
                                            session.created_at,
                                        ).toLocaleDateString()}</span
                                    >
                                </div>
                            </div>

                            {#if activeReview?.id === session.id}
                                <div class="review-items">
                                    {#each session.items as item (item.id)}
                                        <div class="review-item">
                                            <span class="review-item-id"
                                                >{item.node_id.slice(
                                                    0,
                                                    8,
                                                )}&#x2026;</span
                                            >
                                            <div class="review-item-verdicts">
                                                <button
                                                    class="verdict-btn approved"
                                                    class:active={item.verdict ===
                                                        "approved"}
                                                    on:click={() =>
                                                        setVerdict(
                                                            item.id,
                                                            "approved",
                                                        )}
                                                    >&#x2713; Approve</button
                                                >
                                                <button
                                                    class="verdict-btn needs_changes"
                                                    class:active={item.verdict ===
                                                        "needs_changes"}
                                                    on:click={() =>
                                                        setVerdict(
                                                            item.id,
                                                            "needs_changes",
                                                        )}
                                                    >&#x26A0; Changes</button
                                                >
                                                <button
                                                    class="verdict-btn rejected"
                                                    class:active={item.verdict ===
                                                        "rejected"}
                                                    on:click={() =>
                                                        setVerdict(
                                                            item.id,
                                                            "rejected",
                                                        )}
                                                    >&#x2715; Reject</button
                                                >
                                            </div>
                                        </div>
                                    {/each}

                                    {#if session.status === "open" || session.status === "in_progress"}
                                        <div class="review-close-row">
                                            <button
                                                class="btn-approve-all"
                                                on:click={() =>
                                                    closeReview(
                                                        session.id,
                                                        "approved",
                                                    )}
                                                >Approve All &amp; Close</button
                                            >
                                            <button
                                                class="btn-reject-all"
                                                on:click={() =>
                                                    closeReview(
                                                        session.id,
                                                        "rejected",
                                                    )}
                                                >Reject &amp; Close</button
                                            >
                                            <button
                                                class="btn-close-review"
                                                on:click={() =>
                                                    closeReview(
                                                        session.id,
                                                        "closed",
                                                    )}
                                                >Close Without Decision</button
                                            >
                                        </div>
                                    {/if}
                                </div>
                            {/if}
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    {/if}

    <!-- Inline add form -->
    {#if showAddForm}
        <div class="add-form" transition:slide={{ duration: 180, axis: "y" }}>
            <input
                class="add-input"
                placeholder="REQ-001"
                bind:value={newReqId}
                disabled={$readOnly || !canCreateRequirementForAllocations()}
                style="width: 120px; font-family: var(--font-mono); font-size: var(--text-xs);"
            />
            <input
                class="add-input"
                placeholder="Requirement name (required)"
                bind:value={newReqName}
                disabled={$readOnly || !canCreateRequirementForAllocations()}
                style="flex: 1;"
                on:keydown={(e) => e.key === "Enter" && addRequirement()}
            />
            <button
                class="btn-confirm"
                on:click={addRequirement}
                disabled={!newReqName.trim() ||
                    $readOnly ||
                    !canCreateRequirementForAllocations()}>Add</button
            >
            <button class="btn-cancel" on:click={() => (showAddForm = false)}
                >Cancel</button
            >
        </div>
    {/if}

    <!-- View tab bar -->
    <div class="view-tabs">
        <button
            class="view-tab"
            class:active={activeView === "table"}
            on:click={() => (activeView = "table")}
        >Requirements</button>
        <button
            class="view-tab"
            class:active={activeView === "allocation"}
            on:click={() => (activeView = "allocation")}
        >Allocation Matrix</button>
    </div>

    {#if activeView === "allocation"}
        <!-- Allocation matrix -->
        <div class="alloc-wrap">
            {#if subsystems.length === 0}
                <div class="alloc-empty">
                    No subsystem blocks found. Add <strong>Block</strong> nodes in the System diagram to see the allocation matrix.
                </div>
            {:else}
                <table class="alloc-table">
                    <thead>
                        <tr>
                            <th class="alloc-req-col">Requirement</th>
                            {#each subsystems as block}
                                <th class="alloc-block-col" title={block.name}>
                                    <span class="alloc-block-name">{block.name}</span>
                                </th>
                            {/each}
                        </tr>
                    </thead>
                    <tbody>
                        {#each requirements as req (req.id)}
                            {@const d = reqData(req)}
                            <tr>
                                <td class="alloc-req-cell">
                                    <span class="alloc-req-id">{d.req_id ?? "—"}</span>
                                    <span class="alloc-req-name">{req.name}</span>
                                </td>
                                {#each subsystems as block}
                                    {@const byAlloc = (d.allocations ?? []).some(
                                        (a) => a.trim().toLowerCase() === block.name.trim().toLowerCase()
                                    )}
                                    {@const bySatisfies = $edges.some(
                                        (e) => e.kind === "satisfies" && e.source_id === block.id && e.target_id === req.id
                                    )}
                                    {@const allocated = byAlloc || bySatisfies}
                                    <td
                                        class="alloc-cell"
                                        class:allocated
                                        title={allocated
                                            ? `${req.name} → ${block.name}${byAlloc && bySatisfies ? " (alloc + satisfies)" : byAlloc ? " (allocated)" : " (satisfies edge)"}`
                                            : ""}
                                    >
                                        {#if allocated}
                                            <span class="alloc-dot" aria-label="allocated">
                                                {bySatisfies ? "●" : "◌"}
                                            </span>
                                        {/if}
                                    </td>
                                {/each}
                            </tr>
                        {/each}
                    </tbody>
                </table>
                <div class="alloc-legend">
                    <span class="legend-item"><span class="legend-dot">●</span> satisfies edge</span>
                    <span class="legend-item"><span class="legend-dot">◌</span> allocation field</span>
                </div>
            {/if}
        </div>
    {:else}

    <!-- Table -->
    <div class="table-wrap">
        <table class="req-table">
            <thead>
                <tr>
                    {#each [{ field: "req_id", label: "ID" }, { field: "name", label: "Name" }, { field: "priority", label: "Priority" }, { field: "status", label: "Status" }, { field: "verification_method", label: "Verification" }, { field: "source", label: "Source" }, { field: "allocations", label: "Subsystems" }, { field: "text", label: "Text" }] as col}
                        <th>
                            <button
                                class="sort-btn"
                                on:click={() => toggleSort(col.field)}
                            >
                                {col.label}
                                {#if sortField === col.field}
                                    <span class="sort-indicator"
                                        >{sortDir === "asc" ? "^" : "v"}</span
                                    >
                                {/if}
                            </button>
                        </th>
                    {/each}
                    <th class="col-delete"></th>
                </tr>
            </thead>

            <tbody>
                {#if sorted.length === 0}
                    <tr>
                        <td colspan="9" class="empty-cell">
                            <div class="empty-state req-empty-state">
                                <div class="empty-state-icon">
                                    {#if filterText}
                                        <Search size={28} />
                                    {:else}
                                        <ClipboardList size={28} />
                                    {/if}
                                </div>
                                <div class="empty-state-title">
                                    {#if filterText}
                                        No matching requirements
                                    {:else}
                                        No requirements yet
                                    {/if}
                                </div>
                                <div class="empty-state-body">
                                    {#if filterText}
                                        Adjust your filter to see more results.
                                    {:else}
                                        Create the first requirement to start
                                        allocation and coverage tracking.
                                    {/if}
                                </div>
                            </div>
                        </td>
                    </tr>
                {/if}

                {#each sorted as node (node.id + '|' + node.modified_at)}
                    {@const d = node.data}
                    {@const req = d.kind === "requirement" ? d : null}

                    <tr
                        class="data-row"
                        class:expanded={expandedId === node.id}
                    >
                        <!-- ID -->
                        <td class="cell-id">
                            <input
                                class="cell-input mono"
                                value={req?.req_id ?? ""}
                                placeholder="REQ-???"
                                disabled={$readOnly || !canEditNode(node)}
                                on:change={(e) =>
                                    updateDataField(
                                        node,
                                        "req_id",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </td>

                        <!-- Name (click to expand) -->
                        <td class="cell-name">
                            <button
                                class="name-btn"
                                on:click={() => toggleExpand(node.id)}
                            >
                                {node.name}
                            </button>
                            {#if suspectNodeIds.has(node.id)}
                                <span
                                    class="suspect-badge"
                                    title="This requirement may need review - a source requirement was updated"
                                    >&#9888; Suspect</span
                                >
                            {/if}
                        </td>

                        <!-- Priority -->
                        <td>
                            <select
                                class="cell-select priority-{req?.priority ??
                                    'shall'}"
                                value={req?.priority ?? "shall"}
                                disabled={$readOnly || !canEditNode(node)}
                                on:change={(e) =>
                                    updateDataField(
                                        node,
                                        "priority",
                                        e.currentTarget.value,
                                    )}
                            >
                                {#each PRIORITY_OPTIONS as p}
                                    <option value={p}>{p}</option>
                                {/each}
                            </select>
                        </td>

                        <!-- Status -->
                        <td>
                            <select
                                class="cell-select status-{req?.status ??
                                    'draft'}"
                                value={req?.status ?? "draft"}
                                disabled={$readOnly || !canEditNode(node)}
                                on:change={(e) =>
                                    updateDataField(
                                        node,
                                        "status",
                                        e.currentTarget.value,
                                    )}
                            >
                                {#each STATUS_OPTIONS as s}
                                    <option value={s}>{s}</option>
                                {/each}
                            </select>
                        </td>

                        <!-- Verification -->
                        <td>
                            <select
                                class="cell-select"
                                value={req?.verification_method ?? ""}
                                disabled={$readOnly || !canEditNode(node)}
                                on:change={(e) =>
                                    updateDataField(
                                        node,
                                        "verification_method",
                                        e.currentTarget.value || undefined,
                                    )}
                            >
                                {#each VERIF_OPTIONS as v}
                                    <option value={v}>{v || "-- none --"}</option>
                                {/each}
                            </select>
                        </td>

                        <!-- Source -->
                        <td>
                            <input
                                class="cell-input"
                                value={req?.source ?? ""}
                                placeholder="--"
                                disabled={$readOnly || !canEditNode(node)}
                                on:change={(e) =>
                                    updateDataField(
                                        node,
                                        "source",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </td>

                        <!-- Subsystems allocation dropdown -->
                        <td class="cell-alloc">
                            <div class="alloc-wrap">
                                <button
                                    class="alloc-btn"
                                    class:alloc-has-value={req &&
                                        (req.allocations ?? []).length > 0}
                                    on:click|stopPropagation={() =>
                                        toggleAllocationDropdown(node.id)}
                                    disabled={$readOnly || !canEditNode(node)}
                                    title="Assign to subsystems"
                                >
                                    {#if req && (req.allocations ?? []).length > 0}
                                        {(req.allocations ?? []).join(", ")}
                                    {:else}
                                        <span class="alloc-placeholder"
                                            >System Level</span
                                        >
                                    {/if}
                                    <span class="alloc-chevron">v</span>
                                </button>

                                {#if allocationDropdownId === node.id}
                                    <div
                                        class="alloc-dropdown"
                                        role="dialog"
                                        aria-label="Allocate requirement"
                                        transition:slide={{
                                            duration: 150,
                                            axis: "y",
                                        }}
                                    >
                                        <div class="alloc-dropdown-header">
                                            Allocate to subsystem
                                        </div>

                                        <!-- System level option -->
                                        <button
                                            class="alloc-option"
                                            class:alloc-option-active={!req ||
                                                (req.allocations ?? [])
                                                    .length === 0}
                                            on:click|stopPropagation={() =>
                                                setSystemLevel(node)}
                                        >
                                            <span class="alloc-check"
                                                >{!req ||
                                                (req.allocations ?? [])
                                                    .length === 0
                                                    ? "[x]"
                                                    : ""}</span
                                            >
                                            System Level (unallocated)
                                        </button>

                                        {#if subsystems.length === 0}
                                            <div class="alloc-none">
                                                No subsystems defined yet.
                                            </div>
                                        {:else}
                                            {#each subsystems as sub (sub.id)}
                                                {@const isOwned = isAllocated(
                                                    req,
                                                    sub.name,
                                                )}
                                                {@const ownedByOther =
                                                    !isOwned &&
                                                    (req?.allocations ?? [])
                                                        .length > 0}
                                                <button
                                                    class="alloc-option"
                                                    class:alloc-option-active={isOwned}
                                                    class:alloc-option-disabled={ownedByOther}
                                                    disabled={ownedByOther}
                                                    on:click|stopPropagation={() =>
                                                        !ownedByOther &&
                                                        selectAllocation(
                                                            node,
                                                            sub.name,
                                                        )}
                                                    title={ownedByOther
                                                        ? `Already allocated to ${(req?.allocations ?? []).join(", ")}`
                                                        : ""}
                                                >
                                                    <span class="alloc-check"
                                                        >{isOwned
                                                            ? "[x]"
                                                            : ""}</span
                                                    >
                                                    {sub.name}
                                                    {#if ownedByOther}<span
                                                            class="alloc-taken"
                                                            >taken</span
                                                        >{/if}
                                                </button>
                                            {/each}
                                        {/if}

                                        <div class="alloc-dropdown-footer">
                                            <button
                                                class="alloc-close"
                                                on:click|stopPropagation={() =>
                                                    (allocationDropdownId =
                                                        null)}>Done</button
                                            >
                                        </div>
                                    </div>
                                {/if}
                            </div>
                        </td>

                        <!-- Text preview -->
                        <td class="cell-text">
                            <span class="text-preview" title={req?.text ?? ""}>
                                {(req?.text ?? "").slice(0, 60)}{(
                                    req?.text ?? ""
                                ).length > 60
                                    ? "--"
                                    : ""}
                            </span>
                        </td>

                        <!-- Delete -->
                        <td class="col-delete">
                            <button
                                class="comment-count-btn"
                                on:click|stopPropagation={() =>
                                    toggleComments(node.id)}
                                title="Toggle comments"
                            >
                                &#x1F4AC; {commentCounts[node.id] ?? 0}
                            </button>
                            <button
                                class="btn-row-delete"
                                on:click={() => deleteReq(node.id)}
                                title="Delete requirement"
                                disabled={$readOnly || !canEditNode(node)}
                            >
                                ?
                            </button>
                        </td>
                    </tr>

                    <!-- Comment thread row -->
                    {#if expandedCommentNodeId === node.id}
                        <tr
                            class="comment-expand-row"
                            transition:slide={{ duration: 180, axis: "y" }}
                        >
                            <td colspan="9">
                                <div class="comment-expand-area">
                                    <CommentThread
                                        nodeId={node.id}
                                        {projectId}
                                        on:changed={loadCommentCounts}
                                    />
                                </div>
                            </td>
                        </tr>
                    {/if}

                    <!-- Expanded detail row -->
                    {#if expandedId === node.id}
                        <tr
                            class="detail-row"
                            transition:slide={{ duration: 180, axis: "y" }}
                        >
                            <td colspan="9">
                                <div class="detail-grid">
                                    <div class="detail-field">
                                        <label class="detail-label"
                                            >Full text</label
                                        >
                                        <textarea
                                            class="detail-textarea"
                                            rows="4"
                                            value={req?.text ?? ""}
                                            disabled={$readOnly ||
                                                !canEditNode(node)}
                                            on:change={(e) =>
                                                updateDataField(
                                                    node,
                                                    "text",
                                                    e.currentTarget.value,
                                                )}
                                        ></textarea>
                                    </div>
                                    <div class="detail-field">
                                        <label class="detail-label"
                                            >Rationale</label
                                        >
                                        <textarea
                                            class="detail-textarea"
                                            rows="4"
                                            value={req?.rationale ?? ""}
                                            disabled={$readOnly ||
                                                !canEditNode(node)}
                                            on:change={(e) =>
                                                updateDataField(
                                                    node,
                                                    "rationale",
                                                    e.currentTarget.value,
                                                )}
                                        ></textarea>
                                    </div>
                                    <div class="detail-field">
                                        <label class="detail-label"
                                            >Description</label
                                        >
                                        <textarea
                                            class="detail-textarea"
                                            rows="4"
                                            value={node.description ?? ""}
                                            disabled={$readOnly ||
                                                !canEditNode(node)}
                                            on:change={(e) =>
                                                updateField(
                                                    node,
                                                    "description",
                                                    e.currentTarget.value,
                                                )}
                                        ></textarea>
                                    </div>
                                </div>

                                {#if (derivesByReqId.get(node.id) ?? []).length > 0}
                                    {@const secIds =
                                        derivesByReqId.get(node.id) ?? []}
                                    <div class="source-sections">
                                        <div class="source-sections-title">
                                            Derived from document sections
                                        </div>
                                        <div class="source-badges">
                                            {#each secIds as sid (sid)}
                                                {@const sec =
                                                    sectionCache.get(sid)}
                                                {#if sec}
                                                    <span
                                                        class="source-badge type-{sec.section_type}"
                                                    >
                                                        {#if sec.section_ref}
                                                            <span
                                                                class="source-badge-ref"
                                                                >{sec.section_ref}</span
                                                            >
                                                        {/if}
                                                        <span
                                                            class="source-badge-title"
                                                            >{sec.title.slice(
                                                                0,
                                                                60,
                                                            )}{sec.title
                                                                .length > 60
                                                                ? "--"
                                                                : ""}</span
                                                        >
                                                        <span
                                                            class="source-badge-type"
                                                            >{sec.section_type.replace(
                                                                "_",
                                                                " ",
                                                            )}</span
                                                        >
                                                    </span>
                                                {:else}
                                                    <span
                                                        class="source-badge loading"
                                                        >Loading...</span
                                                    >
                                                {/if}
                                            {/each}
                                        </div>
                                    </div>
                                {/if}

                                {#if getRequirementHistory(node.id).length > 0}
                                    <div class="history">
                                        <div class="history-title">History</div>
                                        {#each getRequirementHistory(node.id).slice(0, 5) as h (h.id)}
                                            <div class="history-entry">
                                                <div class="history-meta">
                                                    {fmtDateTime(h.ts)}
                                                    {#if h.source}
                                                        <span>
                                                            · {h.source}</span
                                                        >
                                                    {/if}
                                                    {#if h.actor}
                                                        <span>
                                                            · {h.actor}</span
                                                        >
                                                    {/if}
                                                </div>
                                                <div class="history-changes">
                                                    {#each diffHistory(h) as c (c.field)}
                                                        <div
                                                            class="history-line"
                                                        >
                                                            <span
                                                                class="history-field"
                                                                >{c.field}</span
                                                            >
                                                            <span
                                                                class="history-prev"
                                                                >{c.prev ||
                                                                    "--"}</span
                                                            >
                                                            <span
                                                                class="history-arrow"
                                                                >-></span
                                                            >
                                                            <span
                                                                class="history-next"
                                                                >{c.next ||
                                                                    "--"}</span
                                                            >
                                                        </div>
                                                    {/each}
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                {/if}
                            </td>
                        </tr>
                    {/if}
                {/each}
            </tbody>
        </table>
    </div>

    <div class="status-bar">
        {sorted.length} of {requirements.length} requirement{requirements.length !==
        1
            ? "s"
            : ""}
        {filterText ? ` matching "${filterText}"` : ""}
    </div>

    {/if}<!-- end activeView === "allocation" else -->
</div>

<style>
    .req-page {
        height: 100%;
        overflow: hidden;
        background: var(--surface-base);
    }

    /* ── Header ── */
    .req-header {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        flex-shrink: 0;
    }

    .header-left {
        flex: 0 1 360px;
        min-width: 220px;
    }

    .stat-bar {
        display: flex;
        gap: 6px;
        align-items: center;
        flex-wrap: nowrap;
    }

    .stat {
        display: flex;
        flex-direction: column;
        align-items: flex-start;
        justify-content: center;
        min-width: 64px;
        padding: 4px 8px;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        background: var(--surface-overlay);
    }

    .stat-value {
        font-size: var(--text-base);
        font-weight: var(--weight-bold);
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
        gap: 6px;
        flex-wrap: wrap;
        justify-content: flex-end;
        margin-left: auto;
        flex-shrink: 0;
    }

    .search {
        width: 190px;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-xs);
        padding: 7px 10px;
        font-family: var(--font-sans);
        transition:
            border-color var(--transition-fast),
            box-shadow var(--transition-fast);
    }
    .search:focus {
        outline: none;
        border-color: var(--accent-border);
        box-shadow: 0 0 0 3px var(--accent-glow);
    }

    .req-header .btn-ghost,
    .req-header .btn-primary,
    .req-header .btn-review {
        padding: 6px 10px;
        font-size: var(--text-xs);
    }

    @media (max-width: 1100px) {
        .req-header {
            flex-wrap: wrap;
        }
        .stat-bar {
            flex-wrap: wrap;
        }
        .search {
            width: 100%;
        }
    }

    .btn-ghost {
        padding: var(--space-1) var(--space-3);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        cursor: pointer;
        transition: all var(--transition-fast);
        white-space: nowrap;
    }
    .btn-ghost:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }
    .btn-ghost:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    /* ── Add form ── */
    .add-form {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-2) var(--space-5);
        background: var(--surface-overlay);
        border-bottom: 1px solid var(--surface-border);
        flex-shrink: 0;
    }

    .add-input {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-sm);
        font-family: var(--font-sans);
        padding: var(--space-1) var(--space-2);
        transition: border-color var(--transition-fast);
    }
    .add-input:focus {
        outline: none;
        border-color: var(--accent);
    }

    .btn-confirm {
        padding: var(--space-1) var(--space-3);
        background: var(--accent);
        border: none;
        border-radius: var(--radius-md);
        color: white;
        font-size: var(--text-sm);
        cursor: pointer;
        transition: background var(--transition-fast);
    }
    .btn-confirm:hover:not(:disabled) {
        background: var(--accent-hover);
    }
    .btn-confirm:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .btn-cancel {
        padding: var(--space-1) var(--space-3);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-cancel:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }

    /* ── Table ── */
    .table-wrap {
        flex: 1;
        overflow: auto;
        min-height: 0;
        padding: var(--space-4) var(--space-5) var(--space-5);
        background:
            radial-gradient(
                circle at 85% -15%,
                var(--accent-glow) 0%,
                transparent 34%
            ),
            var(--surface-base);
    }

    .import-error {
        padding: var(--space-2) var(--space-5);
        color: var(--color-error);
        background: #ef444420;
        border-bottom: 1px solid #ef444440;
        font-size: var(--text-xs);
    }

    .req-table {
        width: 100%;
        min-width: 1180px;
        border-collapse: separate;
        border-spacing: 0;
        font-size: var(--text-sm);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-xl);
        overflow: hidden;
        background: var(--surface-raised);
        box-shadow: var(--shadow-sm);
    }

    thead {
        position: sticky;
        top: 0;
        z-index: 10;
        background: var(--surface-overlay);
    }

    th {
        text-align: left;
        padding: 0;
        border-bottom: 1px solid var(--surface-border);
        border-right: 1px solid var(--surface-border-subtle);
        white-space: nowrap;
    }
    th:last-child {
        border-right: none;
    }

    .sort-btn {
        display: flex;
        align-items: center;
        gap: var(--space-1);
        width: 100%;
        padding: var(--space-3) var(--space-3);
        background: linear-gradient(
            180deg,
            var(--surface-overlay) 0%,
            var(--surface-raised) 90%
        );
        border: none;
        color: var(--text-secondary);
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        cursor: pointer;
        transition:
            color var(--transition-fast),
            background var(--transition-fast);
        white-space: nowrap;
    }
    .sort-btn:hover {
        color: var(--text-primary);
        background: var(--surface-hover);
    }

    .sort-indicator {
        color: var(--accent-hover);
    }

    td {
        padding: var(--space-2) var(--space-2);
        border-bottom: 1px solid var(--surface-border);
        border-right: 1px solid var(--surface-border-subtle);
        vertical-align: middle;
    }
    td:last-child {
        border-right: none;
    }
    tbody tr:last-child td {
        border-bottom: none;
    }

    .data-row:hover td {
        background: #ffffff05;
    }
    .data-row.expanded td {
        background: var(--surface-overlay);
    }

    /* ── Cell inputs ── */
    .cell-input {
        width: 100%;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border-subtle);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-xs);
        font-family: var(--font-sans);
        font-weight: var(--weight-medium);
        padding: 6px var(--space-2);
        transition:
            border-color var(--transition-fast),
            box-shadow var(--transition-fast),
            background-color var(--transition-fast);
    }
    .cell-input:hover {
        border-color: var(--surface-border-bright);
    }
    .cell-input:focus {
        outline: none;
        border-color: var(--accent-border);
        box-shadow: 0 0 0 3px var(--accent-glow);
        background: var(--surface-raised);
    }
    .cell-input.mono {
        font-family: var(--font-mono);
        font-size: var(--text-xs);
    }
    .cell-input:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .cell-select {
        width: 100%;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border-subtle);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-xs);
        font-family: var(--font-sans);
        font-weight: var(--weight-medium);
        padding: 6px var(--space-2);
        cursor: pointer;
        transition:
            border-color var(--transition-fast),
            box-shadow var(--transition-fast);
    }
    .cell-select:hover {
        border-color: var(--surface-border-bright);
    }
    .cell-select:focus {
        outline: none;
        border-color: var(--accent-border);
        box-shadow: 0 0 0 3px var(--accent-glow);
    }
    .cell-select:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    /* Priority color coding */
    .priority-shall {
        color: var(--color-error);
        background: var(--color-error-bg);
        border-color: #ef444450;
    }
    .priority-should {
        color: var(--color-warning);
        background: var(--color-warning-bg);
        border-color: #f59e0b50;
    }
    .priority-may {
        color: var(--text-secondary);
        background: var(--surface-overlay);
    }

    /* Status color coding */
    .status-draft {
        color: var(--text-muted);
        background: var(--surface-overlay);
    }
    .status-approved {
        color: var(--color-success);
        background: var(--color-success-bg);
        border-color: #22c55e50;
    }
    .status-obsolete {
        color: var(--color-error);
        background: var(--color-error-bg);
        border-color: #ef444450;
    }

    .cell-id {
        width: 110px;
    }
    .cell-name {
        min-width: 220px;
    }
    .cell-text {
        min-width: 300px;
        max-width: 420px;
    }

    .name-btn {
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border-subtle);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        font-family: var(--font-sans);
        cursor: pointer;
        text-align: left;
        width: 100%;
        padding: 6px var(--space-2);
        text-decoration: underline;
        text-decoration-color: transparent;
        transition:
            text-decoration-color var(--transition-fast),
            border-color var(--transition-fast),
            background-color var(--transition-fast);
    }
    .name-btn:hover {
        text-decoration-color: var(--accent);
        color: var(--accent-hover);
        border-color: var(--accent-border);
    }

    .text-preview {
        color: var(--text-secondary);
        font-size: var(--text-xs);
    }

    .col-delete {
        width: 92px;
        text-align: center;
    }

    .btn-row-delete {
        background: var(--surface-overlay);
        border: 1px solid transparent;
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        padding: 4px 8px;
        border-radius: var(--radius-md);
        transition: all var(--transition-fast);
        opacity: 0;
    }
    .data-row:hover .btn-row-delete {
        opacity: 1;
    }
    .btn-row-delete:hover {
        color: var(--color-error);
        background: #ef444420;
        border-color: #ef444450;
    }
    .btn-row-delete:disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }

    .empty-cell {
        text-align: center;
        padding: var(--space-10) !important;
        border-right: none;
        background: transparent !important;
    }

    .req-empty-state {
        max-width: 540px;
        min-height: 280px;
        margin: 0 auto;
        justify-content: center;
    }

    /* ── Detail row ── */
    .detail-row td {
        padding: var(--space-3) var(--space-4);
        background: var(--surface-overlay);
        border-bottom: 2px solid var(--accent);
    }

    .detail-grid {
        display: grid;
        grid-template-columns: 1fr 1fr 1fr;
        gap: var(--space-4);
    }

    .detail-field {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
    }

    .detail-label {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.06em;
        color: var(--text-muted);
    }

    .detail-textarea {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-sm);
        font-family: var(--font-sans);
        padding: var(--space-2);
        resize: vertical;
        transition: border-color var(--transition-fast);
    }
    .detail-textarea:focus {
        outline: none;
        border-color: var(--accent);
    }

    .history {
        margin-top: var(--space-4);
        padding-top: var(--space-3);
        border-top: 1px solid var(--surface-border);
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }

    .history-title {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.06em;
        color: var(--text-muted);
    }

    .history-entry {
        display: grid;
        grid-template-columns: 160px 1fr;
        gap: var(--space-3);
        padding: var(--space-2) 0;
        border-bottom: 1px dashed var(--surface-border);
    }

    .history-meta {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .history-changes {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .history-line {
        display: grid;
        grid-template-columns: 140px 1fr 20px 1fr;
        gap: var(--space-2);
        font-size: var(--text-xs);
        color: var(--text-secondary);
    }

    .history-field {
        font-family: var(--font-mono);
        color: var(--text-muted);
    }

    .history-prev,
    .history-next {
        color: var(--text-primary);
        word-break: break-word;
    }

    .history-arrow {
        text-align: center;
        color: var(--text-muted);
    }

    /* ── Allocation dropdown ── */
    .cell-alloc {
        width: 160px;
        position: relative;
    }

    .alloc-wrap {
        position: relative;
    }

    .alloc-btn {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-1);
        width: 100%;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border-subtle);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-xs);
        font-family: var(--font-sans);
        font-weight: var(--weight-medium);
        padding: 6px var(--space-2);
        cursor: pointer;
        text-align: left;
        transition:
            border-color var(--transition-fast),
            box-shadow var(--transition-fast);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .alloc-btn:hover {
        border-color: var(--surface-border-bright);
    }
    .alloc-btn.alloc-has-value {
        color: var(--accent-hover);
        border-color: var(--accent-border);
    }
    .alloc-btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .alloc-placeholder {
        color: var(--text-muted);
        font-style: italic;
    }
    .alloc-chevron {
        color: var(--text-muted);
        flex-shrink: 0;
        font-size: var(--text-xs);
    }

    .alloc-dropdown {
        position: absolute;
        top: calc(100% + 4px);
        left: 0;
        z-index: var(--z-dropdown);
        min-width: 200px;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        box-shadow: var(--shadow-lg);
        display: flex;
        flex-direction: column;
        overflow: hidden;
    }

    .alloc-dropdown-header {
        padding: var(--space-2) var(--space-3);
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
        border-bottom: 1px solid var(--surface-border);
    }

    .alloc-option {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        width: 100%;
        padding: var(--space-2) var(--space-3);
        background: none;
        border: none;
        color: var(--text-secondary);
        font-size: var(--text-sm);
        font-family: var(--font-sans);
        cursor: pointer;
        text-align: left;
        transition: background var(--transition-fast);
    }
    .alloc-option:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }
    .alloc-option.alloc-option-active {
        color: var(--accent-hover);
    }

    .alloc-check {
        width: 24px;
        flex-shrink: 0;
        color: var(--accent-hover);
        font-family: var(--font-mono);
        font-size: 11px;
    }

    .alloc-none {
        padding: var(--space-3);
        font-size: var(--text-xs);
        color: var(--text-muted);
        font-style: italic;
    }

    .alloc-option-disabled {
        opacity: 0.4;
        cursor: not-allowed;
    }
    .alloc-option-disabled:hover {
        background: none;
        color: var(--text-secondary);
    }

    .alloc-taken {
        margin-left: auto;
        font-size: var(--text-xs);
        color: var(--text-muted);
        font-style: italic;
        padding-left: var(--space-2);
    }

    .alloc-dropdown-footer {
        padding: var(--space-2) var(--space-3);
        border-top: 1px solid var(--surface-border);
        display: flex;
        justify-content: flex-end;
    }

    .alloc-close {
        padding: var(--space-1) var(--space-3);
        background: var(--accent-dim);
        border: 1px solid var(--accent);
        border-radius: var(--radius-md);
        color: var(--accent-hover);
        font-size: var(--text-xs);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .alloc-close:hover {
        background: var(--accent);
        color: #fff;
    }

    /* ── View tabs ── */
    .view-tabs {
        display: flex;
        gap: 2px;
        padding: 0 var(--space-5);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
        flex-shrink: 0;
    }
    .view-tab {
        padding: 8px var(--space-4);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        color: var(--text-muted);
        background: none;
        border: none;
        border-bottom: 2px solid transparent;
        cursor: pointer;
        transition: color 0.15s, border-color 0.15s;
        margin-bottom: -1px;
    }
    .view-tab:hover { color: var(--text-secondary); }
    .view-tab.active {
        color: var(--text-primary);
        border-bottom-color: var(--accent);
    }

    /* ── Allocation matrix ── */
    .alloc-wrap {
        flex: 1;
        overflow: auto;
        padding: var(--space-4) var(--space-5);
    }
    .alloc-empty {
        text-align: center;
        color: var(--text-muted);
        font-size: var(--text-sm);
        padding: var(--space-8);
    }
    .alloc-empty strong { color: var(--text-secondary); }
    .alloc-table {
        border-collapse: collapse;
        font-size: var(--text-xs);
        width: 100%;
    }
    .alloc-req-col {
        text-align: left;
        min-width: 260px;
        padding: var(--space-2) var(--space-3);
        background: var(--surface-raised);
        border-bottom: 2px solid var(--surface-border);
        position: sticky;
        top: 0;
        z-index: 2;
    }
    .alloc-block-col {
        min-width: 80px;
        max-width: 120px;
        padding: var(--space-2) var(--space-2);
        text-align: center;
        background: var(--surface-raised);
        border-bottom: 2px solid var(--surface-border);
        position: sticky;
        top: 0;
        z-index: 2;
    }
    .alloc-block-name {
        writing-mode: vertical-rl;
        transform: rotate(180deg);
        display: block;
        max-height: 100px;
        overflow: hidden;
        text-overflow: ellipsis;
        color: var(--text-secondary);
        font-weight: var(--weight-medium);
    }
    .alloc-req-cell {
        padding: var(--space-2) var(--space-3);
        border-bottom: 1px solid var(--surface-border-subtle);
        background: var(--surface-raised);
        position: sticky;
        left: 0;
        z-index: 1;
    }
    .alloc-req-id {
        font-family: var(--font-mono, monospace);
        font-size: 10px;
        color: var(--text-muted);
        margin-right: 6px;
    }
    .alloc-req-name {
        color: var(--text-secondary);
    }
    .alloc-cell {
        text-align: center;
        border-bottom: 1px solid var(--surface-border-subtle);
        border-left: 1px solid var(--surface-border-subtle);
        padding: var(--space-1);
        background: var(--surface-base);
        transition: background 0.1s;
    }
    .alloc-cell.allocated {
        background: #16a34a14;
    }
    .alloc-table tbody tr:hover .alloc-cell {
        background: var(--surface-hover);
    }
    .alloc-table tbody tr:hover .alloc-cell.allocated {
        background: #16a34a28;
    }
    .alloc-dot {
        font-size: 14px;
        color: #16a34a;
        line-height: 1;
    }
    .alloc-legend {
        display: flex;
        gap: var(--space-4);
        padding: var(--space-3) 0;
        font-size: var(--text-xs);
        color: var(--text-muted);
    }
    .legend-item { display: flex; align-items: center; gap: 4px; }
    .legend-dot { font-size: 13px; color: #16a34a; }

    /* ── Status bar ── */
    .status-bar {
        padding: var(--space-1) var(--space-5);
        font-size: var(--text-xs);
        color: var(--text-muted);
        border-top: 1px solid var(--surface-border);
        background: var(--surface-raised);
        flex-shrink: 0;
    }

    /* ── Source section badges ── */
    .source-sections {
        margin: var(--space-3) 0 var(--space-2);
        padding: var(--space-3) var(--space-4);
        background: var(--surface-overlay);
        border-radius: var(--radius-lg);
        border: 1px solid var(--surface-border);
    }

    .source-sections-title {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.1em;
        color: var(--text-muted);
        margin-bottom: var(--space-2);
    }

    .source-badges {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-2);
    }

    .source-badge {
        display: inline-flex;
        align-items: center;
        gap: var(--space-1);
        padding: var(--space-1) var(--space-3);
        border-radius: 999px;
        border: 1px solid var(--surface-border);
        background: var(--surface-raised);
        font-size: var(--text-xs);
        color: var(--text-secondary);
        border-left: 3px solid var(--surface-border);
    }
    .source-badge.type-heading {
        border-left-color: var(--accent);
    }
    .source-badge.type-requirement {
        border-left-color: #22c55e;
    }
    .source-badge.type-bom_item {
        border-left-color: #f59e0b;
    }
    .source-badge.type-sow_section {
        border-left-color: #818cf8;
    }
    .source-badge.loading {
        opacity: 0.5;
        font-style: italic;
    }

    .source-badge-ref {
        font-family: var(--font-mono);
        color: var(--accent-hover);
        font-size: 10px;
    }

    .source-badge-title {
        color: var(--text-primary);
        font-weight: var(--weight-medium);
    }

    .source-badge-type {
        font-size: 10px;
        color: var(--text-muted);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        padding: 0 4px;
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }

    /* -- Suspect links -- */
    .suspect-banner {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.5rem 1rem;
        background: rgba(234, 179, 8, 0.15);
        border: 1px solid rgba(234, 179, 8, 0.4);
        border-radius: 6px;
        margin: 0.5rem 1rem 0;
        color: #eab308;
        font-size: 0.875rem;
        flex-shrink: 0;
    }
    .suspect-banner button {
        padding: 0.25rem 0.75rem;
        background: rgba(234, 179, 8, 0.2);
        border: 1px solid rgba(234, 179, 8, 0.5);
        border-radius: 4px;
        color: #eab308;
        cursor: pointer;
        font-size: 0.8rem;
    }
    .suspect-panel {
        background: var(--surface-1, var(--surface-overlay));
        border: 1px solid var(--border, var(--surface-border));
        border-radius: 8px;
        padding: 1rem;
        margin: 0.5rem 1rem 0;
        flex-shrink: 0;
    }
    .suspect-panel h3 {
        margin: 0 0 0.75rem;
        color: #eab308;
        font-size: 0.95rem;
    }
    .suspect-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.5rem 0;
        border-bottom: 1px solid var(--border, var(--surface-border));
    }
    .suspect-row:last-child {
        border-bottom: none;
    }
    .suspect-info {
        font-size: 0.85rem;
        color: var(--text-2, var(--text-secondary));
    }
    .suspect-date {
        margin-left: 0.5rem;
        font-size: 0.75rem;
        opacity: 0.7;
    }
    .resolve-btn {
        padding: 0.2rem 0.6rem;
        background: rgba(34, 197, 94, 0.15);
        border: 1px solid rgba(34, 197, 94, 0.4);
        border-radius: 4px;
        color: #22c55e;
        cursor: pointer;
        font-size: 0.8rem;
        white-space: nowrap;
    }
    .suspect-badge {
        display: inline-block;
        padding: 0.1rem 0.4rem;
        background: rgba(234, 179, 8, 0.2);
        border: 1px solid rgba(234, 179, 8, 0.4);
        border-radius: 3px;
        color: #eab308;
        font-size: 0.72rem;
        font-weight: 600;
        margin-left: 0.4rem;
    }

    /* -- Comment count button -- */
    .comment-count-btn {
        display: inline-flex;
        align-items: center;
        gap: 0.25rem;
        padding: 0.15rem 0.5rem;
        background: transparent;
        border: 1px solid var(--surface-border);
        border-radius: 4px;
        color: var(--text-muted);
        cursor: pointer;
        font-size: 0.78rem;
        margin-right: 0.25rem;
    }
    .comment-count-btn:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }

    /* -- Comment expand area -- */
    .comment-expand-row td {
        padding: 0;
        background: var(--surface-base);
        border-bottom: 2px solid var(--accent);
    }
    .comment-expand-area {
        padding: 0.75rem 1rem;
        background: var(--surface-overlay);
        border-top: 1px solid var(--surface-border);
    }

    /* -- Review panel button -- */
    .btn-review {
        display: inline-flex;
        align-items: center;
        gap: 0.4rem;
        padding: var(--space-1) var(--space-3);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        cursor: pointer;
        transition: all var(--transition-fast);
        white-space: nowrap;
    }
    .btn-review:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }

    .btn-primary {
        padding: var(--space-1) var(--space-3);
        background: var(--accent);
        border: none;
        border-radius: var(--radius-md);
        color: white;
        font-size: var(--text-sm);
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

    .review-badge {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-width: 18px;
        height: 18px;
        padding: 0 4px;
        background: var(--accent);
        border-radius: 999px;
        color: white;
        font-size: 11px;
        font-weight: 600;
    }

    /* -- Review panel -- */
    .review-panel {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        margin: 0.5rem 1rem 0;
        flex-shrink: 0;
        max-height: 480px;
        overflow-y: auto;
    }
    .review-panel-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.75rem 1rem;
        border-bottom: 1px solid var(--surface-border);
        position: sticky;
        top: 0;
        background: var(--surface-raised);
        z-index: 1;
    }
    .review-panel-header h3 {
        margin: 0;
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
    }
    .panel-close {
        background: none;
        border: none;
        color: var(--text-muted);
        cursor: pointer;
        font-size: var(--text-base);
        padding: 0 4px;
        border-radius: var(--radius-sm);
        transition: color var(--transition-fast);
    }
    .panel-close:hover {
        color: var(--text-primary);
    }

    .review-create-section {
        padding: 0.75rem 1rem;
        border-bottom: 1px solid var(--surface-border);
    }
    .review-create-section h4 {
        margin: 0 0 0.25rem;
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
    }
    .review-help {
        font-size: var(--text-xs);
        color: var(--text-muted);
        margin: 0 0 0.5rem;
    }
    .review-create-row {
        display: flex;
        gap: 0.5rem;
        align-items: center;
    }
    .review-title-input {
        flex: 1;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-size: var(--text-sm);
        font-family: var(--font-sans);
        padding: var(--space-1) var(--space-2);
    }
    .review-title-input:focus {
        outline: none;
        border-color: var(--accent);
    }

    .review-list {
        padding: 0.5rem;
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
    }
    .review-session-card {
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        background: var(--surface-overlay);
        overflow: hidden;
    }
    .review-session-card.active {
        border-color: var(--accent);
    }
    .review-session-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.5rem 0.75rem;
        cursor: pointer;
    }
    .review-session-header:hover {
        background: var(--surface-hover);
    }
    .review-session-title {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        font-size: var(--text-sm);
    }
    .review-session-meta {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        font-size: var(--text-xs);
        color: var(--text-muted);
    }
    .review-status-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        flex-shrink: 0;
        background: var(--text-muted);
    }
    .review-status-dot.status-open {
        background: #3b82f6;
    }
    .review-status-dot.status-in_progress {
        background: #f59e0b;
    }
    .review-status-dot.status-approved {
        background: #22c55e;
    }
    .review-status-dot.status-rejected {
        background: #ef4444;
    }
    .review-status-dot.status-closed {
        background: var(--text-muted);
    }
    .review-status-label {
        text-transform: capitalize;
    }
    .review-item-count {
        color: var(--text-muted);
    }
    .review-date {
        color: var(--text-muted);
    }

    .review-items {
        padding: 0.25rem 0.75rem 0.5rem;
        border-top: 1px solid var(--surface-border);
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
    }
    .review-item {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 0.5rem;
        padding: 0.25rem 0;
    }
    .review-item-id {
        font-family: var(--font-mono);
        font-size: var(--text-xs);
        color: var(--text-muted);
        min-width: 80px;
    }
    .review-item-verdicts {
        display: flex;
        gap: 0.3rem;
    }
    .verdict-btn {
        padding: 0.15rem 0.6rem;
        border-radius: 4px;
        border: 1px solid var(--surface-border);
        background: transparent;
        cursor: pointer;
        font-size: var(--text-xs);
        color: var(--text-muted);
        transition: all var(--transition-fast);
    }
    .verdict-btn.approved {
        border-color: #22c55e40;
    }
    .verdict-btn.approved.active {
        background: #22c55e20;
        color: #22c55e;
        border-color: #22c55e;
    }
    .verdict-btn.approved:hover {
        background: #22c55e20;
        color: #22c55e;
        border-color: #22c55e;
    }
    .verdict-btn.needs_changes {
        border-color: #f59e0b40;
    }
    .verdict-btn.needs_changes.active {
        background: #f59e0b20;
        color: #f59e0b;
        border-color: #f59e0b;
    }
    .verdict-btn.needs_changes:hover {
        background: #f59e0b20;
        color: #f59e0b;
        border-color: #f59e0b;
    }
    .verdict-btn.rejected {
        border-color: #ef444440;
    }
    .verdict-btn.rejected.active {
        background: #ef444420;
        color: #ef4444;
        border-color: #ef4444;
    }
    .verdict-btn.rejected:hover {
        background: #ef444420;
        color: #ef4444;
        border-color: #ef4444;
    }

    .review-close-row {
        display: flex;
        gap: 0.4rem;
        padding: 0.4rem 0 0.25rem;
        flex-wrap: wrap;
    }
    .btn-approve-all {
        padding: 0.2rem 0.6rem;
        border-radius: 4px;
        border: 1px solid #22c55e40;
        background: #22c55e15;
        color: #22c55e;
        cursor: pointer;
        font-size: var(--text-xs);
        transition: all var(--transition-fast);
    }
    .btn-approve-all:hover {
        background: #22c55e30;
        border-color: #22c55e;
    }
    .btn-reject-all {
        padding: 0.2rem 0.6rem;
        border-radius: 4px;
        border: 1px solid #ef444440;
        background: #ef444415;
        color: #ef4444;
        cursor: pointer;
        font-size: var(--text-xs);
        transition: all var(--transition-fast);
    }
    .btn-reject-all:hover {
        background: #ef444430;
        border-color: #ef4444;
    }
    .btn-close-review {
        padding: 0.2rem 0.6rem;
        border-radius: 4px;
        border: 1px solid var(--surface-border);
        background: transparent;
        color: var(--text-muted);
        cursor: pointer;
        font-size: var(--text-xs);
        transition: all var(--transition-fast);
    }
    .btn-close-review:hover {
        background: var(--surface-hover);
        color: var(--text-primary);
    }
</style>

<script lang="ts">
    import { page } from "$app/stores";
    import { invoke } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { GitBranch, Plus, Trash2, Download, ChevronDown, ChevronRight } from "lucide-svelte";

    $: projectId = $page.params.id;

    interface SnapshotNode {
        kind?: string;
        name?: string;
        id?: string;
    }

    interface ModelBaseline {
        id: string;
        project_id: string;
        name: string;
        description: string;
        created_by: string;
        created_at: string;
        snapshot: {
            nodes?: unknown[];
            edges?: unknown[];
        };
    }

    function asNode(n: unknown): SnapshotNode {
        return n as SnapshotNode;
    }

    let baselines: ModelBaseline[] = [];
    let loading = false;
    let creating = false;
    let error = "";

    // Create form state
    let newName = "";
    let newDescription = "";
    let showCreateForm = false;

    // Expanded baseline (for diff preview)
    let expandedId = "";

    onMount(() => void load());

    async function load() {
        loading = true;
        error = "";
        try {
            baselines = await invoke<ModelBaseline[]>("list_baselines", { projectId });
        } catch (e) {
            error = String(e);
        } finally {
            loading = false;
        }
    }

    async function createBaseline() {
        if (!newName.trim()) return;
        creating = true;
        error = "";
        try {
            const b = await invoke<ModelBaseline>("create_baseline", {
                projectId,
                name: newName.trim(),
                description: newDescription.trim(),
            });
            baselines = [b, ...baselines];
            newName = "";
            newDescription = "";
            showCreateForm = false;
        } catch (e) {
            error = String(e);
        } finally {
            creating = false;
        }
    }

    async function deleteBaseline(id: string) {
        if (!confirm("Delete this baseline? This cannot be undone.")) return;
        try {
            await invoke("delete_baseline", { id });
            baselines = baselines.filter((b) => b.id !== id);
            if (expandedId === id) expandedId = "";
        } catch (e) {
            error = String(e);
        }
    }

    function downloadBaseline(b: ModelBaseline) {
        const json = JSON.stringify(b.snapshot, null, 2);
        const blob = new Blob([json], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `${b.name.replace(/\s+/g, "_")}_baseline.json`;
        a.click();
        URL.revokeObjectURL(url);
    }

    function formatDate(iso: string): string {
        const d = new Date(iso);
        return d.toLocaleDateString("en-US", {
            year: "numeric",
            month: "short",
            day: "numeric",
            hour: "2-digit",
            minute: "2-digit",
        });
    }

    function nodeCount(b: ModelBaseline): number {
        return b.snapshot?.nodes?.length ?? 0;
    }
    function edgeCount(b: ModelBaseline): number {
        return b.snapshot?.edges?.length ?? 0;
    }
</script>

<div class="page">
    <!-- Header -->
    <div class="page-header">
        <div class="header-left">
            <GitBranch size={18} class="header-icon" />
            <div>
                <h1 class="page-title">Model Baselines</h1>
                <p class="page-subtitle">
                    Named snapshots of the full model state for design reviews
                    and comparison.
                </p>
            </div>
        </div>
        <button
            class="btn-primary"
            on:click={() => (showCreateForm = !showCreateForm)}
        >
            <Plus size={14} />
            New Baseline
        </button>
    </div>

    {#if error}
        <div class="error-banner">{error}</div>
    {/if}

    <!-- Create form -->
    {#if showCreateForm}
        <div class="create-card">
            <h2 class="create-title">Save Current Snapshot</h2>
            <div class="form-row">
                <label class="form-label" for="baseline-name">Name</label>
                <input
                    id="baseline-name"
                    class="form-input"
                    type="text"
                    placeholder="e.g. CDR Baseline v1.0"
                    bind:value={newName}
                    on:keydown={(e) => e.key === "Enter" && void createBaseline()}
                />
            </div>
            <div class="form-row">
                <label class="form-label" for="baseline-desc">Description</label>
                <input
                    id="baseline-desc"
                    class="form-input"
                    type="text"
                    placeholder="Optional notes about this snapshot"
                    bind:value={newDescription}
                />
            </div>
            <div class="form-actions">
                <button
                    class="btn-secondary"
                    on:click={() => (showCreateForm = false)}>Cancel</button
                >
                <button
                    class="btn-primary"
                    disabled={!newName.trim() || creating}
                    on:click={() => void createBaseline()}
                >
                    {#if creating}Saving…{:else}Save Baseline{/if}
                </button>
            </div>
        </div>
    {/if}

    <!-- Baselines list -->
    {#if loading}
        <div class="empty-state">Loading baselines…</div>
    {:else if baselines.length === 0}
        <div class="empty-state">
            <GitBranch size={40} class="empty-icon" />
            <p>No baselines yet.</p>
            <p class="empty-sub">
                Click <strong>New Baseline</strong> to capture the current model
                state.
            </p>
        </div>
    {:else}
        <div class="baselines-list">
            {#each baselines as b (b.id)}
                {@const isExpanded = expandedId === b.id}
                <div class="baseline-card" class:expanded={isExpanded}>
                    <div class="baseline-header">
                        <button
                            class="expand-btn"
                            on:click={() =>
                                (expandedId = isExpanded ? "" : b.id)}
                            title={isExpanded ? "Collapse" : "Expand"}
                        >
                            {#if isExpanded}
                                <ChevronDown size={14} />
                            {:else}
                                <ChevronRight size={14} />
                            {/if}
                        </button>

                        <div class="baseline-meta">
                            <span class="baseline-name">{b.name}</span>
                            {#if b.description}
                                <span class="baseline-desc">{b.description}</span>
                            {/if}
                        </div>

                        <div class="baseline-stats">
                            <span class="stat-pill"
                                >{nodeCount(b)} nodes</span
                            >
                            <span class="stat-pill"
                                >{edgeCount(b)} edges</span
                            >
                        </div>

                        <div class="baseline-date">
                            <span>{formatDate(b.created_at)}</span>
                            <span class="baseline-by">by {b.created_by}</span>
                        </div>

                        <div class="baseline-actions">
                            <button
                                class="icon-btn"
                                title="Download snapshot JSON"
                                on:click={() => downloadBaseline(b)}
                            >
                                <Download size={13} />
                            </button>
                            <button
                                class="icon-btn danger"
                                title="Delete baseline"
                                on:click={() => void deleteBaseline(b.id)}
                            >
                                <Trash2 size={13} />
                            </button>
                        </div>
                    </div>

                    {#if isExpanded}
                        <div class="baseline-detail">
                            <div class="detail-section">
                                <h3 class="detail-heading">
                                    Nodes ({nodeCount(b)})
                                </h3>
                                <div class="node-grid">
                                    {#each (b.snapshot?.nodes ?? []) as node}
                                        {@const n = asNode(node)}
                                        <div class="node-chip">
                                            <span class="node-kind"
                                                >{n.kind ?? "?"}</span
                                            >
                                            <span class="node-name"
                                                >{n.name ?? n.id ?? "—"}</span
                                            >
                                        </div>
                                    {/each}
                                </div>
                            </div>
                        </div>
                    {/if}
                </div>
            {/each}
        </div>
    {/if}
</div>

<style>
    .page {
        flex: 1;
        display: flex;
        flex-direction: column;
        gap: var(--space-4);
        padding: var(--space-5) var(--space-6);
        overflow-y: auto;
        min-height: 0;
    }

    /* ── Header ── */
    .page-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: var(--space-4);
    }
    .header-left {
        display: flex;
        align-items: flex-start;
        gap: var(--space-3);
    }
    :global(.header-icon) {
        color: var(--accent);
        margin-top: 3px;
        flex-shrink: 0;
    }
    .page-title {
        font-size: var(--text-xl);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
        margin: 0 0 2px;
    }
    .page-subtitle {
        font-size: var(--text-sm);
        color: var(--text-muted);
        margin: 0;
    }

    /* ── Buttons ── */
    .btn-primary {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        padding: 8px var(--space-4);
        background: var(--accent);
        border: 1px solid var(--accent);
        border-radius: var(--radius-md);
        color: #fff;
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        cursor: pointer;
        white-space: nowrap;
        transition: all var(--transition-fast);
    }
    .btn-primary:hover:not(:disabled) {
        background: var(--accent-hover);
    }
    .btn-primary:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .btn-secondary {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        padding: 8px var(--space-4);
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-secondary:hover {
        background: var(--surface-hover);
        border-color: var(--surface-border-bright);
    }

    /* ── Error ── */
    .error-banner {
        padding: var(--space-3) var(--space-4);
        background: #7f1d1d22;
        border: 1px solid #ef444444;
        border-radius: var(--radius-md);
        color: #f87171;
        font-size: var(--text-sm);
    }

    /* ── Create card ── */
    .create-card {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        padding: var(--space-5);
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }
    .create-title {
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
        margin: 0;
    }
    .form-row {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .form-label {
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        color: var(--text-muted);
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }
    .form-input {
        padding: 8px var(--space-3);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-primary);
        font-family: var(--font-sans);
        font-size: var(--text-sm);
        width: 100%;
        box-sizing: border-box;
    }
    .form-input:focus {
        outline: none;
        border-color: var(--accent);
    }
    .form-actions {
        display: flex;
        justify-content: flex-end;
        gap: var(--space-2);
    }

    /* ── Empty ── */
    .empty-state {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: var(--space-3);
        color: var(--text-muted);
        font-size: var(--text-sm);
        padding: var(--space-10) 0;
        text-align: center;
    }
    :global(.empty-icon) {
        color: var(--text-muted);
        opacity: 0.4;
    }
    .empty-sub {
        font-size: var(--text-xs);
        color: var(--text-muted);
        opacity: 0.7;
    }

    /* ── Baselines list ── */
    .baselines-list {
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .baseline-card {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        overflow: hidden;
        transition: border-color var(--transition-fast);
    }
    .baseline-card:hover {
        border-color: var(--surface-border-bright);
    }
    .baseline-card.expanded {
        border-color: var(--accent-border);
    }

    .baseline-header {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        padding: var(--space-3) var(--space-4);
    }

    .expand-btn {
        background: none;
        border: none;
        color: var(--text-muted);
        cursor: pointer;
        padding: 4px;
        border-radius: var(--radius-sm);
        display: flex;
        align-items: center;
        flex-shrink: 0;
        transition: color var(--transition-fast);
    }
    .expand-btn:hover {
        color: var(--text-secondary);
    }

    .baseline-meta {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 2px;
    }
    .baseline-name {
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .baseline-desc {
        font-size: var(--text-xs);
        color: var(--text-muted);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .baseline-stats {
        display: flex;
        gap: var(--space-2);
        flex-shrink: 0;
    }
    .stat-pill {
        padding: 2px 8px;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-full);
        font-size: 11px;
        color: var(--text-muted);
        white-space: nowrap;
    }

    .baseline-date {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        gap: 1px;
        flex-shrink: 0;
    }
    .baseline-date > span:first-child {
        font-size: var(--text-xs);
        color: var(--text-secondary);
    }
    .baseline-by {
        font-size: 11px;
        color: var(--text-muted);
    }

    .baseline-actions {
        display: flex;
        gap: var(--space-1);
        flex-shrink: 0;
    }
    .icon-btn {
        background: none;
        border: 1px solid transparent;
        border-radius: var(--radius-md);
        color: var(--text-muted);
        cursor: pointer;
        padding: 5px;
        display: flex;
        align-items: center;
        transition: all var(--transition-fast);
    }
    .icon-btn:hover {
        background: var(--surface-hover);
        border-color: var(--surface-border);
        color: var(--text-secondary);
    }
    .icon-btn.danger:hover {
        background: #7f1d1d22;
        border-color: #ef444444;
        color: #f87171;
    }

    /* ── Expanded detail ── */
    .baseline-detail {
        border-top: 1px solid var(--surface-border);
        padding: var(--space-4);
        background: var(--surface-base);
    }
    .detail-section {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }
    .detail-heading {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        color: var(--text-muted);
        text-transform: uppercase;
        letter-spacing: 0.05em;
        margin: 0;
    }
    .node-grid {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-2);
    }
    .node-chip {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        padding: 3px 8px;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        font-size: 11px;
    }
    .node-kind {
        color: var(--accent);
        font-weight: var(--weight-medium);
        text-transform: uppercase;
        font-size: 10px;
        letter-spacing: 0.04em;
    }
    .node-name {
        color: var(--text-secondary);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        max-width: 180px;
    }
</style>

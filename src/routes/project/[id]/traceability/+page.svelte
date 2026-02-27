<script lang="ts">
    import { onMount } from "svelte";
    import { page } from "$app/stores";
    import { loadProject, nodes, edges } from "$lib/store/model";
    import TraceabilityMatrix from "$lib/panels/TraceabilityMatrix.svelte";
    import type { Node } from "$lib/types";

    $: projectId = $page.params.id;

    $: requirements = $nodes.filter((n) => n.kind === "requirement") as Node[];
    $: blocks = $nodes.filter((n) => n.kind === "block") as Node[];
    $: testCases = $nodes.filter((n) => n.kind === "test_case") as Node[];

    $: satisfiesCount = $edges.filter((e) => e.kind === "satisfies").length;
    $: verifiesCount = $edges.filter((e) => e.kind === "verifies").length;
    $: satisfiesTargets = new Set(
        $edges.filter((e) => e.kind === "satisfies").map((e) => e.target_id),
    );
    $: verifiesTargets = new Set(
        $edges.filter((e) => e.kind === "verifies").map((e) => e.target_id),
    );
    $: reqMissingSatisfies = requirements.filter(
        (r) => !satisfiesTargets.has(r.id),
    );
    $: reqMissingVerifies = requirements.filter(
        (r) => !verifiesTargets.has(r.id),
    );

    onMount(async () => {
        await loadProject(projectId);
    });
</script>

<div class="trace-root page-frame">
    <header class="trace-header page-header">
        <div>
            <div class="page-eyebrow">Traceability</div>
            <h1 class="page-title">Coverage & Links</h1>
            <p class="page-subtitle">
                Check coverage across requirements, design blocks, and
                verification assets.
            </p>
        </div>
        <div class="stat-bar">
            <div class="stat">
                <div class="stat-value">{requirements.length}</div>
                <div class="stat-label">Requirements</div>
            </div>
            <div class="stat">
                <div class="stat-value">{blocks.length}</div>
                <div class="stat-label">Blocks</div>
            </div>
            <div class="stat">
                <div class="stat-value">{testCases.length}</div>
                <div class="stat-label">Test Cases</div>
            </div>
            <div class="stat">
                <div class="stat-value">{satisfiesCount}</div>
                <div class="stat-label">Satisfies Links</div>
            </div>
            <div class="stat">
                <div class="stat-value">{verifiesCount}</div>
                <div class="stat-label">Verifies Links</div>
            </div>
        </div>
    </header>

    <main class="trace-body page-body">
        <section class="coverage-grid">
            <div class="coverage-card">
                <div class="card-left">
                    <div
                        class="card-count"
                        class:zero={reqMissingSatisfies.length === 0}
                    >
                        {reqMissingSatisfies.length}
                    </div>
                    <div class="card-title">No satisfies</div>
                </div>
                <div class="card-divider"></div>
                <div class="card-right">
                    {#if reqMissingSatisfies.length === 0}
                        <div class="card-empty">All requirements covered</div>
                    {:else}
                        <div class="card-list">
                            {#each reqMissingSatisfies.slice(0, 3) as r (r.id)}
                                <div class="card-item">{r.name}</div>
                            {/each}
                            {#if reqMissingSatisfies.length > 3}
                                <div class="card-more">
                                    +{reqMissingSatisfies.length - 3} more
                                </div>
                            {/if}
                        </div>
                    {/if}
                </div>
            </div>

            <div class="coverage-card">
                <div class="card-left">
                    <div
                        class="card-count"
                        class:zero={reqMissingVerifies.length === 0}
                    >
                        {reqMissingVerifies.length}
                    </div>
                    <div class="card-title">No verifies</div>
                </div>
                <div class="card-divider"></div>
                <div class="card-right">
                    {#if reqMissingVerifies.length === 0}
                        <div class="card-empty">All requirements covered</div>
                    {:else}
                        <div class="card-list">
                            {#each reqMissingVerifies.slice(0, 3) as r (r.id)}
                                <div class="card-item">{r.name}</div>
                            {/each}
                            {#if reqMissingVerifies.length > 3}
                                <div class="card-more">
                                    +{reqMissingVerifies.length - 3} more
                                </div>
                            {/if}
                        </div>
                    {/if}
                </div>
            </div>
        </section>

        <TraceabilityMatrix />
    </main>
</div>

<style>
    .trace-root {
        display: flex;
        flex-direction: column;
        height: 100%;
        background: var(--surface-base);
    }

    .trace-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: var(--space-6);
        padding: var(--space-4) var(--space-6) var(--space-3);
    }

    .stat-bar {
        display: flex;
        gap: var(--space-3);
        flex-wrap: wrap;
        align-items: center;
    }

    .stat {
        display: flex;
        flex-direction: column;
        align-items: flex-end;
        min-width: 72px;
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

    .trace-body {
        flex: 1;
        overflow: auto;
        min-height: 0;
    }

    .coverage-grid {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
        gap: var(--space-3);
        padding: var(--space-3) var(--space-6);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
    }

    .coverage-card {
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
        padding: var(--space-3) var(--space-4);
        display: flex;
        align-items: center;
        gap: var(--space-4);
    }

    .card-left {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        flex-shrink: 0;
        min-width: 44px;
    }

    .card-title {
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: var(--tracking-wide);
        color: var(--text-muted);
        white-space: nowrap;
    }

    .card-count {
        font-size: var(--text-2xl);
        font-weight: var(--weight-bold);
        line-height: 1;
        color: var(--text-primary);
    }
    .card-count.zero {
        color: var(--color-success);
    }

    .card-divider {
        width: 1px;
        align-self: stretch;
        background: var(--surface-border);
        flex-shrink: 0;
    }

    .card-right {
        flex: 1;
        min-width: 0;
    }

    .card-empty {
        font-size: var(--text-xs);
        color: var(--color-success);
        display: flex;
        align-items: center;
        gap: 5px;
    }

    .card-list {
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .card-item {
        font-size: var(--text-xs);
        color: var(--text-secondary);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .card-more {
        font-size: var(--text-xs);
        color: var(--accent-hover);
        margin-top: 2px;
    }
</style>

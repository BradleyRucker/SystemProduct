<script lang="ts">
    import { onMount } from "svelte";
    import { fade, slide } from "svelte/transition";
    import { page } from "$app/stores";
    import { invoke } from "@tauri-apps/api/core";
    import { nodes } from "$lib/store/model";
    import type {
        Node,
        SimulationScenario,
        SimulationScenarioEvent,
        SimulationResult,
        BlockSimMetrics,
        SimParams,
    } from "$lib/types";
    import { v4 as uuidv4 } from "uuid";

    $: projectId = $page.params.id;
    $: blockNodes = $nodes.filter((n): n is Node => n.kind === "block");

    // ── Block sim config (script + params per block) ───────────────────────────
    // blockConfigs[blockId] = { sim_params, sim_script }
    let blockConfigs: Record<
        string,
        { sim_params: SimParams; sim_script: string }
    > = {};
    let expandedBlock: string | null = null;
    let configSaving: Record<string, boolean> = {};

    async function loadBlockConfig(blockId: string) {
        try {
            const result = await invoke<{
                sim_params: SimParams | null;
                sim_script: string | null;
            }>("get_sim_params", { nodeId: blockId });
            blockConfigs[blockId] = {
                sim_params: result.sim_params ?? {},
                sim_script: result.sim_script ?? "",
            };
            blockConfigs = blockConfigs; // trigger reactivity
        } catch {
            blockConfigs[blockId] = { sim_params: {}, sim_script: "" };
            blockConfigs = blockConfigs;
        }
    }

    async function saveBlockConfig(blockId: string) {
        const cfg = blockConfigs[blockId];
        if (!cfg) return;
        configSaving[blockId] = true;
        configSaving = configSaving;
        try {
            await invoke("save_sim_params", {
                nodeId: blockId,
                params:
                    Object.keys(cfg.sim_params).length > 0
                        ? cfg.sim_params
                        : null,
                script: cfg.sim_script.trim() || null,
            });
        } finally {
            configSaving[blockId] = false;
            configSaving = configSaving;
        }
    }

    function updateBlockParam(blockId: string, field: string, value: unknown) {
        const cfg = blockConfigs[blockId] ?? { sim_params: {}, sim_script: "" };
        blockConfigs[blockId] = {
            ...cfg,
            sim_params: { ...cfg.sim_params, [field]: value },
        };
        blockConfigs = blockConfigs;
    }

    function updateBlockScript(blockId: string, script: string) {
        const cfg = blockConfigs[blockId] ?? { sim_params: {}, sim_script: "" };
        blockConfigs[blockId] = { ...cfg, sim_script: script };
        blockConfigs = blockConfigs;
    }

    async function importBlockScript(blockId: string, e: Event) {
        const input = e.target as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;
        const text = await file.text();
        updateBlockScript(blockId, text);
        input.value = "";
        // Auto-show the script
        if (!blockConfigs[blockId])
            blockConfigs[blockId] = { sim_params: {}, sim_script: text };
        else
            blockConfigs[blockId] = {
                ...blockConfigs[blockId],
                sim_script: text,
            };
        blockConfigs = blockConfigs;
    }

    async function toggleBlock(blockId: string) {
        if (expandedBlock === blockId) {
            expandedBlock = null;
        } else {
            expandedBlock = blockId;
            if (!blockConfigs[blockId]) await loadBlockConfig(blockId);
        }
    }

    function blockHasScript(blockId: string): boolean {
        return !!blockConfigs[blockId]?.sim_script?.trim();
    }

    function blockHasParams(blockId: string): boolean {
        const p = blockConfigs[blockId]?.sim_params ?? {};
        return Object.values(p).some(
            (v) => v !== undefined && v !== null && v !== "",
        );
    }

    // ── Scenario state ────────────────────────────────────────────────────────
    let scenarios: SimulationScenario[] = [];
    let activeScenario: SimulationScenario = makeEmptyScenario();
    let scenarioListOpen = false;

    function makeEmptyScenario(): SimulationScenario {
        const now = new Date().toISOString();
        return {
            id: uuidv4(),
            project_id: projectId,
            name: "New Scenario",
            description: "",
            duration_ms: 10000,
            events: [],
            created_at: now,
            modified_at: now,
        };
    }

    function addEvent() {
        const first = blockNodes[0];
        activeScenario = {
            ...activeScenario,
            events: [
                ...activeScenario.events,
                {
                    time_ms: 0,
                    block_id: first?.id ?? "",
                    signal_type: "trigger",
                    value: null,
                },
            ],
        };
    }

    function removeEvent(index: number) {
        activeScenario = {
            ...activeScenario,
            events: activeScenario.events.filter((_, i) => i !== index),
        };
    }

    function updateEvent(
        index: number,
        field: keyof SimulationScenarioEvent,
        value: unknown,
    ) {
        const evs = [...activeScenario.events];
        evs[index] = { ...evs[index], [field]: value };
        activeScenario = { ...activeScenario, events: evs };
    }

    async function saveScenario() {
        // Persist all block configs first so they are included in the run
        const dirtyBlocks = Object.keys(blockConfigs);
        await Promise.all(dirtyBlocks.map((id) => saveBlockConfig(id)));

        activeScenario = {
            ...activeScenario,
            project_id: projectId,
            modified_at: new Date().toISOString(),
        };
        await invoke("save_scenario", { scenario: activeScenario });
        await loadScenarios();
    }

    async function loadScenarios() {
        try {
            scenarios = await invoke<SimulationScenario[]>("list_scenarios", {
                projectId,
            });
        } catch {
            scenarios = [];
        }
    }

    function selectScenario(s: SimulationScenario) {
        activeScenario = { ...s };
        scenarioListOpen = false;
    }

    // ── Run state ─────────────────────────────────────────────────────────────
    let running = false;
    let runError = "";
    let lastResult: SimulationResult | null = null;
    let selectedResultBlock: string | null = null;

    async function runSimulation() {
        if (running) return;
        running = true;
        runError = "";
        lastResult = null;
        selectedResultBlock = null;
        try {
            await saveScenario();
            const resultId = await invoke<string>("run_simulation", {
                scenarioId: activeScenario.id,
            });
            lastResult = await invoke<SimulationResult>(
                "get_simulation_result",
                {
                    resultId,
                },
            );
            // Auto-select first block with metrics
            if (lastResult?.metrics) {
                const first = blockNodes.find((b) => getMetric(b.id) !== null);
                selectedResultBlock = first?.id ?? null;
            }
        } catch (e) {
            runError = String(e);
        } finally {
            running = false;
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────
    function getMetric(blockId: string): BlockSimMetrics | null {
        if (!lastResult?.metrics) return null;
        return (
            (lastResult.metrics as Record<string, BlockSimMetrics>)[blockId] ??
            null
        );
    }

    function statusClass(s: string) {
        if (s === "complete") return "badge-ok";
        if (s === "error") return "badge-err";
        return "badge-pending";
    }

    function blockName(blockId: string): string {
        return (
            $nodes.find((n) => n.id === blockId)?.name ?? blockId.slice(0, 8)
        );
    }

    function utilBar(util: number): string {
        if (util > 0.85) return "bar-danger";
        if (util > 0.6) return "bar-warn";
        return "bar-ok";
    }

    $: blockTimeline =
        lastResult && selectedResultBlock
            ? (
                  lastResult.timeline as Array<{
                      timestamp_ms: number;
                      block_id: string;
                      event_type: string;
                      detail: string;
                  }>
              ).filter((e) => e.block_id === selectedResultBlock)
            : ((lastResult?.timeline as Array<{
                  timestamp_ms: number;
                  block_id: string;
                  event_type: string;
                  detail: string;
              }>) ?? []);

    onMount(() => {
        void loadScenarios();
    });
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="sim-page">
    <!-- ── Left: block configurator ─────────────────────────────────────── -->
    <aside class="sim-left">
        <div class="panel-hdr">
            <span class="panel-title">Block Configs</span>
            <span class="panel-hint"
                >{blockNodes.length} block{blockNodes.length === 1
                    ? ""
                    : "s"}</span
            >
        </div>

        {#if blockNodes.length === 0}
            <div class="empty-hint hint-center">No blocks in project yet.</div>
        {:else}
            {#each blockNodes as b (b.id)}
                {@const hasScript = blockHasScript(b.id)}
                {@const hasParams = blockHasParams(b.id)}
                {@const isOpen = expandedBlock === b.id}
                {@const cfg = blockConfigs[b.id] ?? {
                    sim_params: {},
                    sim_script: "",
                }}

                <div class="block-cfg-item" class:open={isOpen}>
                    <button
                        class="block-cfg-hdr"
                        on:click={() => toggleBlock(b.id)}
                    >
                        <span class="block-cfg-name">{b.name}</span>
                        <span class="block-cfg-badges">
                            {#if hasScript}<span
                                    class="badge-script"
                                    title="Script configured">PY</span
                                >{/if}
                            {#if hasParams}<span
                                    class="badge-params"
                                    title="Params configured">⚙</span
                                >{/if}
                        </span>
                        <span class="block-cfg-arrow">{isOpen ? "▲" : "▼"}</span
                        >
                    </button>

                    {#if isOpen}
                        <div
                            class="block-cfg-body"
                            transition:slide={{ duration: 150 }}
                        >
                            <!-- Params -->
                            <div class="cfg-section-label">Parameters</div>
                            <div class="cfg-row">
                                <label class="cfg-lbl"
                                    >Processing ms
                                    <input
                                        class="cfg-field"
                                        type="number"
                                        min="0"
                                        value={cfg.sim_params
                                            .processing_time_ms ?? ""}
                                        on:change={(e) =>
                                            updateBlockParam(
                                                b.id,
                                                "processing_time_ms",
                                                parseFloat(
                                                    e.currentTarget.value,
                                                ) || undefined,
                                            )}
                                    />
                                </label>
                                <label class="cfg-lbl"
                                    >Failure rate
                                    <input
                                        class="cfg-field"
                                        type="number"
                                        min="0"
                                        max="1"
                                        step="0.01"
                                        value={cfg.sim_params.failure_rate ??
                                            ""}
                                        on:change={(e) =>
                                            updateBlockParam(
                                                b.id,
                                                "failure_rate",
                                                parseFloat(
                                                    e.currentTarget.value,
                                                ) || undefined,
                                            )}
                                    />
                                </label>
                            </div>
                            <div class="cfg-row">
                                <label class="cfg-lbl"
                                    >Queue cap
                                    <input
                                        class="cfg-field"
                                        type="number"
                                        min="1"
                                        value={cfg.sim_params.queue_capacity ??
                                            ""}
                                        on:change={(e) =>
                                            updateBlockParam(
                                                b.id,
                                                "queue_capacity",
                                                parseInt(
                                                    e.currentTarget.value,
                                                ) || undefined,
                                            )}
                                    />
                                </label>
                                <label class="cfg-lbl"
                                    >Throughput/s
                                    <input
                                        class="cfg-field"
                                        type="number"
                                        min="0"
                                        value={cfg.sim_params
                                            .throughput_per_sec ?? ""}
                                        on:change={(e) =>
                                            updateBlockParam(
                                                b.id,
                                                "throughput_per_sec",
                                                parseFloat(
                                                    e.currentTarget.value,
                                                ) || undefined,
                                            )}
                                    />
                                </label>
                            </div>

                            <!-- Script -->
                            <div
                                class="cfg-section-label"
                                style="margin-top:6px"
                            >
                                Script
                                <label
                                    class="btn-import-py"
                                    title="Import a .py file"
                                >
                                    Import .py
                                    <input
                                        type="file"
                                        accept=".py,text/plain"
                                        style="display:none"
                                        on:change={(e) =>
                                            importBlockScript(b.id, e)}
                                    />
                                </label>
                            </div>
                            <div class="script-hint">
                                Available: <code>item</code>,
                                <code>params</code>, <code>env_now</code><br />
                                Set: <code>result["failed"]</code>,
                                <code>result["processing_time_ms"]</code>
                            </div>
                            <textarea
                                class="cfg-script"
                                rows="8"
                                value={cfg.sim_script}
                                on:input={(e) =>
                                    updateBlockScript(
                                        b.id,
                                        e.currentTarget.value,
                                    )}
                                placeholder="# Python per-item logic&#10;# e.g.:&#10;# import random&#10;# proc = params.get('processing_time_ms', 100)&#10;# result['processing_time_ms'] = proc * random.uniform(0.8, 1.2)&#10;# result['failed'] = random.random() < params.get('failure_rate', 0.01)"
                            ></textarea>
                            <div class="cfg-save-row">
                                <button
                                    class="btn-cfg-save"
                                    disabled={configSaving[b.id]}
                                    on:click={() => saveBlockConfig(b.id)}
                                    >{configSaving[b.id]
                                        ? "Saving…"
                                        : "Save Config"}</button
                                >
                            </div>
                        </div>
                    {/if}
                </div>
            {/each}
        {/if}
    </aside>

    <!-- ── Center: scenario builder ──────────────────────────────────────── -->
    <div class="sim-center">
        <div class="panel-hdr">
            <span class="panel-title">Scenario</span>
            <div class="hdr-actions">
                <button
                    class="btn-xs"
                    on:click={() => (activeScenario = makeEmptyScenario())}
                    >+ New</button
                >
                <button
                    class="btn-xs"
                    on:click={() => (scenarioListOpen = !scenarioListOpen)}
                >
                    Load ({scenarios.length})
                </button>
            </div>
        </div>

        {#if scenarioListOpen}
            <div class="scenario-list" transition:fade={{ duration: 120 }}>
                {#each scenarios as s (s.id)}
                    <button
                        class="scenario-item"
                        class:active={s.id === activeScenario.id}
                        on:click={() => selectScenario(s)}>{s.name}</button
                    >
                {/each}
                {#if scenarios.length === 0}
                    <div class="empty-hint">No saved scenarios</div>
                {/if}
            </div>
        {/if}

        <div class="field-group">
            <label class="field-label"
                >Name
                <input class="field" bind:value={activeScenario.name} />
            </label>
        </div>
        <div class="field-group">
            <label class="field-label"
                >Description
                <textarea
                    class="field"
                    rows="2"
                    bind:value={activeScenario.description}
                ></textarea>
            </label>
        </div>
        <div class="field-group">
            <label class="field-label"
                >Duration (ms)
                <input
                    class="field"
                    type="number"
                    min="100"
                    bind:value={activeScenario.duration_ms}
                />
            </label>
        </div>

        <div class="section-div">
            Stimulus Events
            <button class="btn-xs" on:click={addEvent}>+ Add</button>
        </div>

        {#each activeScenario.events as ev, i (i)}
            <div class="event-row">
                <input
                    class="field field-sm"
                    type="number"
                    min="0"
                    placeholder="ms"
                    value={ev.time_ms}
                    on:change={(e) =>
                        updateEvent(
                            i,
                            "time_ms",
                            parseFloat(e.currentTarget.value) || 0,
                        )}
                />
                <select
                    class="field field-sm"
                    value={ev.block_id}
                    on:change={(e) =>
                        updateEvent(i, "block_id", e.currentTarget.value)}
                >
                    {#each blockNodes as b}
                        <option value={b.id}>{b.name}</option>
                    {/each}
                </select>
                <input
                    class="field field-sm"
                    placeholder="signal"
                    value={ev.signal_type}
                    on:change={(e) =>
                        updateEvent(i, "signal_type", e.currentTarget.value)}
                />
                <button class="btn-del" on:click={() => removeEvent(i)}
                    >✕</button
                >
            </div>
        {/each}
        {#if activeScenario.events.length === 0}
            <div class="empty-hint">No events — add stimulus events above</div>
        {/if}

        <div class="sim-actions">
            <button class="btn-save" on:click={saveScenario}>Save</button>
            <button class="btn-run" disabled={running} on:click={runSimulation}>
                {running ? "Running…" : "▶ Run Simulation"}
            </button>
        </div>
    </div>

    <!-- ── Right: results viewer ──────────────────────────────────────────── -->
    <aside class="sim-right">
        <div class="panel-hdr">
            <span class="panel-title">Results</span>
            {#if lastResult}
                <span class="status-badge {statusClass(lastResult.status)}"
                    >{lastResult.status}</span
                >
            {/if}
        </div>

        {#if runError}
            <div class="error-box" transition:fade>{runError}</div>
        {/if}

        {#if running}
            <div class="running-row" transition:fade>
                <div class="spinner"></div>
                <span>Simulation running…</span>
            </div>
        {/if}

        {#if lastResult && !running}
            <!-- Block selector tabs -->
            <div class="result-tabs">
                <button
                    class="result-tab"
                    class:active={selectedResultBlock === null}
                    on:click={() => (selectedResultBlock = null)}>All</button
                >
                {#each blockNodes as b (b.id)}
                    {@const m = getMetric(b.id)}
                    {#if m}
                        <button
                            class="result-tab"
                            class:active={selectedResultBlock === b.id}
                            class:tab-warn={m.failures > 0}
                            on:click={() =>
                                (selectedResultBlock =
                                    selectedResultBlock === b.id ? null : b.id)}
                            >{b.name.length > 10
                                ? b.name.slice(0, 9) + "…"
                                : b.name}</button
                        >
                    {/if}
                {/each}
            </div>

            <!-- Summary metrics -->
            <div class="section-div">
                {selectedResultBlock
                    ? blockName(selectedResultBlock) + " Metrics"
                    : "All Block Metrics"}
            </div>

            {#each blockNodes as b (b.id)}
                {@const m = getMetric(b.id)}
                {#if m && (selectedResultBlock === null || selectedResultBlock === b.id)}
                    <div
                        class="metric-card"
                        class:metric-card-warn={m.failures > 0}
                    >
                        <div class="metric-card-hdr">
                            <span class="metric-name">{b.name}</span>
                            {#if m.failures > 0}
                                <span class="metric-fail-badge"
                                    >{m.failures} fail{m.failures === 1
                                        ? ""
                                        : "s"}</span
                                >
                            {/if}
                        </div>

                        <!-- Utilization bar -->
                        <div class="metric-row">
                            <span class="metric-lbl">Utilization</span>
                            <div class="bar-bg">
                                <div
                                    class="bar-fill {utilBar(m.utilization)}"
                                    style="width:{Math.min(
                                        100,
                                        m.utilization * 100,
                                    ).toFixed(1)}%"
                                ></div>
                            </div>
                            <span class="metric-val"
                                >{(m.utilization * 100).toFixed(1)}%</span
                            >
                        </div>

                        <div class="metric-grid">
                            <div class="metric-cell">
                                <span class="mcell-label">Processed</span>
                                <span class="mcell-value"
                                    >{m.processed_count}</span
                                >
                            </div>
                            <div class="metric-cell">
                                <span class="mcell-label">Failures</span>
                                <span
                                    class="mcell-value"
                                    class:val-fail={m.failures > 0}
                                    >{m.failures}</span
                                >
                            </div>
                            <div class="metric-cell">
                                <span class="mcell-label">Queue depth</span>
                                <span class="mcell-value">{m.queue_depth}</span>
                            </div>
                            {#if m.processed_count > 0}
                                <div class="metric-cell">
                                    <span class="mcell-label">Fail rate</span>
                                    <span
                                        class="mcell-value"
                                        class:val-fail={m.failures > 0}
                                    >
                                        {(
                                            (m.failures /
                                                (m.processed_count +
                                                    m.failures)) *
                                            100
                                        ).toFixed(1)}%
                                    </span>
                                </div>
                            {/if}
                        </div>
                    </div>
                {/if}
            {/each}

            <!-- Errors/warnings -->
            {#if lastResult.errors.length > 0}
                <div class="section-div">Warnings / Errors</div>
                {#each lastResult.errors as err}
                    <div class="err-msg">{err}</div>
                {/each}
            {/if}

            <!-- Timeline -->
            <div class="section-div">
                Timeline ({blockTimeline.length} event{blockTimeline.length ===
                1
                    ? ""
                    : "s"}
                {#if selectedResultBlock}
                    for {blockName(selectedResultBlock)}{/if})
            </div>
            <div class="timeline-scroll">
                {#each blockTimeline.slice(0, 300) as ev}
                    <div class="tl-event tl-{ev.event_type}">
                        <span class="tl-time"
                            >{ev.timestamp_ms.toFixed(0)}ms</span
                        >
                        {#if selectedResultBlock === null}
                            <span class="tl-block"
                                >{blockName(ev.block_id)}</span
                            >
                        {/if}
                        <span class="tl-type">{ev.event_type}</span>
                        <span class="tl-detail">{ev.detail}</span>
                    </div>
                {/each}
                {#if blockTimeline.length > 300}
                    <div class="tl-more">
                        …{blockTimeline.length - 300} more events
                    </div>
                {/if}
                {#if blockTimeline.length === 0}
                    <div class="empty-hint">
                        No timeline events for selection.
                    </div>
                {/if}
            </div>
        {:else if !running && !runError}
            <div class="empty-hint hint-center">
                Configure blocks, set up a scenario, and click Run Simulation.
            </div>
        {/if}
    </aside>
</div>

<style>
    .sim-page {
        display: flex;
        height: 100%;
        overflow: hidden;
        background: var(--surface-base);
    }

    /* ── Left panel (block configs) ─────────────────────────────────────── */
    .sim-left {
        width: 260px;
        flex-shrink: 0;
        border-right: 1px solid var(--surface-border);
        overflow-y: auto;
        display: flex;
        flex-direction: column;
    }

    .panel-hdr {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: var(--space-3) var(--space-4);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        flex-shrink: 0;
    }

    .panel-title {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
    }

    .panel-hint {
        font-size: var(--text-xs);
        color: var(--text-muted);
        opacity: 0.6;
    }

    .hdr-actions {
        display: flex;
        gap: var(--space-1);
    }

    /* ── Block config items ──────────────────────────────────────────────── */
    .block-cfg-item {
        border-bottom: 1px solid var(--surface-border-subtle);
    }

    .block-cfg-hdr {
        display: flex;
        align-items: center;
        gap: 6px;
        width: 100%;
        padding: var(--space-2) var(--space-3);
        background: none;
        border: none;
        cursor: pointer;
        font-family: var(--font-sans);
        transition: background var(--transition-fast);
    }
    .block-cfg-hdr:hover,
    .block-cfg-item.open .block-cfg-hdr {
        background: var(--surface-hover);
    }

    .block-cfg-name {
        flex: 1;
        text-align: left;
        font-size: var(--text-xs);
        color: var(--text-secondary);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .block-cfg-badges {
        display: flex;
        gap: 3px;
    }

    .badge-script {
        font-size: 9px;
        padding: 1px 5px;
        border-radius: 3px;
        background: #7c3aed20;
        color: #a78bfa;
        border: 1px solid #7c3aed40;
        font-family: var(--font-mono);
        font-weight: 700;
    }

    .badge-params {
        font-size: 10px;
        color: var(--text-muted);
    }

    .block-cfg-arrow {
        font-size: 9px;
        color: var(--text-muted);
        flex-shrink: 0;
    }

    .block-cfg-body {
        padding: var(--space-2) var(--space-3);
        background: var(--surface-base);
        border-top: 1px solid var(--surface-border-subtle);
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
    }

    .cfg-section-label {
        font-size: 10px;
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.07em;
        color: var(--text-muted);
        display: flex;
        align-items: center;
        justify-content: space-between;
    }

    .cfg-row {
        display: flex;
        gap: var(--space-2);
    }

    .cfg-lbl {
        flex: 1;
        font-size: 10px;
        color: var(--text-muted);
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .cfg-field {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-secondary);
        font-family: var(--font-sans);
        font-size: 11px;
        padding: 3px 5px;
        width: 100%;
        box-sizing: border-box;
    }

    .btn-import-py {
        font-size: 9px;
        padding: 2px 6px;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-muted);
        cursor: pointer;
        font-family: var(--font-sans);
        transition: all var(--transition-fast);
    }
    .btn-import-py:hover {
        color: #a78bfa;
        border-color: #7c3aed40;
    }

    .script-hint {
        font-size: 10px;
        color: var(--text-muted);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border-subtle);
        border-radius: var(--radius-sm);
        padding: 3px 6px;
        line-height: 1.5;
    }
    .script-hint code {
        font-family: var(--font-mono);
        color: #a78bfa;
    }

    .cfg-script {
        width: 100%;
        box-sizing: border-box;
        background: #0a0e1a;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: #c4b5fd;
        font-family: var(--font-mono);
        font-size: 11px;
        padding: 5px 7px;
        resize: vertical;
        min-height: 100px;
    }

    .cfg-save-row {
        display: flex;
        justify-content: flex-end;
        margin-top: 2px;
    }

    .btn-cfg-save {
        padding: 3px 10px;
        background: var(--accent-dim);
        border: 1px solid var(--accent-border);
        border-radius: var(--radius-sm);
        color: var(--accent-hover);
        font-size: 10px;
        font-weight: var(--weight-medium);
        cursor: pointer;
        font-family: var(--font-sans);
    }
    .btn-cfg-save:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    /* ── Center panel ────────────────────────────────────────────────────── */
    .sim-center {
        width: 260px;
        flex-shrink: 0;
        border-right: 1px solid var(--surface-border);
        overflow-y: auto;
        display: flex;
        flex-direction: column;
    }

    .btn-xs {
        font-size: var(--text-xs);
        padding: 2px 8px;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-muted);
        cursor: pointer;
        font-family: var(--font-sans);
    }

    .scenario-list {
        border-bottom: 1px solid var(--surface-border);
        max-height: 140px;
        overflow-y: auto;
    }

    .scenario-item {
        display: block;
        width: 100%;
        text-align: left;
        padding: var(--space-2) var(--space-4);
        background: none;
        border: none;
        font-size: var(--text-xs);
        color: var(--text-muted);
        cursor: pointer;
        font-family: var(--font-sans);
    }
    .scenario-item.active {
        color: var(--accent);
        background: var(--accent-dim);
    }

    .field-group {
        padding: var(--space-2) var(--space-3);
        display: flex;
        flex-direction: column;
        gap: 3px;
    }

    .field-label {
        font-size: var(--text-xs);
        color: var(--text-muted);
        font-weight: var(--weight-medium);
        display: flex;
        flex-direction: column;
        gap: 3px;
    }

    .field {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-secondary);
        font-family: var(--font-sans);
        font-size: var(--text-xs);
        padding: 4px 7px;
        width: 100%;
        box-sizing: border-box;
    }

    .section-div {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
        padding: var(--space-2) var(--space-3) var(--space-1);
        border-top: 1px solid var(--surface-border-subtle);
        margin-top: var(--space-1);
        display: flex;
        align-items: center;
        justify-content: space-between;
    }

    .event-row {
        display: flex;
        gap: 3px;
        padding: 2px var(--space-3);
        align-items: center;
    }

    .field-sm {
        font-size: 11px;
        padding: 3px 5px;
    }

    .btn-del {
        background: none;
        border: none;
        color: var(--color-error, #ef4444);
        cursor: pointer;
        font-size: var(--text-xs);
        padding: 2px 4px;
        flex-shrink: 0;
    }

    .sim-actions {
        display: flex;
        gap: var(--space-2);
        padding: var(--space-3);
        margin-top: auto;
        border-top: 1px solid var(--surface-border);
        flex-shrink: 0;
    }

    .btn-save {
        flex: 1;
        padding: var(--space-2);
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        font-family: var(--font-sans);
    }

    .btn-run {
        flex: 2;
        padding: var(--space-2);
        background: var(--accent-dim);
        border: 1px solid var(--accent-border);
        border-radius: var(--radius-md);
        color: var(--accent-hover);
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        cursor: pointer;
        font-family: var(--font-sans);
    }
    .btn-run:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    /* ── Right panel ─────────────────────────────────────────────────────── */
    .sim-right {
        flex: 1;
        overflow-y: auto;
        display: flex;
        flex-direction: column;
    }

    .status-badge {
        font-size: 10px;
        font-weight: var(--weight-semibold);
        padding: 2px 8px;
        border-radius: 99px;
    }

    .badge-ok {
        background: rgba(34, 197, 94, 0.12);
        color: #22c55e;
    }
    .badge-err {
        background: rgba(239, 68, 68, 0.1);
        color: #ef4444;
    }
    .badge-pending {
        background: rgba(234, 179, 8, 0.12);
        color: #eab308;
    }

    .error-box {
        margin: var(--space-3);
        padding: var(--space-3);
        background: rgba(239, 68, 68, 0.08);
        border: 1px solid rgba(239, 68, 68, 0.3);
        border-radius: var(--radius-md);
        font-size: var(--text-xs);
        color: #ef4444;
        word-break: break-word;
    }

    .running-row {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        padding: var(--space-3);
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    .spinner {
        width: 14px;
        height: 14px;
        border: 2px solid var(--surface-border);
        border-top-color: var(--accent);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
        flex-shrink: 0;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    /* Result tabs */
    .result-tabs {
        display: flex;
        flex-wrap: wrap;
        gap: 2px;
        padding: var(--space-2) var(--space-3);
        border-bottom: 1px solid var(--surface-border);
    }

    .result-tab {
        font-size: 10px;
        padding: 2px 8px;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: 99px;
        color: var(--text-muted);
        cursor: pointer;
        font-family: var(--font-sans);
        transition: all var(--transition-fast);
    }
    .result-tab:hover {
        color: var(--text-secondary);
    }
    .result-tab.active {
        background: var(--accent-dim);
        color: var(--accent-hover);
        border-color: var(--accent);
    }
    .result-tab.tab-warn {
        border-color: #ef444440;
        color: #f87171;
    }
    .result-tab.tab-warn.active {
        background: rgba(239, 68, 68, 0.1);
        border-color: #ef4444;
        color: #ef4444;
    }

    /* Metric cards */
    .metric-card {
        margin: var(--space-2) var(--space-3);
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        padding: var(--space-3);
    }
    .metric-card-warn {
        border-color: rgba(239, 68, 68, 0.3);
    }

    .metric-card-hdr {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: var(--space-2);
    }

    .metric-name {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        color: var(--text-secondary);
    }

    .metric-fail-badge {
        font-size: 10px;
        padding: 1px 6px;
        background: rgba(239, 68, 68, 0.12);
        border: 1px solid rgba(239, 68, 68, 0.3);
        border-radius: 99px;
        color: #ef4444;
    }

    .metric-row {
        display: flex;
        align-items: center;
        gap: 5px;
        font-size: 10px;
        color: var(--text-muted);
        margin-bottom: var(--space-2);
    }

    .metric-lbl {
        width: 60px;
        flex-shrink: 0;
    }

    .bar-bg {
        flex: 1;
        height: 5px;
        background: var(--surface-border);
        border-radius: 3px;
        overflow: hidden;
    }

    .bar-fill {
        height: 100%;
        border-radius: 3px;
        transition: width 0.5s ease;
    }
    .bar-ok {
        background: var(--accent);
    }
    .bar-warn {
        background: #f59e0b;
    }
    .bar-danger {
        background: #ef4444;
    }

    .metric-val {
        flex-shrink: 0;
        font-family: var(--font-mono);
        font-size: 11px;
        width: 42px;
        text-align: right;
    }

    .metric-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 4px;
    }

    .metric-cell {
        display: flex;
        flex-direction: column;
        gap: 1px;
        background: var(--surface-overlay);
        border-radius: var(--radius-sm);
        padding: 4px 6px;
    }

    .mcell-label {
        font-size: 9px;
        color: var(--text-muted);
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    .mcell-value {
        font-family: var(--font-mono);
        font-size: 13px;
        color: var(--text-secondary);
        font-weight: var(--weight-semibold);
    }

    .val-fail {
        color: #ef4444;
    }

    .err-msg {
        padding: 3px var(--space-3);
        font-size: 10px;
        color: #ef4444;
        font-family: var(--font-mono);
        word-break: break-all;
    }

    .timeline-scroll {
        overflow-y: auto;
        flex: 1;
        padding: 0 var(--space-3) var(--space-3);
    }

    .tl-event {
        display: flex;
        gap: 5px;
        align-items: baseline;
        padding: 2px 0;
        border-bottom: 1px solid var(--surface-border-subtle);
        font-size: 10px;
    }

    .tl-stimulus {
        color: var(--text-secondary);
    }
    .tl-processed {
        color: #22c55e;
    }
    .tl-failure {
        color: #ef4444;
    }

    .tl-time {
        width: 50px;
        flex-shrink: 0;
        font-family: var(--font-mono);
        color: var(--text-muted);
    }

    .tl-block {
        width: 70px;
        flex-shrink: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        color: var(--text-muted);
    }

    .tl-type {
        width: 58px;
        flex-shrink: 0;
    }

    .tl-detail {
        flex: 1;
        color: var(--text-muted);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .tl-more {
        font-size: 10px;
        color: var(--text-muted);
        padding: 4px 0;
        font-style: italic;
    }

    .empty-hint {
        font-size: var(--text-xs);
        color: var(--text-muted);
        padding: var(--space-2) var(--space-3);
        font-style: italic;
    }

    .hint-center {
        text-align: center;
        padding: var(--space-8) var(--space-4);
    }
</style>

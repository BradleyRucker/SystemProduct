<script lang="ts">
    /**
     * Properties panel shown on the right when a node or edge is selected.
     * Edits persist immediately on blur/change.
     */
    import { saveNode, nodes } from "$lib/store/model";
    import type {
        Node,
        Edge,
        EdgeKind,
        RequirementData,
        BlockData,
        PortData,
        TestCaseData,
        ValueTypeData,
        ConstraintBlockData,
        StateData,
        IRNode,
        SimParams,
    } from "$lib/types";
    import { createEventDispatcher } from "svelte";
    import { invoke } from "@tauri-apps/api/core";

    export let node: Node | null = null;
    export let edge: Edge | null = null;
    export let readOnly = false;
    export let irNode: IRNode | null = null; // live traceability data for the selected node

    const dispatch = createEventDispatcher<{
        updateEdge: Edge;
        deleteNode: string;
        deleteEdge: string;
    }>();

    const EDGE_KINDS: EdgeKind[] = [
        "satisfies",
        "refines",
        "allocates",
        "realizes",
        "traces",
        "verifies",
        "connects",
        "composes",
        "specializes",
    ];

    const VERIF_OPTIONS = [
        "analysis",
        "test",
        "inspection",
        "demonstration",
    ] as const;
    const PRIORITY_OPTIONS = ["shall", "should", "may"] as const;
    const STATUS_OPTIONS = ["draft", "approved", "obsolete"] as const;

    async function updateNode(field: string, value: unknown) {
        if (!node) return;
        const updated: Node = {
            ...node,
            [field]: value,
            modified_at: new Date().toISOString(),
        };
        node = updated;
        await saveNode(updated);
    }

    async function updateReqField(field: string, value: unknown) {
        if (!node || node.kind !== "requirement") return;
        const updated: Node = {
            ...node,
            data: { ...node.data, [field]: value },
            modified_at: new Date().toISOString(),
        };
        node = updated;
        await saveNode(updated);
    }

    async function updateBlockField(field: string, value: unknown) {
        if (!node || node.kind !== "block") return;
        const updated: Node = {
            ...node,
            data: { ...node.data, [field]: value },
            modified_at: new Date().toISOString(),
        };
        node = updated;
        await saveNode(updated);
    }

    function edgeMetaString(key: string): string {
        if (!edge) return "";
        const val = (edge.meta ?? {})[key];
        return typeof val === "string" ? val : "";
    }

    function edgeMetaList(key: string): string[] {
        if (!edge) return [];
        const val = (edge.meta ?? {})[key];
        return Array.isArray(val) ? (val as string[]) : [];
    }

    function parseCommaList(raw: string): string[] | undefined {
        const list = raw
            .split(",")
            .map((entry) => entry.trim())
            .filter(Boolean);
        return list.length ? list : undefined;
    }

    function updateEdgeMeta(key: string, value: unknown) {
        if (!edge) return;
        const meta = { ...edge.meta, [key]: value };
        dispatch("updateEdge", { ...edge, meta });
    }

    function composeTransitionLabel(
        trigger: string,
        guard: string,
        action: string,
    ): string {
        let lbl = trigger;
        if (guard) lbl += lbl ? ` [${guard}]` : `[${guard}]`;
        if (action) lbl += lbl ? ` / ${action}` : `/ ${action}`;
        return lbl;
    }

    function updateEdgeKind(value: string) {
        if (!edge) return;
        if (!EDGE_KINDS.includes(value as EdgeKind)) return;
        dispatch("updateEdge", { ...edge, kind: value as EdgeKind });
    }

    $: requirementOptions = $nodes.filter((n) => n.kind === "requirement");
    $: subsystemOptions = $nodes.filter(
        (n) =>
            n.kind === "block" &&
            !(n.meta as Record<string, unknown>)?.system_root,
    );

    function currentAllocation(data: RequirementData | null): string {
        return (data?.allocations ?? [])[0] ?? "";
    }

    async function setAllocation(value: string) {
        await updateReqField("allocations", value ? [value] : undefined);
    }

    function reqDataFor(node: Node): RequirementData {
        return node.data as RequirementData;
    }

    $: reqData =
        node?.kind === "requirement" ? (node.data as RequirementData) : null;
    $: blockData = node?.kind === "block" ? (node.data as BlockData) : null;
    $: tcData = node?.kind === "test_case" ? (node.data as TestCaseData) : null;
    $: portData = node?.kind === "port" ? (node.data as PortData) : null;
    $: vtData =
        node?.kind === "value_type" ? (node.data as ValueTypeData) : null;
    $: cbData =
        node?.kind === "constraint_block"
            ? (node.data as ConstraintBlockData)
            : null;
    $: stateData = node?.kind === "state" ? (node.data as StateData) : null;

    const VT_BASE_TYPES = [
        "Real",
        "Integer",
        "Boolean",
        "String",
        "Complex",
    ] as const;
    const PORT_DIRECTIONS = ["in", "out", "inout"] as const;
    const STATE_PSEUDO_KINDS = [
        "",
        "initial",
        "final",
        "choice",
        "fork",
        "join",
    ] as const;

    async function updatePortField(field: string, value: unknown) {
        if (!node || node.kind !== "port") return;
        const updated: Node = {
            ...node,
            data: { ...node.data, [field]: value },
            modified_at: new Date().toISOString(),
        };
        node = updated;
        await saveNode(updated);
    }

    async function updateVtField(field: string, value: unknown) {
        if (!node || node.kind !== "value_type") return;
        const updated: Node = {
            ...node,
            data: { ...node.data, [field]: value },
            modified_at: new Date().toISOString(),
        };
        node = updated;
        await saveNode(updated);
    }

    async function updateCbField(field: string, value: unknown) {
        if (!node || node.kind !== "constraint_block") return;
        const updated: Node = {
            ...node,
            data: { ...node.data, [field]: value },
            modified_at: new Date().toISOString(),
        };
        node = updated;
        await saveNode(updated);
    }

    async function updateStateField(field: string, value: unknown) {
        if (!node || node.kind !== "state") return;
        const updated: Node = {
            ...node,
            data: { ...node.data, [field]: value },
            modified_at: new Date().toISOString(),
        };
        node = updated;
        await saveNode(updated);
    }

    $: valueTypeOptions = $nodes.filter((n) => n.kind === "value_type");

    // irNode is consumed in the template — referenced here to suppress Svelte's
    // "unused export property" lint warning (it's a read-only input prop)
    $: _irNodeLinkedCount = irNode?.linked_req_ids.length ?? 0;

    // ── Simulation tab ────────────────────────────────────────────────────────
    type PropsTab = "properties" | "simulation";
    let activePropsTab: PropsTab = "properties";
    let simParams: SimParams = {};
    let simScript: string = "";
    let simSaving = false;
    let scriptSectionOpen = false;
    let simEditorExpanded = false;
    let simImportEl: HTMLInputElement | null = null;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let MonacoEditor: any = null;

    $: if (node?.kind !== "block") activePropsTab = "properties";

    async function onSimTabClick() {
        activePropsTab = "simulation";
        if (!MonacoEditor) {
            const mod = await import("$lib/components/MonacoEditor.svelte");
            MonacoEditor = mod.default;
        }
        if (node) await loadSimParams(node.id);
    }

    async function loadSimParams(nodeId: string) {
        try {
            const result = await invoke<{
                sim_params: SimParams | null;
                sim_script: string | null;
            }>("get_sim_params", { nodeId });
            simParams = result.sim_params ?? {};
            simScript = result.sim_script ?? "";
        } catch {
            simParams = {};
            simScript = "";
        }
    }

    async function saveSimParams() {
        if (!node || node.kind !== "block") return;
        simSaving = true;
        try {
            await invoke("save_sim_params", {
                nodeId: node.id,
                params: Object.keys(simParams).length > 0 ? simParams : null,
                script: simScript.trim() || null,
            });
        } catch (e) {
            console.error("Failed to save sim params:", e);
        } finally {
            simSaving = false;
        }
    }

    async function importScript(e: Event) {
        const input = e.target as HTMLInputElement;
        const file = input.files?.[0];
        if (!file) return;
        simScript = await file.text();
        scriptSectionOpen = true;
        // Reset input so same file can be re-selected
        input.value = "";
    }
</script>

<aside class="props-panel">
    <fieldset class="props-fieldset" disabled={readOnly}>
        {#if node}
            {#key node.id + "|" + node.modified_at}
                <div class="panel-header">
                    <span class="panel-title">Properties</span>
                    <span class="kind-badge kind-{node.kind}"
                        >{node.kind.replace("_", " ")}</span
                    >
                </div>

                {#if node.kind === "block"}
                    <div class="props-tab-bar">
                        <button
                            class="props-tab"
                            class:active={activePropsTab === "properties"}
                            on:click={() => (activePropsTab = "properties")}
                            >Properties</button
                        >
                        <button
                            class="props-tab"
                            class:active={activePropsTab === "simulation"}
                            on:click={onSimTabClick}>Simulation</button
                        >
                    </div>
                {/if}

                <!-- Name -->
                <div class="field-group">
                    <label class="field-label">
                        Name
                        <input
                            class="field"
                            value={node.name}
                            on:change={(e) =>
                                updateNode("name", e.currentTarget.value)}
                        />
                    </label>
                </div>

                <!-- Description -->
                <div class="field-group">
                    <label class="field-label">
                        Description
                        <textarea
                            class="field"
                            rows="3"
                            value={node.description}
                            on:change={(e) =>
                                updateNode(
                                    "description",
                                    e.currentTarget.value,
                                )}
                        ></textarea>
                    </label>
                </div>

                <!-- Requirement fields -->
                {#if reqData}
                    <div class="section-divider">Requirement</div>

                    <div class="field-group">
                        <label class="field-label">
                            ID
                            <input
                                class="field mono"
                                placeholder="REQ-001"
                                value={reqData.req_id ?? ""}
                                on:change={(e) =>
                                    updateReqField(
                                        "req_id",
                                        e.currentTarget.value,
                                    )}
                            />
                        </label>
                    </div>

                    <div class="field-group">
                        <label class="field-label">
                            Text
                            <textarea
                                class="field"
                                rows="4"
                                value={reqData.text ?? ""}
                                on:change={(e) =>
                                    updateReqField(
                                        "text",
                                        e.currentTarget.value,
                                    )}
                            ></textarea>
                        </label>
                    </div>

                    <div class="field-group">
                        <label class="field-label">
                            Rationale
                            <textarea
                                class="field"
                                rows="2"
                                value={reqData.rationale ?? ""}
                                on:change={(e) =>
                                    updateReqField(
                                        "rationale",
                                        e.currentTarget.value,
                                    )}
                            ></textarea>
                        </label>
                    </div>

                    <div class="field-row">
                        <div class="field-group">
                            <label class="field-label">
                                Priority
                                <select
                                    class="field"
                                    value={reqData.priority}
                                    on:change={(e) =>
                                        updateReqField(
                                            "priority",
                                            e.currentTarget.value,
                                        )}
                                >
                                    {#each PRIORITY_OPTIONS as p}
                                        <option value={p}>{p}</option>
                                    {/each}
                                </select>
                            </label>
                        </div>

                        <div class="field-group">
                            <label class="field-label">
                                Status
                                <select
                                    class="field"
                                    value={reqData.status}
                                    on:change={(e) =>
                                        updateReqField(
                                            "status",
                                            e.currentTarget.value,
                                        )}
                                >
                                    {#each STATUS_OPTIONS as s}
                                        <option value={s}>{s}</option>
                                    {/each}
                                </select>
                            </label>
                        </div>
                    </div>

                    <div class="field-group">
                        <label class="field-label">
                            Verification
                            <select
                                class="field"
                                value={reqData.verification_method ?? ""}
                                on:change={(e) =>
                                    updateReqField(
                                        "verification_method",
                                        e.currentTarget.value || undefined,
                                    )}
                            >
                                <option value="">-- none --</option>
                                {#each VERIF_OPTIONS as v}
                                    <option value={v}>{v}</option>
                                {/each}
                            </select>
                        </label>
                    </div>

                    <div class="field-group">
                        <label class="field-label">
                            Source
                            <input
                                class="field"
                                value={reqData.source ?? ""}
                                on:change={(e) =>
                                    updateReqField(
                                        "source",
                                        e.currentTarget.value,
                                    )}
                            />
                        </label>
                    </div>

                    <div class="field-group">
                        <label class="field-label">
                            Allocated Subsystem
                            <select
                                class="field"
                                value={currentAllocation(reqData)}
                                on:change={(e) =>
                                    setAllocation(e.currentTarget.value)}
                            >
                                <option value="">-- System Level --</option>
                                {#each subsystemOptions as sub (sub.id)}
                                    <option value={sub.name}>{sub.name}</option>
                                {/each}
                            </select>
                        </label>
                    </div>
                {/if}

                <!-- Block fields -->
                {#if blockData && activePropsTab === "properties"}
                    <div class="section-divider">Block</div>
                    <div class="field-group">
                        <label class="checkbox-label">
                            <input
                                type="checkbox"
                                checked={blockData.is_abstract}
                                on:change={(e) =>
                                    updateBlockField(
                                        "is_abstract",
                                        e.currentTarget.checked,
                                    )}
                            />
                            Abstract
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Multiplicity
                            <input
                                class="field mono"
                                placeholder="1, 0..*, 1..n"
                                value={blockData.multiplicity ?? ""}
                                on:change={(e) =>
                                    updateBlockField(
                                        "multiplicity",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </label>
                    </div>
                {/if}

                <!-- Simulation tab panel -->
                {#if blockData && activePropsTab === "simulation"}
                    <div class="section-divider">Declarative Parameters</div>

                    <div class="field-row">
                        <div class="field-group half">
                            <label class="field-label">
                                Processing Time (ms)
                                <input
                                    class="field"
                                    type="number"
                                    min="0"
                                    value={simParams.processing_time_ms ?? ""}
                                    on:change={(e) =>
                                        (simParams = {
                                            ...simParams,
                                            processing_time_ms:
                                                parseFloat(
                                                    e.currentTarget.value,
                                                ) || undefined,
                                        })}
                                />
                            </label>
                        </div>
                        <div class="field-group half">
                            <label class="field-label">
                                Failure Rate (0–1)
                                <input
                                    class="field"
                                    type="number"
                                    min="0"
                                    max="1"
                                    step="0.001"
                                    value={simParams.failure_rate ?? ""}
                                    on:change={(e) =>
                                        (simParams = {
                                            ...simParams,
                                            failure_rate:
                                                parseFloat(
                                                    e.currentTarget.value,
                                                ) || undefined,
                                        })}
                                />
                            </label>
                        </div>
                    </div>

                    <div class="field-row">
                        <div class="field-group half">
                            <label class="field-label">
                                Queue Capacity
                                <input
                                    class="field"
                                    type="number"
                                    min="1"
                                    value={simParams.queue_capacity ?? ""}
                                    on:change={(e) =>
                                        (simParams = {
                                            ...simParams,
                                            queue_capacity:
                                                parseInt(
                                                    e.currentTarget.value,
                                                ) || undefined,
                                        })}
                                />
                            </label>
                        </div>
                        <div class="field-group half">
                            <label class="field-label">
                                Throughput/sec
                                <input
                                    class="field"
                                    type="number"
                                    min="0"
                                    value={simParams.throughput_per_sec ?? ""}
                                    on:change={(e) =>
                                        (simParams = {
                                            ...simParams,
                                            throughput_per_sec:
                                                parseFloat(
                                                    e.currentTarget.value,
                                                ) || undefined,
                                        })}
                                />
                            </label>
                        </div>
                    </div>

                    <div class="field-group">
                        <label class="field-label">
                            Input Signal Type
                            <input
                                class="field"
                                value={simParams.input_signal_type ?? ""}
                                on:change={(e) =>
                                    (simParams = {
                                        ...simParams,
                                        input_signal_type:
                                            e.currentTarget.value || undefined,
                                    })}
                            />
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Output Signal Type
                            <input
                                class="field"
                                value={simParams.output_signal_type ?? ""}
                                on:change={(e) =>
                                    (simParams = {
                                        ...simParams,
                                        output_signal_type:
                                            e.currentTarget.value || undefined,
                                    })}
                            />
                        </label>
                    </div>

                    <!-- Script override collapsible -->
                    <div class="script-header-row">
                        <button
                            class="script-toggle"
                            on:click={() =>
                                (scriptSectionOpen = !scriptSectionOpen)}
                        >
                            Script Override {scriptSectionOpen ? "▲" : "▼"}
                            {#if simScript.trim()}<span
                                    class="script-active-dot"
                                    title="Script active"
                                ></span>{/if}
                        </button>
                        <button
                            class="btn-import-script"
                            title="Import a .py file as the block script"
                            on:click={() => simImportEl?.click()}
                            >Import .py</button
                        >
                        <input
                            type="file"
                            accept=".py,text/x-python,text/plain"
                            style="display:none"
                            bind:this={simImportEl}
                            on:change={importScript}
                        />
                    </div>

                    {#if scriptSectionOpen}
                        <div class="field-group">
                            <div class="script-context-hint">
                                <code>item</code>, <code>params</code>,
                                <code>env_now</code>
                                → set <code>result["failed"]</code>,
                                <code>result["processing_time_ms"]</code>
                            </div>
                            {#if MonacoEditor}
                                <svelte:component
                                    this={MonacoEditor}
                                    value={simScript}
                                    language="python"
                                    height="220px"
                                    on:change={(e) => (simScript = e.detail)}
                                />
                            {:else}
                                <textarea
                                    class="field script-textarea"
                                    rows="10"
                                    bind:value={simScript}
                                    placeholder="# Python per-item script&#10;# item: the signal dict&#10;# params: block sim_params dict&#10;# env_now: current sim time (ms)&#10;# result: dict to set failed/processing_time_ms"
                                ></textarea>
                            {/if}
                            <div class="script-btn-row">
                                <button
                                    class="btn-expand"
                                    on:click={() => (simEditorExpanded = true)}
                                    >Expand</button
                                >
                                <button
                                    class="btn-clear-script"
                                    on:click={() => {
                                        simScript = "";
                                    }}>Clear</button
                                >
                            </div>
                        </div>
                    {/if}

                    <div class="field-group">
                        <button
                            class="btn-sim-save"
                            disabled={simSaving}
                            on:click={saveSimParams}
                        >
                            {simSaving ? "Saving…" : "Save Simulation Config"}
                        </button>
                    </div>
                {/if}

                <!-- Test case fields -->
                {#if tcData}
                    <div class="section-divider">Test Case</div>
                    <div class="field-group">
                        <label class="field-label">
                            Status
                            <select
                                class="field"
                                value={tcData.status}
                                on:change={(e) =>
                                    node &&
                                    updateNode("data", {
                                        ...node.data,
                                        status: e.currentTarget.value,
                                    })}
                            >
                                <option value="not_run">Not run</option>
                                <option value="pass">Pass</option>
                                <option value="fail">Fail</option>
                            </select>
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Procedure
                            <textarea
                                class="field"
                                rows="3"
                                value={tcData.procedure ?? ""}
                                on:change={(e) =>
                                    node &&
                                    updateNode("data", {
                                        ...node.data,
                                        procedure: e.currentTarget.value,
                                    })}
                            ></textarea>
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Expected result
                            <textarea
                                class="field"
                                rows="2"
                                value={tcData.expected ?? ""}
                                on:change={(e) =>
                                    node &&
                                    updateNode("data", {
                                        ...node.data,
                                        expected: e.currentTarget.value,
                                    })}
                            ></textarea>
                        </label>
                    </div>
                {/if}

                <!-- Port fields -->
                {#if portData}
                    <div class="section-divider">Port</div>
                    <div class="field-group">
                        <label class="field-label">
                            Direction
                            <select
                                class="field"
                                value={portData.direction}
                                on:change={(e) =>
                                    updatePortField(
                                        "direction",
                                        e.currentTarget.value,
                                    )}
                            >
                                {#each PORT_DIRECTIONS as d}
                                    <option value={d}>{d}</option>
                                {/each}
                            </select>
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Type
                            <input
                                class="field"
                                list="vt-type-options"
                                placeholder="Real, Voltage, ImageFrame…"
                                value={portData.type_name ?? ""}
                                on:change={(e) =>
                                    updatePortField(
                                        "type_name",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                            <datalist id="vt-type-options">
                                {#each valueTypeOptions as vt}
                                    <option value={vt.name}>{vt.name}</option>
                                {/each}
                            </datalist>
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Multiplicity
                            <input
                                class="field mono"
                                placeholder="1, 0..*, 1..n"
                                value={portData.multiplicity ?? ""}
                                on:change={(e) =>
                                    updatePortField(
                                        "multiplicity",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </label>
                    </div>
                {/if}

                <!-- ValueType fields -->
                {#if vtData}
                    <div class="section-divider">Value Type</div>
                    <div class="field-group">
                        <label class="field-label">
                            Base Type
                            <select
                                class="field"
                                value={vtData.base_type ?? ""}
                                on:change={(e) =>
                                    updateVtField(
                                        "base_type",
                                        e.currentTarget.value || undefined,
                                    )}
                            >
                                <option value="">-- select --</option>
                                {#each VT_BASE_TYPES as t}
                                    <option value={t}>{t}</option>
                                {/each}
                            </select>
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Unit
                            <input
                                class="field mono"
                                placeholder="m/s, kg, V, A, °C…"
                                value={vtData.unit ?? ""}
                                on:change={(e) =>
                                    updateVtField(
                                        "unit",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Constraint
                            <input
                                class="field mono"
                                placeholder="0.0 <= x <= 100.0"
                                value={vtData.constraint ?? ""}
                                on:change={(e) =>
                                    updateVtField(
                                        "constraint",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </label>
                    </div>
                {/if}

                <!-- ConstraintBlock fields -->
                {#if cbData}
                    <div class="section-divider">Constraint Block</div>
                    <div class="field-group">
                        <label class="field-label">
                            Expression
                            <textarea
                                class="field mono"
                                rows="3"
                                placeholder="P = V * I"
                                value={cbData.expression ?? ""}
                                on:change={(e) =>
                                    updateCbField(
                                        "expression",
                                        e.currentTarget.value || undefined,
                                    )}
                            ></textarea>
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Parameters (comma-separated)
                            <input
                                class="field mono"
                                placeholder="P, V, I"
                                value={(cbData.parameters ?? []).join(", ")}
                                on:change={(e) => {
                                    const list = e.currentTarget.value
                                        .split(",")
                                        .map((s) => s.trim())
                                        .filter(Boolean);
                                    updateCbField(
                                        "parameters",
                                        list.length ? list : undefined,
                                    );
                                }}
                            />
                        </label>
                    </div>
                {/if}

                <!-- State fields -->
                {#if stateData}
                    <div class="section-divider">State</div>
                    <div class="field-group">
                        <label class="field-label">
                            Pseudo-state Kind
                            <select
                                class="field"
                                value={stateData.pseudo_kind ?? ""}
                                on:change={(e) =>
                                    updateStateField(
                                        "pseudo_kind",
                                        e.currentTarget.value || undefined,
                                    )}
                            >
                                <option value="">Normal state</option>
                                <option value="initial">Initial</option>
                                <option value="final">Final</option>
                                <option value="choice">Choice</option>
                                <option value="fork">Fork</option>
                                <option value="join">Join</option>
                            </select>
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Entry / [guard] / effect
                            <input
                                class="field"
                                placeholder="entry / action"
                                value={stateData.entry_action ?? ""}
                                on:change={(e) =>
                                    updateStateField(
                                        "entry_action",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Exit action
                            <input
                                class="field"
                                placeholder="exit / action"
                                value={stateData.exit_action ?? ""}
                                on:change={(e) =>
                                    updateStateField(
                                        "exit_action",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </label>
                    </div>
                    <div class="field-group">
                        <label class="field-label">
                            Do activity
                            <input
                                class="field"
                                placeholder="do / activity"
                                value={stateData.do_activity ?? ""}
                                on:change={(e) =>
                                    updateStateField(
                                        "do_activity",
                                        e.currentTarget.value || undefined,
                                    )}
                            />
                        </label>
                    </div>
                {/if}

                <!-- Live traceability health (shown when irNode data is available) -->
                {#if irNode}
                    <div class="section-divider">Traceability</div>

                    <!-- Coverage status pill -->
                    {#if irNode.coverage_status !== "n/a"}
                        <div class="field-group trace-status-row">
                            <span class="trace-label">Coverage</span>
                            <span
                                class="trace-pill coverage-{irNode.coverage_status}"
                            >
                                {#if irNode.coverage_status === "full"}✓ Covered
                                {:else if irNode.coverage_status === "partial"}~
                                    Partial
                                {:else}✗ Not covered{/if}
                            </span>
                        </div>
                    {/if}

                    <!-- Linked requirements (for blocks) -->
                    {#if irNode.linked_req_ids.length > 0}
                        <div class="field-group">
                            <span class="trace-label"
                                >Satisfied by {irNode.linked_req_ids.length} requirement{irNode
                                    .linked_req_ids.length === 1
                                    ? ""
                                    : "s"}</span
                            >
                            <div class="linked-reqs">
                                {#each irNode.linked_req_ids as reqId}
                                    {@const reqNode = $nodes.find(
                                        (n) => n.id === reqId,
                                    )}
                                    {#if reqNode}
                                        {@const rd = reqDataFor(reqNode)}
                                        <div
                                            class="linked-req-chip"
                                            class:req-approved={rd.status ===
                                                "approved"}
                                            class:req-draft={rd.status ===
                                                "draft"}
                                        >
                                            <span class="req-id-mono"
                                                >{rd.req_id ?? "—"}</span
                                            >
                                            <span class="req-name-trunc"
                                                >{reqNode.name}</span
                                            >
                                            <span
                                                class="req-status-dot status-dot-{rd.status}"
                                            ></span>
                                        </div>
                                    {/if}
                                {/each}
                            </div>
                        </div>
                    {:else if irNode.kind === "block"}
                        <div class="field-group">
                            <span class="trace-label trace-warn"
                                >No requirements satisfy this block</span
                            >
                        </div>
                    {/if}

                    <!-- Stats row -->
                    <div class="field-group trace-stats-row">
                        {#if irNode.satisfies_count > 0}
                            <span class="trace-stat"
                                >◈ {irNode.satisfies_count} req{irNode.satisfies_count ===
                                1
                                    ? ""
                                    : "s"}</span
                            >
                        {/if}
                        {#if irNode.verifies_count > 0}
                            <span class="trace-stat"
                                >◧ {irNode.verifies_count} test{irNode.verifies_count ===
                                1
                                    ? ""
                                    : "s"}</span
                            >
                        {/if}
                        {#if irNode.open_comments > 0}
                            <span class="trace-stat trace-stat-comment"
                                >💬 {irNode.open_comments}</span
                            >
                        {/if}
                        {#if irNode.has_suspect}
                            <span class="trace-stat trace-stat-suspect"
                                >⚠ Suspect</span
                            >
                        {/if}
                    </div>
                {/if}

                <div class="delete-zone">
                    <button
                        class="btn-delete"
                        on:click={() => node && dispatch("deleteNode", node.id)}
                    >
                        Delete element
                    </button>
                </div>
            {/key}
        {:else if edge}
            <div class="panel-header">
                <span class="panel-title">Properties</span>
                <span class="kind-badge">relationship</span>
            </div>

            <div class="field-group">
                <label class="field-label">
                    Kind
                    <select
                        class="field"
                        value={edge.kind}
                        on:change={(e) => updateEdgeKind(e.currentTarget.value)}
                    >
                        {#each EDGE_KINDS as k}
                            <option value={k}>&lt;&lt;{k}&gt;&gt;</option>
                        {/each}
                    </select>
                </label>
            </div>

            <div class="field-group">
                <label class="field-label">
                    Label
                    <input
                        class="field"
                        value={edge.label}
                        on:change={(e) =>
                            edge &&
                            dispatch("updateEdge", {
                                ...edge,
                                label: e.currentTarget.value,
                            })}
                    />
                </label>
            </div>

            {#if edge.kind === "transition"}
                <div class="section-divider">Transition</div>
                <p class="field-hint">
                    Fields below compose the label as <code
                        >trigger [guard] / action</code
                    >.
                </p>

                <div class="field-group">
                    <label class="field-label">
                        Trigger / Event
                        <input
                            class="field"
                            placeholder="buttonPress, timeout…"
                            value={edge.meta?.trigger ?? ""}
                            on:change={(e) => {
                                if (!edge) return;
                                const trigger = e.currentTarget.value.trim();
                                const guard = String(edge.meta?.guard ?? "");
                                const action = String(edge.meta?.action ?? "");
                                const lbl = composeTransitionLabel(
                                    trigger,
                                    guard,
                                    action,
                                );
                                dispatch("updateEdge", {
                                    ...edge,
                                    label: lbl,
                                    meta: { ...edge.meta, trigger },
                                });
                            }}
                        />
                    </label>
                </div>

                <div class="field-group">
                    <label class="field-label">
                        Guard condition
                        <input
                            class="field"
                            placeholder="x > 0, mode == IDLE…"
                            value={edge.meta?.guard ?? ""}
                            on:change={(e) => {
                                if (!edge) return;
                                const guard = e.currentTarget.value.trim();
                                const trigger = String(
                                    edge.meta?.trigger ?? "",
                                );
                                const action = String(edge.meta?.action ?? "");
                                const lbl = composeTransitionLabel(
                                    trigger,
                                    guard,
                                    action,
                                );
                                dispatch("updateEdge", {
                                    ...edge,
                                    label: lbl,
                                    meta: { ...edge.meta, guard },
                                });
                            }}
                        />
                    </label>
                </div>

                <div class="field-group">
                    <label class="field-label">
                        Action / Effect
                        <input
                            class="field"
                            placeholder="startTimer(), send(msg)…"
                            value={edge.meta?.action ?? ""}
                            on:change={(e) => {
                                if (!edge) return;
                                const action = e.currentTarget.value.trim();
                                const trigger = String(
                                    edge.meta?.trigger ?? "",
                                );
                                const guard = String(edge.meta?.guard ?? "");
                                const lbl = composeTransitionLabel(
                                    trigger,
                                    guard,
                                    action,
                                );
                                dispatch("updateEdge", {
                                    ...edge,
                                    label: lbl,
                                    meta: { ...edge.meta, action },
                                });
                            }}
                        />
                    </label>
                </div>
            {/if}

            {#if edge.kind === "connects" || edge.kind === "composes"}
                <div class="section-divider">Interconnect</div>

                <div class="field-group">
                    <label class="field-label">
                        Provides
                        <input
                            class="field"
                            placeholder="Signals, power, data..."
                            value={edgeMetaString("provides")}
                            on:change={(e) =>
                                updateEdgeMeta(
                                    "provides",
                                    e.currentTarget.value,
                                )}
                        />
                    </label>
                </div>

                <div class="field-group">
                    <label class="field-label">
                        Receives
                        <input
                            class="field"
                            placeholder="Inputs, dependencies..."
                            value={edgeMetaString("receives")}
                            on:change={(e) =>
                                updateEdgeMeta(
                                    "receives",
                                    e.currentTarget.value,
                                )}
                        />
                    </label>
                </div>

                <div class="field-group">
                    <label class="field-label">
                        Satisfies requirements
                        <input
                            class="field"
                            list="req-options"
                            placeholder="REQ-001, REQ-014"
                            value={edgeMetaList("satisfies").join(", ")}
                            on:change={(e) =>
                                updateEdgeMeta(
                                    "satisfies",
                                    parseCommaList(e.currentTarget.value),
                                )}
                        />
                    </label>
                    <datalist id="req-options">
                        {#each requirementOptions as req}
                            {@const data = reqDataFor(req)}
                            <option value={data.req_id ?? req.name}>
                                {data.req_id ?? "REQ-?"} - {req.name}
                            </option>
                        {/each}
                    </datalist>
                </div>
            {/if}

            <div class="delete-zone">
                <button
                    class="btn-delete"
                    on:click={() => edge && dispatch("deleteEdge", edge.id)}
                >
                    Delete relationship
                </button>
            </div>
        {:else}
            <div class="empty-props empty-state">
                <div
                    class="empty-state-icon props-empty-icon"
                    aria-hidden="true"
                >
                    <svg viewBox="0 0 24 24" fill="none">
                        <rect
                            x="3.5"
                            y="4.5"
                            width="17"
                            height="15"
                            rx="2.5"
                            stroke="currentColor"
                            stroke-width="1.6"
                        ></rect>
                        <path
                            d="M8 9h8M8 12h8M8 15h5"
                            stroke="currentColor"
                            stroke-width="1.6"
                            stroke-linecap="round"
                        ></path>
                    </svg>
                </div>
                <div class="empty-state-title">Select an element</div>
                <div class="empty-state-body">
                    Pick a node or relationship to review and edit details.
                </div>
            </div>
        {/if}
    </fieldset>
</aside>

<!-- Full-screen script editor modal -->
{#if simEditorExpanded}
    <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
    <div
        class="sim-modal-backdrop"
        on:click={() => (simEditorExpanded = false)}
    >
        <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
        <div class="sim-modal-content" on:click|stopPropagation>
            <div class="sim-modal-header">
                <span>Script Override — {node?.name ?? ""}</span>
                <button on:click={() => (simEditorExpanded = false)}
                    >Close</button
                >
            </div>
            {#if MonacoEditor}
                <svelte:component
                    this={MonacoEditor}
                    value={simScript}
                    language="python"
                    height="calc(100vh - 120px)"
                    on:change={(e) => (simScript = e.detail)}
                />
            {/if}
        </div>
    </div>
{/if}

<style>
    .props-panel {
        width: 300px;
        height: 100%;
        background: linear-gradient(
            180deg,
            var(--surface-overlay) 0%,
            var(--surface-raised) 64%
        );
        border-left: 1px solid var(--surface-border);
        box-shadow: inset 1px 0 0 #ffffff0a;
        display: flex;
        flex-direction: column;
        overflow-y: auto;
        flex-shrink: 0;
    }

    .props-fieldset {
        border: none;
        padding: 0;
        margin: 0;
        min-height: 100%;
    }

    .panel-header {
        position: sticky;
        top: 0;
        z-index: 2;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-2);
        padding: var(--space-3) var(--space-4);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        box-shadow: 0 1px 0 #00000050;
    }

    .panel-title {
        font-size: var(--text-xs);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
        font-weight: var(--weight-semibold);
    }

    .kind-badge {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.06em;
        padding: 2px 8px;
        border-radius: 99px;
        background: var(--surface-overlay);
        color: var(--text-secondary);
    }

    .kind-badge.kind-requirement {
        background: var(--color-requirement-bg);
        color: var(--color-requirement);
    }
    .kind-badge.kind-block {
        background: var(--color-block-bg);
        color: var(--color-block);
    }
    .kind-badge.kind-interface {
        background: var(--color-interface-bg);
        color: var(--color-interface);
    }
    .kind-badge.kind-test_case {
        background: var(--color-test-case-bg);
        color: var(--color-test-case);
    }
    .kind-badge.kind-use_case {
        background: var(--color-use-case-bg);
        color: var(--color-use-case);
    }
    .kind-badge.kind-value_type {
        background: #ffedd5;
        color: #ea580c;
    }
    .kind-badge.kind-constraint_block {
        background: #ede9fe;
        color: #7c3aed;
    }
    .kind-badge.kind-state {
        background: #dcfce7;
        color: #16a34a;
    }

    .field-hint {
        font-size: var(--text-xs);
        color: var(--text-muted);
        line-height: 1.5;
        padding: 0 var(--space-4);
        margin: 0;
    }
    .field-hint code {
        font-family: var(--font-mono, monospace);
        font-size: 10px;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: 3px;
        padding: 1px 4px;
        color: var(--text-secondary);
    }

    .section-divider {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--text-muted);
        padding: var(--space-3) var(--space-4) var(--space-1);
        border-top: 1px solid var(--surface-border-subtle);
        margin-top: var(--space-2);
        background: linear-gradient(
            90deg,
            var(--surface-border-subtle) 0%,
            transparent 90%
        );
    }

    .field-group {
        padding: var(--space-2) var(--space-4);
        display: flex;
        flex-direction: column;
        gap: 4px;
    }

    .field-row {
        display: flex;
        gap: 0;
    }
    .field-row .field-group {
        flex: 1;
        padding-right: var(--space-2);
    }

    .field-label {
        font-size: var(--text-xs);
        color: var(--text-muted);
        font-weight: var(--weight-medium);
    }

    .field.mono {
        font-family: var(--font-mono);
        font-size: var(--text-xs);
    }

    .checkbox-label {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        font-size: var(--text-sm);
        color: var(--text-secondary);
        cursor: pointer;
    }

    .delete-zone {
        margin-top: auto;
        padding: var(--space-4);
        border-top: 1px solid var(--surface-border);
        flex-shrink: 0;
        background: var(--surface-overlay);
    }

    .btn-delete {
        width: 100%;
        padding: var(--space-2) var(--space-3);
        background: #ef444415;
        border: 1px solid #ef444440;
        border-radius: var(--radius-md);
        color: var(--color-error);
        font-size: var(--text-sm);
        font-family: var(--font-sans);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .btn-delete:hover {
        background: #ef444430;
        border-color: var(--color-error);
    }

    .empty-props {
        margin: var(--space-4);
        min-height: 280px;
    }
    .props-empty-icon svg {
        width: 100%;
        height: 100%;
    }

    /* ── Live Traceability Health ─────────────────────────────────────── */
    .trace-status-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        flex-direction: row;
        gap: var(--space-2);
    }
    .trace-label {
        font-size: var(--text-xs);
        color: var(--text-muted);
        font-weight: var(--weight-medium);
    }
    .trace-warn {
        color: var(--color-warning);
    }
    .trace-pill {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        padding: 2px 8px;
        border-radius: 99px;
        border: 1px solid;
    }
    .coverage-full {
        background: rgba(34, 197, 94, 0.12);
        color: #22c55e;
        border-color: rgba(34, 197, 94, 0.35);
    }
    .coverage-partial {
        background: rgba(234, 179, 8, 0.12);
        color: #eab308;
        border-color: rgba(234, 179, 8, 0.35);
    }
    .coverage-none {
        background: rgba(239, 68, 68, 0.1);
        color: #ef4444;
        border-color: rgba(239, 68, 68, 0.35);
    }

    .linked-reqs {
        display: flex;
        flex-direction: column;
        gap: 3px;
        margin-top: 4px;
    }
    .linked-req-chip {
        display: flex;
        align-items: center;
        gap: 5px;
        padding: 3px 6px;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border-bright);
        border-radius: var(--radius-sm);
        font-size: var(--text-xs);
        overflow: hidden;
    }
    .linked-req-chip.req-approved {
        border-color: rgba(34, 197, 94, 0.3);
    }
    .linked-req-chip.req-draft {
        border-color: rgba(234, 179, 8, 0.3);
    }
    .req-id-mono {
        font-family: var(--font-mono);
        color: var(--text-muted);
        white-space: nowrap;
        flex-shrink: 0;
    }
    .req-name-trunc {
        color: var(--text-secondary);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        flex: 1;
    }
    .req-status-dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        flex-shrink: 0;
    }
    .status-dot-approved {
        background: #22c55e;
    }
    .status-dot-draft {
        background: #eab308;
    }
    .status-dot-obsolete {
        background: #64748b;
    }

    .trace-stats-row {
        display: flex;
        flex-direction: row;
        flex-wrap: wrap;
        gap: 6px;
        padding-top: var(--space-1);
    }
    .trace-stat {
        font-size: var(--text-xs);
        color: var(--text-muted);
        background: var(--surface-overlay);
        padding: 2px 7px;
        border-radius: 99px;
        border: 1px solid var(--surface-border);
    }
    .trace-stat-comment {
        color: #60a5fa;
        border-color: rgba(96, 165, 250, 0.3);
    }
    .trace-stat-suspect {
        color: #eab308;
        border-color: rgba(234, 179, 8, 0.3);
    }

    /* ── Simulation tab ────────────────────────────────────────────────────── */

    .props-tab-bar {
        display: flex;
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        flex-shrink: 0;
    }

    .props-tab {
        flex: 1;
        padding: var(--space-2) var(--space-3);
        background: none;
        border: none;
        border-bottom: 2px solid transparent;
        color: var(--text-muted);
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        font-family: var(--font-sans);
        cursor: pointer;
        transition: all var(--transition-fast);
    }

    .props-tab.active {
        color: var(--accent);
        border-bottom-color: var(--accent);
    }

    .field-row {
        display: flex;
        gap: var(--space-2);
        padding: 0 var(--space-3);
    }

    .half {
        flex: 1;
        padding: var(--space-2) 0;
    }

    .script-header-row {
        display: flex;
        align-items: center;
        gap: 0;
        border-top: 1px solid var(--surface-border-subtle);
    }

    .script-toggle {
        flex: 1;
        text-align: left;
        padding: var(--space-2) var(--space-3);
        background: none;
        border: none;
        color: var(--text-muted);
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: 0.08em;
        cursor: pointer;
        font-family: var(--font-sans);
        display: flex;
        align-items: center;
        gap: 6px;
    }

    .script-active-dot {
        width: 6px;
        height: 6px;
        border-radius: 50%;
        background: var(--color-success, #22c55e);
        flex-shrink: 0;
    }

    .btn-import-script {
        flex-shrink: 0;
        padding: 3px 8px;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-muted);
        font-size: 10px;
        cursor: pointer;
        font-family: var(--font-sans);
        margin-right: var(--space-2);
        transition: all var(--transition-fast);
    }
    .btn-import-script:hover {
        color: var(--text-secondary);
        border-color: var(--surface-border-bright);
    }

    .script-context-hint {
        font-size: 10px;
        color: var(--text-muted);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border-subtle);
        border-radius: var(--radius-sm);
        padding: 4px 8px;
        margin-bottom: 4px;
        line-height: 1.6;
    }
    .script-context-hint code {
        font-family: var(--font-mono);
        color: #a78bfa;
        background: #7c3aed12;
        padding: 0 3px;
        border-radius: 2px;
    }

    .script-btn-row {
        display: flex;
        gap: 4px;
        margin-top: 4px;
    }

    .script-textarea {
        font-family: var(--font-mono);
        font-size: 12px;
        resize: vertical;
        min-height: 160px;
    }

    .btn-expand {
        padding: 3px 10px;
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        font-family: var(--font-sans);
    }

    .btn-clear-script {
        padding: 3px 10px;
        background: transparent;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--color-error, #ef4444);
        font-size: var(--text-xs);
        cursor: pointer;
        font-family: var(--font-sans);
        opacity: 0.7;
    }
    .btn-clear-script:hover {
        opacity: 1;
    }

    .btn-sim-save {
        width: 100%;
        padding: var(--space-2) var(--space-3);
        background: var(--accent-dim);
        border: 1px solid var(--accent-border);
        border-radius: var(--radius-md);
        color: var(--accent-hover);
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        font-family: var(--font-sans);
        cursor: pointer;
        transition: all var(--transition-fast);
    }

    .btn-sim-save:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    /* Full-screen modal (rendered outside aside via svelte portal-like placement) */
    .sim-modal-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.72);
        z-index: 500;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .sim-modal-content {
        width: 80vw;
        max-width: 1200px;
        background: var(--surface-raised);
        border-radius: var(--radius-lg);
        border: 1px solid var(--surface-border);
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }

    .sim-modal-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: var(--space-3) var(--space-4);
        border-bottom: 1px solid var(--surface-border);
        font-size: var(--text-sm);
        color: var(--text-secondary);
        flex-shrink: 0;
    }

    .sim-modal-header button {
        padding: 3px 10px;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        font-family: var(--font-sans);
    }
</style>

<script lang="ts">
    import { onMount } from "svelte";
    import { page } from "$app/stores";
    import { invoke } from "@tauri-apps/api/core";
    import {
        nodes,
        edges,
        saveNode,
        saveEdge,
        loadProject,
    } from "$lib/store/model";
    import type { Node, Edge, SubsystemArtifact } from "$lib/types";
    import { v4 as uuidv4 } from "uuid";
    import {
        Github,
        Save,
        Zap,
        RefreshCw,
        Eye,
        EyeOff,
        CheckCircle2,
        XCircle,
        Loader,
        Puzzle,
        BrainCircuit,
        Download,
        FileCode2,
        FileJson,
        FileText,
    } from "lucide-svelte";

    $: projectId = $page.params.id;

    let githubToken = "";
    let githubOwner = "";
    let githubRepo = "";
    let saving = false;
    let saved = false;
    let testing = false;
    let testResult: { ok: boolean; message: string } | null = null;
    let syncing = false;
    let syncResult: { ok: boolean; message: string } | null = null;
    let lastLinkCount = 0;
    let showToken = false;

    // ── Anthropic AI ───────────────────────────────────────────────────────────
    let anthropicKey = "";
    // ── Ollama ─────────────────────────────────────────────────────────────────
    let ollamaModel = "qwen2.5:7b";
    let ollamaBaseUrl = "http://localhost:11434";
    let savingOllama = false;
    let savedOllama = false;
    let ollamaConfigured = false;
    let ollamaTesting = false;
    let ollamaTestResult: { ok: boolean; message: string } | null = null;
    let showAnthropicKey = false;
    let savingAi = false;
    let savedAi = false;
    let aiConfigured = false;

    $: externalNodes = $nodes.filter((n) => n.kind === "external");

    async function loadSettings() {
        githubToken =
            (await invoke<string | null>("get_setting", {
                key: "integration.github.token",
                project_id: projectId,
            })) ?? "";
        githubOwner =
            (await invoke<string | null>("get_setting", {
                key: "integration.github.owner",
                project_id: projectId,
            })) ?? "";
        githubRepo =
            (await invoke<string | null>("get_setting", {
                key: "integration.github.repo",
                project_id: projectId,
            })) ?? "";

        // Load AI key — stored as a masked placeholder if set
        aiConfigured = await invoke<boolean>("ai_available").catch(() => false);
        // We never load the raw key back into the field — show placeholder if set
        anthropicKey = "";

        // Load Ollama config
        const ollamaStatus = await invoke<{
            reachable: boolean;
            model: string;
            base_url: string;
            is_active: boolean;
        }>("ollama_status").catch(() => null);
        if (ollamaStatus) {
            ollamaModel = ollamaStatus.model || "qwen2.5:7b";
            ollamaBaseUrl = ollamaStatus.base_url || "http://localhost:11434";
            ollamaConfigured = ollamaStatus.is_active;
        }
    }

    async function saveAnthropicKey() {
        savingAi = true;
        savedAi = false;
        try {
            await invoke("set_anthropic_key", { key: anthropicKey.trim() });
            aiConfigured = anthropicKey.trim().length > 0;
            savedAi = true;
            setTimeout(() => (savedAi = false), 2000);
        } finally {
            savingAi = false;
        }
    }

    async function testOllama() {
        ollamaTesting = true;
        ollamaTestResult = null;
        try {
            const status = await invoke<{
                reachable: boolean;
                model: string;
                base_url: string;
                is_active: boolean;
            }>("ollama_status");
            if (status.reachable) {
                ollamaTestResult = {
                    ok: true,
                    message: `Reachable at ${status.base_url}`,
                };
            } else {
                ollamaTestResult = {
                    ok: false,
                    message: `Ollama not reachable at ${status.base_url}`,
                };
            }
        } catch (e) {
            ollamaTestResult = {
                ok: false,
                message: `Test failed: ${String(e)}`,
            };
        } finally {
            ollamaTesting = false;
        }
    }

    async function saveOllamaConfig() {
        savingOllama = true;
        savedOllama = false;
        try {
            await invoke("set_ollama_config", {
                model: ollamaModel.trim() || "qwen2.5:7b",
                baseUrl: ollamaBaseUrl.trim() || "http://localhost:11434",
            });
            ollamaConfigured = true;
            savedOllama = true;
            setTimeout(() => (savedOllama = false), 2000);
        } catch (e) {
            ollamaTestResult = {
                ok: false,
                message: `Save failed: ${String(e)}`,
            };
        } finally {
            savingOllama = false;
        }
    }

    async function saveSettings() {
        saving = true;
        saved = false;
        await invoke("set_setting", {
            key: "integration.github.token",
            value: githubToken,
            project_id: projectId,
        });
        await invoke("set_setting", {
            key: "integration.github.owner",
            value: githubOwner,
            project_id: projectId,
        });
        await invoke("set_setting", {
            key: "integration.github.repo",
            value: githubRepo,
            project_id: projectId,
        });
        saving = false;
        saved = true;
        setTimeout(() => (saved = false), 1500);
    }

    async function testGithub() {
        testing = true;
        testResult = null;
        try {
            if (!githubToken.trim()) {
                testResult = { ok: false, message: "Enter a token first." };
                return;
            }
            const res = await fetch("https://api.github.com/user", {
                headers: {
                    Authorization: `Bearer ${githubToken.trim()}`,
                    Accept: "application/vnd.github+json",
                },
            });
            const data = await res.json().catch(() => ({}));
            if (!res.ok) {
                const msg = data?.message ? `: ${data.message}` : "";
                testResult = {
                    ok: false,
                    message: `GitHub rejected the token (${res.status})${msg}`,
                };
                return;
            }
            const login = data?.login ? `as ${data.login}` : "";
            testResult = { ok: true, message: `Connected ${login}`.trim() };
        } catch (e) {
            testResult = { ok: false, message: `Test failed: ${String(e)}` };
        } finally {
            testing = false;
        }
    }

    async function testRepoAccess() {
        testing = true;
        testResult = null;
        try {
            if (!githubToken.trim()) {
                testResult = { ok: false, message: "Enter a token first." };
                return;
            }
            if (!githubOwner.trim() || !githubRepo.trim()) {
                testResult = {
                    ok: false,
                    message: "Set owner and repo first.",
                };
                return;
            }
            const res = await fetch(
                `https://api.github.com/repos/${githubOwner}/${githubRepo}`,
                {
                    headers: {
                        Authorization: `Bearer ${githubToken.trim()}`,
                        Accept: "application/vnd.github+json",
                    },
                },
            );
            const data = await res.json().catch(() => ({}));
            if (!res.ok) {
                const msg = data?.message ? `: ${data.message}` : "";
                testResult = {
                    ok: false,
                    message: `Repo access failed (${res.status})${msg}`,
                };
                return;
            }
            testResult = {
                ok: true,
                message: `Repo OK: ${data.full_name ?? githubOwner + "/" + githubRepo}`,
            };
        } catch (e) {
            testResult = {
                ok: false,
                message: `Repo test failed: ${String(e)}`,
            };
        } finally {
            testing = false;
        }
    }

    function metaKey(owner: string, repo: string, number: number): string {
        return `${owner}/${repo}#${number}`;
    }

    function parseGithubLink(
        link: string,
    ): { owner: string; repo: string; number?: number } | null {
        try {
            const url = new URL(link);
            if (!url.hostname.includes("github.com")) return null;
            const parts = url.pathname.split("/").filter(Boolean);
            if (parts.length < 2) return null;
            const owner = parts[0];
            const repo = parts[1];
            if (
                parts.length >= 4 &&
                (parts[2] === "issues" || parts[2] === "pull")
            ) {
                const num = parseInt(parts[3], 10);
                if (!Number.isNaN(num)) return { owner, repo, number: num };
            }
            return { owner, repo };
        } catch {
            return null;
        }
    }

    function extractGithubKey(meta: Record<string, unknown>): string | null {
        const external = meta?.external as Record<string, unknown> | undefined;
        if (!external) return null;
        if (external.system !== "github") return null;
        const owner = external.owner as string | undefined;
        const repo = external.repo as string | undefined;
        const number = external.number as number | undefined;
        if (!owner || !repo || typeof number !== "number") return null;
        return metaKey(owner, repo, number);
    }

    async function syncGithub() {
        syncing = true;
        syncResult = null;
        try {
            if (!githubOwner.trim() || !githubRepo.trim()) {
                syncResult = {
                    ok: false,
                    message: "Set default owner and repo first.",
                };
                return;
            }
            const headers: Record<string, string> = {
                Accept: "application/vnd.github+json",
            };
            if (githubToken.trim())
                headers.Authorization = `Bearer ${githubToken.trim()}`;

            const artifacts = await invoke<SubsystemArtifact[]>(
                "list_project_artifacts",
                {
                    projectId,
                },
            );
            const artifactByLink = new Map<string, SubsystemArtifact[]>();
            const artifactByKey = new Map<string, SubsystemArtifact[]>();
            for (const a of artifacts) {
                if (!a.link) continue;
                const norm = normalizeLink(a.link);
                const list = artifactByLink.get(norm) ?? [];
                list.push(a);
                artifactByLink.set(norm, list);

                const parsed = parseGithubLink(a.link);
                if (parsed?.owner && parsed.repo && parsed.number) {
                    const key = metaKey(
                        parsed.owner,
                        parsed.repo,
                        parsed.number,
                    );
                    const klist = artifactByKey.get(key) ?? [];
                    klist.push(a);
                    artifactByKey.set(key, klist);
                }
            }

            const res = await fetch(
                `https://api.github.com/repos/${githubOwner}/${githubRepo}/issues?state=all&per_page=50`,
                { headers },
            );
            if (!res.ok) {
                syncResult = {
                    ok: false,
                    message: `GitHub sync failed (${res.status}).`,
                };
                return;
            }
            const data = await res.json();
            const existing = new Map<string, Node>();
            for (const n of $nodes) {
                const key = extractGithubKey(n.meta as Record<string, unknown>);
                if (key) existing.set(key, n);
            }

            let created = 0;
            let updated = 0;
            let linked = 0;
            const now = new Date().toISOString();

            for (const item of data as any[]) {
                if (!item || !item.number) continue;
                const key = metaKey(githubOwner, githubRepo, item.number);
                const isPull = Boolean(item.pull_request);
                const title = item.title ?? `#${item.number}`;
                const name = `#${item.number} ${title}`;
                const externalMeta = {
                    system: "github",
                    kind: isPull ? "pull" : "issue",
                    owner: githubOwner,
                    repo: githubRepo,
                    number: item.number,
                    url: item.html_url,
                    state: item.state,
                };

                const existingNode = existing.get(key);
                let nodeId: string;
                if (existingNode) {
                    const updatedNode: Node = {
                        ...existingNode,
                        name,
                        description: item.body ?? "",
                        meta: { ...existingNode.meta, external: externalMeta },
                        modified_at: now,
                    };
                    await saveNode(updatedNode);
                    updated++;
                    nodeId = updatedNode.id;
                } else {
                    const node: Node = {
                        id: uuidv4(),
                        project_id: projectId,
                        kind: "external",
                        name,
                        description: item.body ?? "",
                        data: { kind: "external" },
                        meta: { external: externalMeta },
                        created_at: now,
                        modified_at: now,
                    };
                    await saveNode(node);
                    created++;
                    nodeId = node.id;
                }

                const link = item.html_url ? normalizeLink(item.html_url) : "";
                const keyMatches = artifactByKey.get(key) ?? [];
                const linkMatches = link
                    ? (artifactByLink.get(link) ?? [])
                    : [];
                const matches = [...new Set([...keyMatches, ...linkMatches])];
                for (const a of matches) {
                    const didLink = await ensureTraceEdge(
                        a.subsystem_id,
                        nodeId,
                    );
                    if (didLink) linked++;
                }
            }

            lastLinkCount = linked;
            syncResult = {
                ok: true,
                message: `Synced — ${created} new, ${updated} updated, ${linked} linked.`,
            };
        } catch (e) {
            syncResult = { ok: false, message: `Sync failed: ${String(e)}` };
        } finally {
            syncing = false;
        }
    }

    function normalizeLink(link: string): string {
        try {
            const url = new URL(link);
            return `${url.protocol}//${url.host}${url.pathname}`
                .toLowerCase()
                .replace(/\/$/, "");
        } catch {
            return link.trim().toLowerCase().replace(/\/$/, "");
        }
    }

    async function ensureTraceEdge(
        subsystemId: string,
        externalId: string,
    ): Promise<boolean> {
        const exists = $edges.some(
            (e) =>
                e.kind === "traces" &&
                ((e.source_id === subsystemId && e.target_id === externalId) ||
                    (e.source_id === externalId &&
                        e.target_id === subsystemId)),
        );
        if (exists) return false;
        const now = new Date().toISOString();
        const edge: Edge = {
            id: uuidv4(),
            project_id: projectId,
            kind: "traces",
            source_id: subsystemId,
            target_id: externalId,
            label: "",
            meta: {},
            created_at: now,
            modified_at: now,
        };
        await saveEdge(edge);
        return true;
    }

    // ── Model export ───────────────────────────────────────────────────────────
    let exporting: "markdown" | "json" | "xmi" | null = null;
    let exportResult: { ok: boolean; message: string } | null = null;

    async function downloadExport(format: "markdown" | "json" | "xmi") {
        exporting = format;
        exportResult = null;
        try {
            let content: string;
            let filename: string;
            let mime: string;
            if (format === "markdown") {
                content = await invoke<string>("export_markdown", { projectId });
                filename = "model.md";
                mime = "text/markdown";
            } else if (format === "json") {
                content = await invoke<string>("export_json", { projectId });
                filename = "model.json";
                mime = "application/json";
            } else {
                content = await invoke<string>("export_xmi", { projectId });
                filename = "model.xmi";
                mime = "application/xml";
            }
            const blob = new Blob([content], { type: mime });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = filename;
            a.click();
            URL.revokeObjectURL(url);
            exportResult = { ok: true, message: `${filename} downloaded` };
            setTimeout(() => (exportResult = null), 3000);
        } catch (e) {
            exportResult = { ok: false, message: `Export failed: ${String(e)}` };
        } finally {
            exporting = null;
        }
    }

    onMount(async () => {
        await loadProject(projectId);
        await loadSettings();
    });
</script>

<div class="integrations-root page-frame">
    <header class="integrations-header page-header">
        <div class="header-left">
            <div class="page-eyebrow">Integrations</div>
            <h1 class="page-title">Connected Services</h1>
            <p class="page-subtitle">
                Link external tools to sync artifacts and trace work items to
                your system model.
            </p>
        </div>
        <div class="stat-bar">
            <div class="stat">
                <div class="stat-value">4</div>
                <div class="stat-label">Available</div>
            </div>
            <div class="stat">
                <div class="stat-value">
                    {(githubToken ? 1 : 0) +
                        (aiConfigured ? 1 : 0) +
                        (ollamaConfigured ? 1 : 0)}
                </div>
                <div class="stat-label">Connected</div>
            </div>
            <div class="stat">
                <div class="stat-value">{externalNodes.length}</div>
                <div class="stat-label">Synced items</div>
            </div>
        </div>
    </header>

    <main class="integrations-body page-body">
        <!-- GitHub integration card -->
        <div class="integration-card">
            <!-- Card header -->
            <div class="int-header">
                <div class="int-icon">
                    <Github size={20} />
                </div>
                <div class="int-meta">
                    <div class="int-name">GitHub</div>
                    <div class="int-desc">
                        Issues & pull requests as traceable system artifacts
                    </div>
                </div>
                <div class="int-badge" class:connected={!!githubToken}>
                    {githubToken ? "Configured" : "Not configured"}
                </div>
            </div>

            <div class="int-divider"></div>

            <!-- Credentials section -->
            <div class="int-section">
                <div class="section-label">Authentication</div>
                <label class="field-label">
                    Personal access token
                    <div class="token-wrap">
                        {#if showToken}
                            <input
                                class="field"
                                type="text"
                                bind:value={githubToken}
                                placeholder="ghp_..."
                                autocomplete="off"
                            />
                        {:else}
                            <input
                                class="field"
                                type="password"
                                bind:value={githubToken}
                                placeholder="ghp_..."
                                autocomplete="off"
                            />
                        {/if}
                        <button
                            class="token-eye"
                            type="button"
                            on:click={() => (showToken = !showToken)}
                            title={showToken ? "Hide token" : "Show token"}
                        >
                            {#if showToken}<EyeOff size={14} />{:else}<Eye
                                    size={14}
                                />{/if}
                        </button>
                    </div>
                </label>
            </div>

            <div class="int-divider"></div>

            <!-- Repository section -->
            <div class="int-section">
                <div class="section-label">Default repository</div>
                <div class="row">
                    <label class="field-label">
                        Owner / org
                        <input
                            class="field"
                            bind:value={githubOwner}
                            placeholder="my-org"
                        />
                    </label>
                    <label class="field-label">
                        Repository
                        <input
                            class="field"
                            bind:value={githubRepo}
                            placeholder="my-repo"
                        />
                    </label>
                </div>
            </div>

            <div class="int-divider"></div>

            <!-- Actions section -->
            <div class="int-section">
                <div class="section-label">Actions</div>
                <div class="actions">
                    <button
                        class="btn-primary"
                        on:click={saveSettings}
                        disabled={saving}
                    >
                        {#if saving}<Loader
                                size={13}
                                class="spin"
                            />{:else}<Save size={13} />{/if}
                        {saving ? "Saving…" : "Save"}
                    </button>
                    <button
                        class="btn-ghost"
                        on:click={testGithub}
                        disabled={testing}
                    >
                        {#if testing}<Loader
                                size={13}
                                class="spin"
                            />{:else}<Zap size={13} />{/if}
                        Test Token
                    </button>
                    <button
                        class="btn-ghost"
                        on:click={testRepoAccess}
                        disabled={testing}
                    >
                        {#if testing}<Loader
                                size={13}
                                class="spin"
                            />{:else}<Zap size={13} />{/if}
                        Test Repo
                    </button>
                    <button
                        class="btn-ghost"
                        on:click={syncGithub}
                        disabled={syncing}
                    >
                        {#if syncing}<Loader
                                size={13}
                                class="spin"
                            />{:else}<RefreshCw size={13} />{/if}
                        {syncing ? "Syncing…" : "Sync Issues & PRs"}
                    </button>
                </div>

                <!-- Status pills -->
                <div class="status-row">
                    {#if saved}
                        <span class="status-pill ok">
                            <CheckCircle2 size={11} /> Saved
                        </span>
                    {/if}
                    {#if testResult}
                        <span
                            class="status-pill"
                            class:ok={testResult.ok}
                            class:bad={!testResult.ok}
                        >
                            {#if testResult.ok}<CheckCircle2
                                    size={11}
                                />{:else}<XCircle size={11} />{/if}
                            {testResult.message}
                        </span>
                    {/if}
                    {#if syncResult}
                        <span
                            class="status-pill"
                            class:ok={syncResult.ok}
                            class:bad={!syncResult.ok}
                        >
                            {#if syncResult.ok}<CheckCircle2
                                    size={11}
                                />{:else}<XCircle size={11} />{/if}
                            {syncResult.message}
                        </span>
                    {/if}
                </div>
            </div>
        </div>

        <!-- Anthropic AI card -->
        <div class="integration-card">
            <div class="int-header">
                <div class="int-icon ai-icon">
                    <BrainCircuit size={20} />
                </div>
                <div class="int-meta">
                    <div class="int-name">
                        Claude AI <span class="int-name-sub">by Anthropic</span>
                    </div>
                    <div class="int-desc">
                        AI-powered requirement extraction from engineering
                        documents
                    </div>
                </div>
                <div class="int-badge" class:connected={aiConfigured}>
                    {aiConfigured ? "Configured" : "Not configured"}
                </div>
            </div>

            <div class="int-divider"></div>

            <div class="int-section">
                <div class="section-label">Authentication</div>
                <label class="field-label">
                    API key
                    <div class="token-wrap">
                        {#if showAnthropicKey}
                            <input
                                class="field"
                                type="text"
                                bind:value={anthropicKey}
                                placeholder={aiConfigured
                                    ? "sk-ant-… (saved — enter new key to replace)"
                                    : "sk-ant-…"}
                                autocomplete="off"
                            />
                        {:else}
                            <input
                                class="field"
                                type="password"
                                bind:value={anthropicKey}
                                placeholder={aiConfigured
                                    ? "••••••••••••••• (saved)"
                                    : "sk-ant-…"}
                                autocomplete="off"
                            />
                        {/if}
                        <button
                            class="token-eye"
                            type="button"
                            on:click={() =>
                                (showAnthropicKey = !showAnthropicKey)}
                            title={showAnthropicKey ? "Hide key" : "Show key"}
                        >
                            {#if showAnthropicKey}<EyeOff
                                    size={14}
                                />{:else}<Eye size={14} />{/if}
                        </button>
                    </div>
                </label>
                <p class="field-hint">
                    Your key is stored locally on this device and never leaves
                    the app. Get one at <strong>console.anthropic.com</strong>.
                </p>
            </div>

            <div class="int-divider"></div>

            <div class="int-section">
                <div class="section-label">Actions</div>
                <div class="actions">
                    <button
                        class="btn-primary"
                        on:click={saveAnthropicKey}
                        disabled={savingAi || !anthropicKey.trim()}
                    >
                        {#if savingAi}<Loader
                                size={13}
                                class="spin"
                            />{:else}<Save size={13} />{/if}
                        {savingAi ? "Saving…" : "Save key"}
                    </button>
                    {#if aiConfigured && !anthropicKey.trim()}
                        <button
                            class="btn-ghost"
                            on:click={async () => {
                                await invoke("set_anthropic_key", { key: "" });
                                aiConfigured = false;
                            }}
                            disabled={savingAi}
                        >
                            <XCircle size={13} /> Remove key
                        </button>
                    {/if}
                </div>
                <div class="status-row">
                    {#if savedAi}
                        <span class="status-pill ok">
                            <CheckCircle2 size={11} /> Saved — AI extraction enabled
                        </span>
                    {/if}
                </div>
            </div>
        </div>

        <!-- Ollama card -->
        <div class="integration-card">
            <div class="int-header">
                <div class="int-icon ollama-icon">
                    <BrainCircuit size={20} />
                </div>
                <div class="int-meta">
                    <div class="int-name">
                        Ollama <span class="int-name-sub">Local LLM</span>
                    </div>
                    <div class="int-desc">
                        Run open-source models locally — no API key required
                    </div>
                </div>
                <div class="int-badge" class:connected={ollamaConfigured}>
                    {ollamaConfigured ? "Active" : "Not configured"}
                </div>
            </div>

            <div class="int-divider"></div>

            <div class="int-section">
                <div class="section-label">Configuration</div>
                <label class="field-label">
                    Model
                    <input
                        class="field"
                        bind:value={ollamaModel}
                        placeholder="qwen2.5:3b"
                    />
                </label>
                <label class="field-label">
                    Base URL
                    <input
                        class="field"
                        bind:value={ollamaBaseUrl}
                        placeholder="http://localhost:11434"
                    />
                </label>
                <p class="field-hint">
                    Pull a model with <code>ollama pull qwen2.5:3b</code> then
                    hit
                    <strong>Test connection</strong> to verify.
                </p>
            </div>

            <div class="int-divider"></div>

            <div class="int-section">
                <div class="section-label">Actions</div>
                <div class="actions">
                    <button
                        class="btn-primary"
                        on:click={saveOllamaConfig}
                        disabled={savingOllama}
                    >
                        {#if savingOllama}<Loader
                                size={13}
                                class="spin"
                            />{:else}<Save size={13} />{/if}
                        {savingOllama ? "Saving…" : "Save & activate"}
                    </button>
                    <button
                        class="btn-ghost"
                        on:click={testOllama}
                        disabled={ollamaTesting}
                    >
                        {#if ollamaTesting}<Loader
                                size={13}
                                class="spin"
                            />{:else}<Zap size={13} />{/if}
                        Test connection
                    </button>
                    {#if ollamaConfigured}
                        <button
                            class="btn-ghost"
                            on:click={async () => {
                                await invoke("set_setting", {
                                    key: "ai.provider",
                                    value: "",
                                    project_id: null,
                                }).catch(() => {});
                                ollamaConfigured = false;
                            }}
                            disabled={savingOllama}
                        >
                            <XCircle size={13} /> Deactivate
                        </button>
                    {/if}
                </div>
                <div class="status-row">
                    {#if savedOllama}
                        <span class="status-pill ok">
                            <CheckCircle2 size={11} /> Saved — Ollama is now active
                        </span>
                    {/if}
                    {#if ollamaTestResult}
                        <span
                            class="status-pill"
                            class:ok={ollamaTestResult.ok}
                            class:bad={!ollamaTestResult.ok}
                        >
                            {#if ollamaTestResult.ok}<CheckCircle2
                                    size={11}
                                />{:else}<XCircle size={11} />{/if}
                            {ollamaTestResult.message}
                        </span>
                    {/if}
                </div>
            </div>
        </div>

        <!-- Model Export card -->
        <div class="integration-card">
            <div class="int-header">
                <div class="int-icon export-icon">
                    <Download size={20} />
                </div>
                <div class="int-meta">
                    <div class="int-name">Model Export</div>
                    <div class="int-desc">
                        Export your SysML model for use in other tools
                    </div>
                </div>
                <div class="int-badge connected">Ready</div>
            </div>

            <div class="int-divider"></div>

            <div class="int-section">
                <div class="section-label">Formats</div>
                <div class="export-grid">
                    <div class="export-format-card">
                        <div class="export-format-icon"><FileText size={18} /></div>
                        <div class="export-format-meta">
                            <div class="export-format-name">Markdown</div>
                            <div class="export-format-desc">Requirements table + traceability matrix</div>
                        </div>
                        <button
                            class="btn-ghost"
                            on:click={() => downloadExport("markdown")}
                            disabled={exporting !== null}
                        >
                            {#if exporting === "markdown"}<Loader size={13} class="spin" />{:else}<Download size={13} />{/if}
                            .md
                        </button>
                    </div>
                    <div class="export-format-card">
                        <div class="export-format-icon"><FileJson size={18} /></div>
                        <div class="export-format-meta">
                            <div class="export-format-name">JSON-LD</div>
                            <div class="export-format-desc">Native round-trip format with full model data</div>
                        </div>
                        <button
                            class="btn-ghost"
                            on:click={() => downloadExport("json")}
                            disabled={exporting !== null}
                        >
                            {#if exporting === "json"}<Loader size={13} class="spin" />{:else}<Download size={13} />{/if}
                            .json
                        </button>
                    </div>
                    <div class="export-format-card">
                        <div class="export-format-icon"><FileCode2 size={18} /></div>
                        <div class="export-format-meta">
                            <div class="export-format-name">SysML XMI</div>
                            <div class="export-format-desc">XMI 2.1 / SysML 1.6 — compatible with Cameo, Rhapsody, Papyrus</div>
                        </div>
                        <button
                            class="btn-ghost"
                            on:click={() => downloadExport("xmi")}
                            disabled={exporting !== null}
                        >
                            {#if exporting === "xmi"}<Loader size={13} class="spin" />{:else}<Download size={13} />{/if}
                            .xmi
                        </button>
                    </div>
                </div>
                <div class="status-row">
                    {#if exportResult}
                        <span class="status-pill" class:ok={exportResult.ok} class:bad={!exportResult.ok}>
                            {#if exportResult.ok}<CheckCircle2 size={11} />{:else}<XCircle size={11} />{/if}
                            {exportResult.message}
                        </span>
                    {/if}
                </div>
            </div>
        </div>

        <!-- More integrations placeholder -->
        <div class="coming-soon">
            <Puzzle size={28} class="coming-icon" />
            <div class="coming-title">More integrations coming soon</div>
            <div class="coming-body">Jira, GitLab, Azure DevOps, and more.</div>
        </div>
    </main>
</div>

<style>
    .integrations-root {
        display: flex;
        flex-direction: column;
        height: 100%;
        background: var(--surface-base);
    }

    /* ── Header ── */
    .integrations-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: var(--space-6);
        padding: var(--space-4) var(--space-6) var(--space-3);
        border-bottom: 1px solid var(--surface-border);
        background: var(--surface-raised);
        flex-shrink: 0;
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

    /* ── Body ── */
    .integrations-body {
        flex: 1;
        min-height: 0;
        padding: var(--space-5) var(--space-6);
        display: flex;
        flex-direction: column;
        gap: var(--space-5);
        overflow-y: auto;
    }

    /* ── Integration card ── */
    .integration-card {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-xl);
        max-width: 680px;
    }

    .int-header {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        padding: var(--space-4) var(--space-5);
    }

    .int-icon {
        width: 38px;
        height: 38px;
        border-radius: var(--radius-lg);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        display: flex;
        align-items: center;
        justify-content: center;
        color: var(--text-secondary);
        flex-shrink: 0;
    }

    .int-meta {
        flex: 1;
    }

    .int-name {
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
    }

    .int-desc {
        font-size: var(--text-xs);
        color: var(--text-muted);
        margin-top: 2px;
    }

    .int-badge {
        font-size: var(--text-xs);
        padding: 3px 10px;
        border-radius: var(--radius-full);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        color: var(--text-muted);
        white-space: nowrap;
    }
    .int-badge.connected {
        background: #22c55e18;
        border-color: #22c55e40;
        color: var(--color-success);
    }

    .ai-icon {
        color: #a78bfa;
        background: #7c3aed18;
        border-color: #7c3aed30;
    }

    .ollama-icon {
        color: #34d399;
        background: #05966918;
        border-color: #05966930;
    }

    .field-hint code {
        font-family: var(--font-mono, monospace);
        font-size: var(--text-xs);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-sm);
        padding: 1px 5px;
    }

    .int-name-sub {
        font-weight: var(--weight-normal);
        color: var(--text-muted);
        font-size: var(--text-xs);
    }

    .field-hint {
        font-size: var(--text-xs);
        color: var(--text-muted);
        line-height: var(--leading-relaxed);
        margin: 0;
    }

    .int-divider {
        height: 1px;
        background: var(--surface-border);
    }

    .int-section {
        padding: var(--space-4) var(--space-5);
        display: flex;
        flex-direction: column;
        gap: var(--space-3);
    }

    .section-label {
        font-size: 10px;
        text-transform: uppercase;
        letter-spacing: 0.12em;
        color: var(--text-muted);
    }

    /* ── Token input with eye toggle ── */
    .token-wrap {
        position: relative;
        display: flex;
        align-items: center;
    }

    .token-wrap .field {
        width: 100%;
        padding-right: 36px;
    }

    .token-eye {
        position: absolute;
        right: 10px;
        background: none;
        border: none;
        color: var(--text-muted);
        cursor: pointer;
        padding: 0;
        display: flex;
        align-items: center;
        transition: color var(--transition-fast);
    }
    .token-eye:hover {
        color: var(--text-secondary);
    }

    /* ── Field label ── */
    .field-label {
        display: flex;
        flex-direction: column;
        gap: var(--space-1);
        font-size: var(--text-xs);
        color: var(--text-secondary);
    }

    .row {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: var(--space-4);
    }

    /* ── Actions ── */
    .actions {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        flex-wrap: wrap;
    }

    /* ── Status pills ── */
    .status-row {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        flex-wrap: wrap;
        min-height: 22px;
    }

    .status-pill {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        font-size: var(--text-xs);
        padding: 3px 8px;
        border-radius: var(--radius-full);
        border: 1px solid var(--surface-border);
        color: var(--text-muted);
        background: var(--surface-overlay);
    }
    .status-pill.ok {
        color: var(--color-success);
        border-color: #22c55e40;
        background: #22c55e18;
    }
    .status-pill.bad {
        color: var(--color-error);
        border-color: #ef444440;
        background: #ef444418;
    }

    /* ── Coming soon ── */
    .coming-soon {
        max-width: 680px;
        border: 1px dashed var(--surface-border);
        border-radius: var(--radius-xl);
        padding: var(--space-6);
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: var(--space-2);
        color: var(--text-muted);
    }

    :global(.coming-icon) {
        opacity: 0.4;
    }

    .coming-title {
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        color: var(--text-secondary);
    }

    .coming-body {
        font-size: var(--text-xs);
        color: var(--text-muted);
    }

    /* ── Spin animation for loader ── */
    :global(.spin) {
        animation: spin 0.8s linear infinite;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    /* ── Export card ── */
    .export-icon {
        color: #60a5fa;
        background: #2563eb18;
        border-color: #2563eb30;
    }

    .export-grid {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
    }

    .export-format-card {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        padding: var(--space-3) var(--space-3);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-lg);
    }

    .export-format-icon {
        width: 34px;
        height: 34px;
        border-radius: var(--radius-md);
        background: var(--surface-base);
        border: 1px solid var(--surface-border);
        display: flex;
        align-items: center;
        justify-content: center;
        color: var(--text-secondary);
        flex-shrink: 0;
    }

    .export-format-meta {
        flex: 1;
        min-width: 0;
    }

    .export-format-name {
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        color: var(--text-primary);
    }

    .export-format-desc {
        font-size: var(--text-xs);
        color: var(--text-muted);
        margin-top: 1px;
    }

    @media (max-width: 900px) {
        .row {
            grid-template-columns: 1fr;
        }
        .integrations-header {
            flex-direction: column;
            align-items: flex-start;
        }
    }
</style>

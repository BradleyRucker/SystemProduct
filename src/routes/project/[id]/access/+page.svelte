<script lang="ts">
    import { onMount } from "svelte";
    import { page } from "$app/stores";
    import { authState } from "$lib/auth/store";
    import {
        authRoleOverrides,
        canManageAccessControl,
        loadProject,
        nodes,
        removeAuthRoleOverride,
        upsertAuthRoleOverride,
        type AccessRole,
    } from "$lib/store/model";

    $: projectId = $page.params.id ?? "";
    $: subsystemNodes = $nodes.filter(
        (n) =>
            n.kind === "block" &&
            !(n.meta as Record<string, unknown>)?.system_root,
    );
    $: canManage = canManageAccessControl();

    let formEmail = "";
    let formUserId = "";
    let formRole: AccessRole = "viewer";
    let formSubsystemId = "";
    let saving = false;
    let notice = "";
    let error = "";

    onMount(async () => {
        if (!projectId) return;
        await loadProject(projectId);
    });

    function clearForm() {
        formEmail = "";
        formUserId = "";
        formRole = "viewer";
        formSubsystemId = "";
        error = "";
        notice = "";
    }

    function editOverride(entry: {
        email?: string;
        user_id?: string;
        role: AccessRole;
        subsystem_id?: string;
    }) {
        formEmail = entry.email ?? "";
        formUserId = entry.user_id ?? "";
        formRole = entry.role;
        formSubsystemId = entry.subsystem_id ?? "";
        error = "";
        notice = "";
    }

    function useCurrentAccount() {
        const user = $authState.user;
        if (!user) return;
        formEmail = user.email ?? "";
        formUserId = user.id;
        error = "";
        notice = "Loaded current signed-in account into the form.";
    }

    async function saveOverride() {
        if (!canManage) {
            error = "Only admins can modify overrides.";
            return;
        }
        const email = formEmail.trim().toLowerCase();
        const userId = formUserId.trim();
        if (!email && !userId) {
            error = "Provide at least Email or User ID.";
            return;
        }
        if (formRole === "subsystem" && !formSubsystemId) {
            error = "Subsystem role requires a subsystem scope.";
            return;
        }

        saving = true;
        error = "";
        notice = "";
        try {
            await upsertAuthRoleOverride({
                email: email || undefined,
                user_id: userId || undefined,
                role: formRole,
                subsystem_id:
                    formRole === "subsystem" ? formSubsystemId || undefined : undefined,
            });
            notice = "Override saved.";
        } catch (e) {
            error = `Failed to save override: ${String(e)}`;
        } finally {
            saving = false;
        }
    }

    async function removeOverride(entry: { email?: string; user_id?: string }) {
        if (!canManage) return;
        saving = true;
        error = "";
        notice = "";
        try {
            await removeAuthRoleOverride({
                email: entry.email,
                user_id: entry.user_id,
            });
            notice = "Override removed.";
        } catch (e) {
            error = `Failed to remove override: ${String(e)}`;
        } finally {
            saving = false;
        }
    }
</script>

<div class="access-root page-frame">
    <header class="access-header page-header">
        <div>
            <div class="page-eyebrow">Access</div>
            <h1 class="page-title">User Role Overrides</h1>
            <p class="page-subtitle">
                Admin-defined role mapping for signed-in users. These overrides apply before
                Nhost metadata role mapping.
            </p>
        </div>
    </header>

    <main class="access-body page-body">
        {#if !canManage}
            <section class="panel blocked">
                <h2>Admin Access Required</h2>
                <p>Switch to an admin access profile to manage role overrides.</p>
            </section>
        {:else}
            <section class="panel editor">
                <div class="panel-title-row">
                    <h2>Edit Override</h2>
                    <div class="actions">
                        <button class="btn-ghost" on:click={useCurrentAccount}>
                            Use current account
                        </button>
                        <button class="btn-ghost" on:click={clearForm}>Clear</button>
                    </div>
                </div>

                <div class="form-grid">
                    <label>
                        Email
                        <input
                            class="field"
                            type="email"
                            placeholder="user@company.com"
                            bind:value={formEmail}
                        />
                    </label>
                    <label>
                        User ID
                        <input
                            class="field"
                            type="text"
                            placeholder="Nhost user UUID"
                            bind:value={formUserId}
                        />
                    </label>
                    <label>
                        Role
                        <select class="field" bind:value={formRole}>
                            <option value="admin">admin</option>
                            <option value="system">system</option>
                            <option value="subsystem">subsystem</option>
                            <option value="viewer">viewer</option>
                        </select>
                    </label>
                    <label>
                        Subsystem scope
                        <select
                            class="field"
                            bind:value={formSubsystemId}
                            disabled={formRole !== "subsystem"}
                        >
                            <option value="">None</option>
                            {#each subsystemNodes as sub (sub.id)}
                                <option value={sub.id}>{sub.name}</option>
                            {/each}
                        </select>
                    </label>
                </div>

                <div class="actions">
                    <button
                        class="btn-primary"
                        on:click={() => void saveOverride()}
                        disabled={saving}
                    >
                        Save override
                    </button>
                </div>

                {#if error}
                    <div class="msg error">{error}</div>
                {/if}
                {#if notice}
                    <div class="msg ok">{notice}</div>
                {/if}
            </section>

            <section class="panel list">
                <h2>Active Overrides ({$authRoleOverrides.length})</h2>
                {#if $authRoleOverrides.length === 0}
                    <div class="empty">No overrides configured.</div>
                {:else}
                    <div class="rows">
                        {#each $authRoleOverrides as entry (`${entry.user_id ?? ""}:${entry.email ?? ""}`)}
                            <div class="row">
                                <div class="id">
                                    <div><strong>Email:</strong> {entry.email ?? "-"}</div>
                                    <div><strong>User ID:</strong> {entry.user_id ?? "-"}</div>
                                </div>
                                <div class="role">{entry.role}</div>
                                <div class="scope">
                                    {#if entry.role === "subsystem"}
                                        {entry.subsystem_id ?? "No subsystem"}
                                    {:else}
                                        -
                                    {/if}
                                </div>
                                <div class="actions">
                                    <button
                                        class="btn-ghost"
                                        on:click={() => editOverride(entry)}
                                        disabled={saving}
                                    >
                                        Edit
                                    </button>
                                    <button
                                        class="btn-ghost danger"
                                        on:click={() => void removeOverride(entry)}
                                        disabled={saving}
                                    >
                                        Remove
                                    </button>
                                </div>
                            </div>
                        {/each}
                    </div>
                {/if}
            </section>
        {/if}
    </main>
</div>

<style>
    .access-root {
        height: 100%;
        display: flex;
        flex-direction: column;
        background: var(--surface-base);
    }

    .access-header {
        padding: var(--space-4) var(--space-6);
    }

    .access-body {
        padding: var(--space-4) var(--space-6) var(--space-6);
        display: grid;
        gap: var(--space-4);
    }

    .panel {
        border: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        border-radius: var(--radius-lg);
        padding: var(--space-4);
    }

    .blocked h2 {
        margin-bottom: var(--space-2);
    }

    .panel-title-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: var(--space-3);
        margin-bottom: var(--space-3);
    }

    h2 {
        font-size: var(--text-base);
    }

    .form-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(220px, 1fr));
        gap: var(--space-3);
        margin-bottom: var(--space-3);
    }

    label {
        display: grid;
        gap: 6px;
        font-size: var(--text-xs);
        color: var(--text-secondary);
    }

    .field {
        width: 100%;
        border: 1px solid var(--surface-border);
        background: #0e1523;
        color: var(--text-primary);
        border-radius: var(--radius-md);
        padding: 8px 10px;
        font: inherit;
    }

    .field:focus {
        outline: none;
        border-color: var(--accent);
    }

    .actions {
        display: flex;
        gap: var(--space-2);
        flex-wrap: wrap;
    }

    .msg {
        margin-top: var(--space-2);
        padding: 8px 10px;
        border-radius: var(--radius-md);
        font-size: var(--text-xs);
    }

    .msg.error {
        border: 1px solid #ef444470;
        background: #ef444420;
        color: #fca5a5;
    }

    .msg.ok {
        border: 1px solid #22c55e66;
        background: #22c55e1f;
        color: #86efac;
    }

    .rows {
        display: grid;
        gap: var(--space-2);
    }

    .row {
        border: 1px solid var(--surface-border-subtle);
        border-radius: var(--radius-md);
        background: #0f1627;
        padding: var(--space-3);
        display: grid;
        grid-template-columns: minmax(250px, 1.4fr) 0.7fr 1fr auto;
        align-items: center;
        gap: var(--space-2);
        font-size: var(--text-xs);
    }

    .role {
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: var(--accent-hover);
    }

    .empty {
        color: var(--text-muted);
        font-size: var(--text-sm);
        padding: var(--space-2) 0;
    }

    .danger {
        border-color: #ef444460;
        color: #fca5a5;
    }

    @media (max-width: 980px) {
        .form-grid {
            grid-template-columns: 1fr;
        }
        .row {
            grid-template-columns: 1fr;
            align-items: flex-start;
        }
    }
</style>

<script lang="ts">
    import { page } from "$app/stores";
    import {
        currentProject,
        nodes,
        readOnly,
        setReadOnly,
        accessControl,
        activeAccessProfile,
        canManageAccessControl,
        setAccessControlEnabled,
        setActiveAccessProfile,
        setActiveAccessProfileSession,
        setSubsystemScope,
        setSubsystemScopeSession,
        authRoleOverrides,
        type AuthRoleOverride,
    } from "$lib/store/model";
    import { onMount } from "svelte";
    import { applyTheme, getTheme, toggleTheme } from "$lib/theme";
    import { invoke } from "@tauri-apps/api/core";
    import { goto } from "$app/navigation";
    import { authEnabled } from "$lib/auth/nhost";
    import { authState, signOut } from "$lib/auth/store";
    import type { Project, Node } from "$lib/types";
    import {
        Layers,
        GitBranch,
        ClipboardList,
        ArrowLeftRight,
        FileText,
        Puzzle,
        Sun,
        Moon,
        Lock,
        Unlock,
        ChevronDown,
        Shield,
        ShieldOff,
        LogOut,
    } from "lucide-svelte";

    $: id = $page.params.id;
    $: projectRouteId = id ?? "";
    $: path = $page.url.pathname;
    $: isSubsystemProfile =
        $accessControl.enabled && $activeAccessProfile.role === "subsystem";
    $: scopedSubsystemRoute =
        isSubsystemProfile && $activeAccessProfile.subsystem_id
            ? `/project/${projectRouteId}/subsystem/${$activeAccessProfile.subsystem_id}`
            : "";

    let theme: "dark" | "light" = "dark";
    let projects: Project[] = [];
    let authProfileAppliedKey = "";

    type AccessProfileId = "admin" | "system" | "subsystem" | "viewer";

    function metadataString(
        metadata: Record<string, unknown> | null | undefined,
        keys: string[],
    ): string | undefined {
        if (!metadata) return undefined;
        for (const key of keys) {
            const value = metadata[key];
            if (typeof value === "string" && value.trim()) {
                return value.trim();
            }
        }
        return undefined;
    }

    function roleFromAuthUser(user: {
        roles?: string[];
        metadata?: Record<string, unknown> | null;
    }): AccessProfileId {
        const metaRole = metadataString(user.metadata, [
            "app_role",
            "access_role",
            "role",
        ])
            ?.toLowerCase()
            .trim();
        if (
            metaRole &&
            ["admin", "system", "subsystem", "viewer"].includes(metaRole)
        ) {
            return metaRole as AccessProfileId;
        }

        const roles = (user.roles ?? []).map((r) => r.toLowerCase().trim());
        if (roles.includes("admin")) return "admin";
        if (roles.includes("system")) return "system";
        if (roles.includes("subsystem")) return "subsystem";
        if (roles.includes("viewer")) return "viewer";
        return "viewer";
    }

    function subsystemScopeFromAuthUser(
        user: { metadata?: Record<string, unknown> | null },
        nodesList: Node[],
    ): string | undefined {
        const explicitId = metadataString(user.metadata, [
            "subsystem_id",
            "subsystemId",
        ]);
        if (explicitId && nodesList.some((n) => n.id === explicitId)) {
            return explicitId;
        }

        const scopedName = metadataString(user.metadata, [
            "subsystem",
            "subsystem_name",
            "subsystemName",
        ])
            ?.toLowerCase()
            .trim();
        if (!scopedName) return undefined;

        const byName = nodesList.find(
            (n) =>
                n.kind === "block" &&
                !(n.meta as Record<string, unknown>)?.system_root &&
                n.name.trim().toLowerCase() === scopedName,
        );
        return byName?.id;
    }

    function roleOverrideForUser(user: {
        id?: string;
        email?: string;
    }): AuthRoleOverride | null {
        const userId = user.id?.trim() ?? "";
        const email = user.email?.trim().toLowerCase() ?? "";

        if (userId) {
            const byId = $authRoleOverrides.find((o) => o.user_id === userId);
            if (byId) return byId;
        }
        if (email) {
            const byEmail = $authRoleOverrides.find(
                (o) => o.email?.trim().toLowerCase() === email,
            );
            if (byEmail) return byEmail;
        }
        return null;
    }

    onMount(() => {
        theme = getTheme();
        applyTheme(theme);
        loadProjects();
    });

    async function loadProjects() {
        try {
            projects = await invoke<Project[]>("list_projects");
        } catch {
            projects = [];
        }
    }

    function onToggleTheme() {
        theme = toggleTheme(theme);
    }
    function onToggleReadOnly() {
        setReadOnly(!$readOnly);
    }

    function onSignOut() {
        void signOut();
    }

    async function onToggleAccessControl() {
        if (!canManageAccessControl()) return;
        await setAccessControlEnabled(!$accessControl.enabled);
    }

    async function onAccessProfileChange(event: Event) {
        if (!canManageAccessControl()) return;
        const target = event.currentTarget as HTMLSelectElement | null;
        const nextId = target?.value ?? "";
        if (!nextId) return;
        await setActiveAccessProfile(nextId);
    }

    async function onSubsystemScopeChange(event: Event) {
        // Allow admin/system managers to change the project-wide setting, but
        // also allow a subsystem-profile user to change their session-only
        // scope via `setSubsystemScopeSession` (non-persisted) so they can
        // pick which subsystem to act as for this session without altering
        // project ACLs.
        const target = event.currentTarget as HTMLSelectElement | null;
        const nextId = target?.value ?? "";
        if (!nextId && nextId !== "") return;
        if (canManageAccessControl()) {
            await setSubsystemScope(nextId || undefined);
        } else {
            // Session-only update
            setSubsystemScopeSession(nextId || undefined);
        }
    }

    function onProjectChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement | null;
        const nextId = target?.value ?? "";
        if (!nextId || nextId === projectRouteId) return;
        if (isSubsystemProfile) {
            goto(`/project/${nextId}/system`);
            return;
        }
        const tail =
            path.replace(`/project/${projectRouteId}`, "") || "/system";
        goto(`/project/${nextId}${tail}`);
    }

    const defaultNavItems = [
        {
            label: "System",
            href: (id: string) => `/project/${id}/system`,
            match: (p: string, id: string) =>
                p === `/project/${id}/system` || p.includes("/subsystem/"),
        },
        {
            label: "Diagrams",
            href: (id: string) => `/project/${id}/diagrams`,
            match: (p: string, id: string) =>
                p.includes(`/project/${id}/diagrams`),
        },
        {
            label: "Requirements",
            href: (id: string) => `/project/${id}/requirements`,
            match: (p: string, id: string) =>
                p.includes(`/project/${id}/requirements`),
        },
        {
            label: "Traceability",
            href: (id: string) => `/project/${id}/traceability`,
            match: (p: string, id: string) =>
                p.includes(`/project/${id}/traceability`),
        },
        {
            label: "Documents",
            href: (id: string) => `/project/${id}/documents`,
            match: (p: string, id: string) =>
                p.includes(`/project/${id}/documents`),
        },
        {
            label: "Integrations",
            href: (id: string) => `/project/${id}/integrations`,
            match: (p: string, id: string) =>
                p.includes(`/project/${id}/integrations`),
        },
        {
            label: "Simulation",
            href: (id: string) => `/project/${id}/simulation`,
            match: (p: string, id: string) =>
                p.includes(`/project/${id}/simulation`),
        },
        {
            label: "Baselines",
            href: (id: string) => `/project/${id}/baselines`,
            match: (p: string, id: string) =>
                p.includes(`/project/${id}/baselines`),
        },
        {
            label: "Access",
            href: (id: string) => `/project/${id}/access`,
            match: (p: string, id: string) =>
                p.includes(`/project/${id}/access`),
        },
    ];
    $: navItems = isSubsystemProfile
        ? [
              {
                  label: "My Subsystem",
                  href: (id: string) =>
                      scopedSubsystemRoute || `/project/${id}/system`,
                  match: (p: string, id: string) =>
                      p.includes(`/project/${id}/subsystem/`) ||
                      p === `/project/${id}/system`,
              },
          ]
        : defaultNavItems;

    $: subsystemNodes = $nodes.filter(
        (n) =>
            n.kind === "block" &&
            !(n.meta as Record<string, unknown>)?.system_root,
    );

    $: {
        const user = $authState.user;
        if (!user) {
            authProfileAppliedKey = "";
        } else {
            const canMap =
                authEnabled &&
                $accessControl.enabled &&
                Boolean(projectRouteId);
            if (canMap) {
                const applyKey = `${projectRouteId}:${user.id}`;
                if (authProfileAppliedKey !== applyKey) {
                    authProfileAppliedKey = applyKey;

                    const override = roleOverrideForUser(user);
                    const roleId = override?.role ?? roleFromAuthUser(user);
                    setActiveAccessProfileSession(roleId);
                    if (roleId === "subsystem") {
                        setSubsystemScopeSession(
                            override?.subsystem_id ??
                                subsystemScopeFromAuthUser(user, $nodes),
                        );
                    } else {
                        setSubsystemScopeSession(undefined);
                    }
                }
            }
        }
    }

    $: if (isSubsystemProfile && projectRouteId) {
        if (!$activeAccessProfile.subsystem_id) {
            const fallback = `/project/${projectRouteId}/system`;
            if (path !== fallback) {
                goto(fallback, { replaceState: true });
            }
        } else if (scopedSubsystemRoute && path !== scopedSubsystemRoute) {
            goto(scopedSubsystemRoute, { replaceState: true });
        }
    }
</script>

<div class="app-shell">
    <!-- Top bar -->
    <header class="app-topbar">
        <!-- Logomark + wordmark -->
        <a href="/" class="brand">
            <div class="brand-mark" aria-hidden="true">
                <svg width="22" height="22" viewBox="0 0 22 22" fill="none">
                    <rect
                        x="1"
                        y="1"
                        width="9"
                        height="9"
                        rx="2"
                        fill="var(--accent)"
                        opacity="0.9"
                    />
                    <rect
                        x="12"
                        y="1"
                        width="9"
                        height="9"
                        rx="2"
                        fill="var(--accent)"
                        opacity="0.5"
                    />
                    <rect
                        x="1"
                        y="12"
                        width="9"
                        height="9"
                        rx="2"
                        fill="var(--accent)"
                        opacity="0.5"
                    />
                    <rect
                        x="12"
                        y="12"
                        width="9"
                        height="9"
                        rx="2"
                        fill="var(--accent)"
                        opacity="0.25"
                    />
                </svg>
            </div>
            <span class="brand-name">Apex</span>
        </a>

        <div class="brand-divider" aria-hidden="true"></div>

        <!-- Project switcher -->
        <div class="project-switcher">
            <select
                class="project-select"
                on:change={onProjectChange}
                value={projectRouteId}
            >
                {#if projects.length === 0}
                    <option value={projectRouteId}
                        >{$currentProject?.name ?? "Project"}</option
                    >
                {:else}
                    {#each projects as project (project.id)}
                        <option value={project.id}>{project.name}</option>
                    {/each}
                {/if}
            </select>
            <ChevronDown size={12} class="project-chevron" />
        </div>

        <div class="topbar-actions">
            {#if canManageAccessControl() || ($accessControl.enabled && $activeAccessProfile.role === "subsystem")}
                <div class="control-cluster acl-cluster">
                    {#if canManageAccessControl()}
                        <button
                            class="topbar-btn"
                            class:active={$accessControl.enabled}
                            on:click={() => void onToggleAccessControl()}
                            title="Enable role-based access control"
                        >
                            {#if $accessControl.enabled}
                                <Shield size={13} />
                                <span>ACL On</span>
                            {:else}
                                <ShieldOff size={13} />
                                <span>ACL Off</span>
                            {/if}
                        </button>
                    {/if}

                    {#if $accessControl.enabled && canManageAccessControl()}
                        <select
                            class="topbar-select"
                            value={$activeAccessProfile.id}
                            on:change={(e) => void onAccessProfileChange(e)}
                            title="Active access profile"
                        >
                            {#each $accessControl.profiles as profile (profile.id)}
                                <option value={profile.id}
                                    >{profile.name}</option
                                >
                            {/each}
                        </select>
                    {/if}

                    {#if $accessControl.enabled && $activeAccessProfile.role === "subsystem"}
                        <select
                            class="topbar-select"
                            value={$activeAccessProfile.subsystem_id ?? ""}
                            on:change={(e) => void onSubsystemScopeChange(e)}
                            disabled={!(
                                canManageAccessControl() ||
                                $activeAccessProfile.role === "subsystem"
                            )}
                            title="Subsystem owner scope"
                        >
                            <option value="">No subsystem</option>
                            {#each subsystemNodes as sub (sub.id)}
                                <option value={sub.id}>{sub.name}</option>
                            {/each}
                        </select>
                    {/if}
                </div>
            {/if}

            <div class="control-cluster session-cluster">
                <button
                    class="topbar-btn"
                    class:active={$readOnly}
                    on:click={onToggleReadOnly}
                    title={$readOnly
                        ? "Switch to editable"
                        : "Switch to read-only"}
                >
                    {#if $readOnly}
                        <Lock size={13} />
                        <span>Read-only</span>
                    {:else}
                        <Unlock size={13} />
                        <span>Editable</span>
                    {/if}
                </button>

                <button
                    class="topbar-btn icon-only"
                    on:click={onToggleTheme}
                    title="Toggle theme"
                >
                    {#if theme === "dark"}
                        <Sun size={14} />
                    {:else}
                        <Moon size={14} />
                    {/if}
                </button>
            </div>

            {#if authEnabled && $authState.user}
                <div class="control-cluster account-cluster">
                    <span
                        class="account-pill"
                        title={$authState.user.email ?? "Signed-in account"}
                    >
                        {$authState.user.email ?? "Signed in"}
                    </span>
                    <button
                        class="topbar-btn"
                        on:click={onSignOut}
                        disabled={$authState.loading}
                        title="Sign out"
                    >
                        <LogOut size={13} />
                        <span>Sign out</span>
                    </button>
                </div>
            {/if}
        </div>
    </header>

    <!-- Nav bar -->
    <nav class="project-nav" aria-label="Project navigation">
        {#each navItems as item}
            {@const active = item.match(path, projectRouteId)}
            <a
                href={item.href(projectRouteId)}
                class="nav-link"
                class:active
                aria-current={active ? "page" : undefined}
            >
                {item.label}
            </a>
        {/each}
    </nav>

    <!-- Content -->
    <div class="app-shell-content">
        <div class="project-content page-enter">
            <slot />
        </div>
    </div>
</div>

<style>
    /* ── Top bar ── */
    .app-topbar {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        justify-content: space-between;
        padding: 0 var(--space-4);
        height: 54px;
        flex-shrink: 0;
        background: var(--surface-raised);
        border-bottom: 1px solid var(--surface-border-subtle);
    }

    /* Brand */
    .brand {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        text-decoration: none;
        flex-shrink: 0;
    }
    .brand-mark {
        display: flex;
        align-items: center;
        justify-content: center;
        flex-shrink: 0;
    }
    .brand-name {
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
        letter-spacing: var(--tracking-tight);
    }

    .brand-divider {
        width: 1px;
        height: 18px;
        background: var(--surface-border);
        flex-shrink: 0;
    }

    /* Project switcher */
    .project-switcher {
        display: flex;
        align-items: center;
        gap: 4px;
        position: relative;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        padding: 4px var(--space-2);
        min-width: 170px;
    }
    .project-select {
        appearance: none;
        background: none;
        border: none;
        color: var(--text-secondary);
        font-family: var(--font-sans);
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        cursor: pointer;
        padding-right: var(--space-4);
        max-width: 250px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .project-select:focus {
        outline: none;
    }
    :global(.project-chevron) {
        color: var(--text-muted);
        pointer-events: none;
        position: absolute;
        right: 0;
    }

    .topbar-actions {
        display: flex;
        align-items: center;
        justify-content: flex-end;
        gap: var(--space-2);
        min-width: 0;
        flex: 1;
    }

    .control-cluster {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        padding: 4px;
        border: 1px solid var(--surface-border-subtle);
        background: var(--surface-overlay);
        border-radius: var(--radius-md);
        min-width: 0;
    }

    /* Topbar buttons */
    .topbar-btn {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        padding: 6px var(--space-3);
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-muted);
        font-family: var(--font-sans);
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        cursor: pointer;
        transition: all var(--transition-fast);
        white-space: nowrap;
    }
    .topbar-btn:hover {
        background: var(--surface-hover);
        border-color: var(--surface-border-bright);
        color: var(--text-secondary);
    }
    .topbar-btn:active:not(:disabled) {
        transform: scale(0.96);
        background: var(--surface-border);
    }
    .topbar-btn.active {
        background: var(--accent-dim);
        border-color: var(--accent-border);
        color: var(--accent-hover);
    }
    .topbar-btn.active:active:not(:disabled) {
        background: var(--accent-glow);
    }
    .topbar-btn.icon-only {
        padding: 6px 9px;
    }
    .topbar-btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }

    .account-pill {
        border: 1px solid var(--surface-border);
        background: var(--surface-raised);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        padding: 4px var(--space-3);
        max-width: 220px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .topbar-select {
        background: var(--surface-raised);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-secondary);
        font-family: var(--font-sans);
        font-size: var(--text-xs);
        font-weight: var(--weight-medium);
        padding: 6px var(--space-2);
        min-width: 108px;
    }
    .topbar-select:focus {
        outline: none;
        border-color: var(--accent);
    }
    .topbar-select:disabled {
        opacity: 0.55;
        cursor: not-allowed;
    }

    @media (max-width: 1200px) {
        .app-topbar {
            flex-wrap: wrap;
            height: auto;
            padding: var(--space-2) var(--space-3);
            gap: var(--space-2);
        }

        .topbar-actions {
            width: 100%;
            justify-content: flex-start;
            flex-wrap: wrap;
        }
    }

    /* ── Nav bar ── */
    .project-nav {
        display: flex;
        align-items: center;
        gap: 2px;
        padding: 0 var(--space-4);
        height: 38px;
        flex-shrink: 0;
        background: var(--surface-raised);
        border-bottom: 1px solid var(--surface-border);
    }

    .nav-link {
        display: inline-flex;
        align-items: center;
        gap: var(--space-2);
        padding: 4px var(--space-3);
        border-radius: var(--radius-md);
        font-size: var(--text-sm);
        font-weight: var(--weight-medium);
        color: var(--text-muted);
        text-decoration: none;
        transition:
            color var(--transition-fast),
            background var(--transition-fast),
            transform var(--transition-fast);
        white-space: nowrap;
        position: relative;
    }
    .nav-link:hover {
        color: var(--text-secondary);
        background: var(--surface-hover);
    }
    .nav-link:active {
        transform: scale(0.97);
    }
    .nav-link.active {
        color: var(--text-primary);
        background: var(--accent-dim);
    }
    /* Left accent bar on active item */
    .nav-link::before {
        content: "";
        position: absolute;
        left: 0;
        top: 20%;
        height: 60%;
        width: 2px;
        background: var(--accent);
        border-radius: var(--radius-full);
        opacity: 0;
        transform: scaleY(0.5);
        transition:
            opacity var(--transition-fast),
            transform var(--transition-fast);
    }
    .nav-link.active::before {
        opacity: 1;
        transform: scaleY(1);
    }

    /* ── Content ── */
    .app-shell-content {
        flex: 1;
        min-height: 0;
        overflow: hidden;
        display: flex;
        flex-direction: column;
    }

    .project-content {
        flex: 1;
        overflow: hidden;
        min-height: 0;
        display: flex;
        flex-direction: column;
        height: 100%;
    }

    /* ── App shell ── */
    :global(.app-shell) {
        display: flex;
        flex-direction: column;
        height: 100vh;
        overflow: hidden;
        background: var(--surface-base);
    }
</style>

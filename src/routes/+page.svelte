<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import type { Project } from "$lib/types";
    import { goto } from "$app/navigation";
    import { applyTheme, getTheme, toggleTheme } from "$lib/theme";
    import { authEnabled } from "$lib/auth/nhost";
    import { authState, signOut } from "$lib/auth/store";
    import { Plus, Sun, Moon, ArrowRight, Clock, Layers, Trash2 } from "lucide-svelte";
    import { fade, fly, scale } from "svelte/transition";
    import { cubicOut } from "svelte/easing";

    let projects: Project[] = [];
    let creating = false;
    let loading = true;
    let creating_busy = false;
    let newName = "";
    let newDesc = "";
    let theme: "dark" | "light" = "dark";
    let confirmDeleteProject: Project | null = null;
    let deleting_busy = false;

    function focusOnMount(el: HTMLElement) {
        el.focus();
    }

    onMount(async () => {
        theme = getTheme();
        applyTheme(theme);
        try {
            projects = await invoke<Project[]>("list_projects");
        } finally {
            loading = false;
        }
    });

    async function createProject() {
        if (!newName.trim() || creating_busy) return;
        creating_busy = true;
        try {
            const project = await invoke<Project>("create_project", {
                name: newName.trim(),
                description: newDesc.trim(),
            });
            creating = false;
            newName = "";
            newDesc = "";
            projects = [project, ...projects];
            goto(`/project/${project.id}/system`);
        } finally {
            creating_busy = false;
        }
    }

    function formatDate(iso: string): string {
        const d = new Date(iso);
        const now = new Date();
        const diff = now.getTime() - d.getTime();
        const days = Math.floor(diff / 86400000);
        if (days === 0) return "Today";
        if (days === 1) return "Yesterday";
        if (days < 7) return `${days} days ago`;
        return d.toLocaleDateString(undefined, {
            month: "short",
            day: "numeric",
            year: "numeric",
        });
    }

    async function deleteProject() {
        if (!confirmDeleteProject || deleting_busy) return;
        deleting_busy = true;
        try {
            await invoke("delete_project", { id: confirmDeleteProject.id });
            projects = projects.filter((p) => p.id !== confirmDeleteProject!.id);
            confirmDeleteProject = null;
        } finally {
            deleting_busy = false;
        }
    }

    function onToggleTheme() {
        theme = toggleTheme(theme);
    }

    function onSignOut() {
        void signOut();
    }
</script>

<svelte:head><title>Apex — Systems Engineering</title></svelte:head>

<main class="dashboard" in:fade={{ duration: 200 }}>
    <!-- Top bar -->
    <header class="dash-topbar">
        <div class="brand">
            <svg
                width="22"
                height="22"
                viewBox="0 0 22 22"
                fill="none"
                aria-hidden="true"
            >
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
            <span class="brand-name">Apex</span>
        </div>

        <div class="dash-topbar-actions">
            {#if authEnabled && $authState.user}
                <span
                    class="session-pill"
                    title={$authState.user.email ?? "Signed-in account"}
                >
                    {$authState.user.email ?? "Signed in"}
                </span>
                <button
                    class="topbar-btn"
                    on:click={onSignOut}
                    disabled={$authState.loading}
                >
                    Sign out
                </button>
            {/if}
            <button
                class="topbar-btn icon-only"
                on:click={onToggleTheme}
                title="Toggle theme"
            >
                {#if theme === "dark"}<Sun size={14} />{:else}<Moon
                        size={14}
                    />{/if}
            </button>
        </div>
    </header>

    <!-- Hero -->
    <section class="hero">
        <div class="hero-inner">
            <div class="hero-eyebrow">Model-Based Systems Engineering</div>
            <h1 class="hero-title">
                Your projects,<br />engineered precisely.
            </h1>
            <p class="hero-sub">
                Design systems architecture, manage requirements, trace links,
                and extract requirements from documents — all in one
                offline-first workspace.
            </p>
            <button
                class="btn-primary hero-cta"
                on:click={() => (creating = true)}
            >
                <Plus size={15} />
                New Project
            </button>
        </div>

        <!-- Background grid decoration -->
        <div class="hero-grid" aria-hidden="true">
            <svg width="100%" height="100%" xmlns="http://www.w3.org/2000/svg">
                <defs>
                    <pattern
                        id="grid"
                        width="32"
                        height="32"
                        patternUnits="userSpaceOnUse"
                    >
                        <path
                            d="M 32 0 L 0 0 0 32"
                            fill="none"
                            stroke="var(--surface-border)"
                            stroke-width="0.5"
                        />
                    </pattern>
                </defs>
                <rect width="100%" height="100%" fill="url(#grid)" />
            </svg>
        </div>
    </section>

    <!-- Create modal -->
    {#if creating}
        <div
            class="modal-backdrop"
            role="button"
            tabindex="0"
            aria-label="Close"
            on:click={() => !creating_busy && (creating = false)}
            on:keydown={(e) => e.key === "Escape" && !creating_busy && (creating = false)}
            transition:fade={{ duration: 150 }}
        >
            <div
                class="modal"
                on:click|stopPropagation
                role="dialog"
                aria-label="New project"
                transition:fly={{ y: 12, duration: 180, easing: cubicOut }}
            >
                <h2>New Project</h2>
                <label class="field-label">
                    Name
                    <input
                        class="field"
                        placeholder="e.g. Autonomous UAS System"
                        bind:value={newName}
                        on:keydown={(e) => e.key === "Enter" && createProject()}
                        use:focusOnMount
                        disabled={creating_busy}
                    />
                </label>
                <label class="field-label">
                    Description <span class="optional">(optional)</span>
                    <textarea
                        class="field"
                        placeholder="Brief description of this project"
                        bind:value={newDesc}
                        rows="3"
                        disabled={creating_busy}
                    ></textarea>
                </label>
                <div class="modal-actions">
                    <button
                        class="btn-ghost"
                        on:click={() => (creating = false)}
                        disabled={creating_busy}
                    >Cancel</button>
                    <button
                        class="btn-primary"
                        on:click={createProject}
                        disabled={!newName.trim() || creating_busy}
                    >
                        {#if creating_busy}
                            <span class="spinner spinner-sm" aria-hidden="true"></span>
                            Creating…
                        {:else}
                            Create Project
                        {/if}
                    </button>
                </div>
            </div>
        </div>
    {/if}

    <!-- Delete confirmation modal -->
    {#if confirmDeleteProject}
        <div
            class="modal-backdrop"
            role="button"
            tabindex="0"
            aria-label="Close"
            on:click={() => !deleting_busy && (confirmDeleteProject = null)}
            on:keydown={(e) =>
                e.key === "Escape" &&
                !deleting_busy &&
                (confirmDeleteProject = null)}
            transition:fade={{ duration: 150 }}
        >
            <div
                class="modal"
                on:click|stopPropagation
                role="dialog"
                aria-label="Delete project"
                transition:fly={{ y: 12, duration: 180, easing: cubicOut }}
            >
                <h2 class="modal-delete-title">Delete Project?</h2>
                <p class="modal-delete-body">
                    <strong>{confirmDeleteProject.name}</strong> and all its
                    requirements, diagrams, and documents will be permanently
                    deleted. This cannot be undone.
                </p>
                <div class="modal-actions">
                    <button
                        class="btn-ghost"
                        on:click={() => (confirmDeleteProject = null)}
                        disabled={deleting_busy}>Cancel</button
                    >
                    <button
                        class="btn-danger"
                        on:click={deleteProject}
                        disabled={deleting_busy}
                    >
                        {#if deleting_busy}
                            <span
                                class="spinner spinner-sm"
                                aria-hidden="true"
                            ></span>
                            Deleting…
                        {:else}
                            Delete Project
                        {/if}
                    </button>
                </div>
            </div>
        </div>
    {/if}

    <!-- Projects section -->
    <section class="projects-section">
        <div class="projects-header">
            <div class="section-title">
                <Layers size={15} />
                Projects
                {#if projects.length > 0}
                    <span class="count-badge">{projects.length}</span>
                {/if}
            </div>
            {#if projects.length > 0}
                <button class="btn-primary" on:click={() => (creating = true)}>
                    <Plus size={13} />
                    New
                </button>
            {/if}
        </div>

        {#if loading}
            <!-- Skeleton loaders -->
            <div class="project-grid" in:fade={{ duration: 150 }}>
                {#each [1, 2, 3] as _}
                    <div class="project-card skeleton">
                        <div class="skeleton-line wide"></div>
                        <div class="skeleton-line short"></div>
                        <div class="skeleton-line medium"></div>
                    </div>
                {/each}
            </div>
        {:else if projects.length === 0}
            <!-- Empty state -->
            <div class="empty-state" in:fly={{ y: 10, duration: 200 }}>
                <div class="empty-illustration" aria-hidden="true">
                    <svg width="64" height="64" viewBox="0 0 64 64" fill="none">
                        <rect
                            x="4"
                            y="4"
                            width="24"
                            height="24"
                            rx="5"
                            fill="var(--accent)"
                            opacity="0.15"
                            stroke="var(--accent)"
                            stroke-width="1.5"
                        />
                        <rect
                            x="36"
                            y="4"
                            width="24"
                            height="24"
                            rx="5"
                            fill="var(--accent)"
                            opacity="0.08"
                            stroke="var(--surface-border)"
                            stroke-width="1.5"
                        />
                        <rect
                            x="4"
                            y="36"
                            width="24"
                            height="24"
                            rx="5"
                            fill="var(--accent)"
                            opacity="0.08"
                            stroke="var(--surface-border)"
                            stroke-width="1.5"
                        />
                        <rect
                            x="36"
                            y="36"
                            width="24"
                            height="24"
                            rx="5"
                            fill="var(--accent)"
                            opacity="0.04"
                            stroke="var(--surface-border-subtle)"
                            stroke-width="1.5"
                        />
                    </svg>
                </div>
                <div class="empty-state-title">No projects yet</div>
                <div class="empty-state-body">
                    Create your first project to start building a system model
                    with requirements, diagrams, and traceability.
                </div>
                <button class="btn-primary" on:click={() => (creating = true)}>
                    <Plus size={15} />
                    Create your first project
                </button>
            </div>
        {:else}
            <div class="project-grid" in:fade={{ duration: 200 }}>
                {#each projects as project (project.id)}
                    <a class="project-card" href="/project/{project.id}/system">
                        <div class="project-card-header">
                            <div class="project-icon" aria-hidden="true">
                                <svg
                                    width="18"
                                    height="18"
                                    viewBox="0 0 18 18"
                                    fill="none"
                                >
                                    <rect
                                        x="1"
                                        y="1"
                                        width="7"
                                        height="7"
                                        rx="1.5"
                                        fill="var(--accent)"
                                        opacity="0.8"
                                    />
                                    <rect
                                        x="10"
                                        y="1"
                                        width="7"
                                        height="7"
                                        rx="1.5"
                                        fill="var(--accent)"
                                        opacity="0.4"
                                    />
                                    <rect
                                        x="1"
                                        y="10"
                                        width="7"
                                        height="7"
                                        rx="1.5"
                                        fill="var(--accent)"
                                        opacity="0.4"
                                    />
                                    <rect
                                        x="10"
                                        y="10"
                                        width="7"
                                        height="7"
                                        rx="1.5"
                                        fill="var(--accent)"
                                        opacity="0.2"
                                    />
                                </svg>
                            </div>
                            <div class="card-header-right">
                                <button
                                    class="card-delete-btn"
                                    title="Delete project"
                                    on:click|preventDefault|stopPropagation={() =>
                                        (confirmDeleteProject = project)}
                                    aria-label="Delete {project.name}"
                                >
                                    <Trash2 size={13} />
                                </button>
                                <ArrowRight size={14} class="project-arrow" />
                            </div>
                        </div>

                        <div class="project-name">{project.name}</div>

                        {#if project.description}
                            <div class="project-desc">
                                {project.description}
                            </div>
                        {/if}

                        <div class="project-meta">
                            <Clock size={11} />
                            <span>{formatDate(project.modified_at)}</span>
                        </div>
                    </a>
                {/each}
            </div>
        {/if}
    </section>
</main>

<style>
    .dashboard {
        min-height: 100vh;
        background: var(--surface-base);
        display: flex;
        flex-direction: column;
    }

    /* ── Top bar ── */
    .dash-topbar {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0 var(--space-8);
        height: 52px;
        flex-shrink: 0;
        border-bottom: 1px solid var(--surface-border-subtle);
    }

    .brand {
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }
    .brand-name {
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
        letter-spacing: var(--tracking-tight);
    }

    .dash-topbar-actions {
        display: flex;
        gap: var(--space-2);
        align-items: center;
    }

    .topbar-btn {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        padding: 4px var(--space-3);
        background: none;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        color: var(--text-muted);
        font-size: var(--text-xs);
        cursor: pointer;
        transition: all var(--transition-fast);
    }
    .topbar-btn:hover {
        background: var(--surface-hover);
        border-color: var(--surface-border-bright);
        color: var(--text-secondary);
    }
    .topbar-btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
    }
    .topbar-btn.icon-only {
        padding: 4px 7px;
    }

    .session-pill {
        border: 1px solid var(--surface-border);
        background: var(--surface-overlay);
        border-radius: var(--radius-md);
        padding: 4px var(--space-3);
        color: var(--text-secondary);
        font-size: var(--text-xs);
        max-width: 220px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    /* ── Hero ── */
    .hero {
        position: relative;
        padding: var(--space-16) var(--space-8) var(--space-12);
        overflow: hidden;
        border-bottom: 1px solid var(--surface-border-subtle);
    }

    .hero-grid {
        position: absolute;
        inset: 0;
        opacity: 0.4;
        pointer-events: none;
        mask-image: radial-gradient(
            ellipse 80% 100% at 50% 0%,
            black 40%,
            transparent 100%
        );
        -webkit-mask-image: radial-gradient(
            ellipse 80% 100% at 50% 0%,
            black 40%,
            transparent 100%
        );
    }

    .hero-inner {
        position: relative;
        max-width: 560px;
        z-index: 1;
    }

    .hero-eyebrow {
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        text-transform: uppercase;
        letter-spacing: var(--tracking-wider);
        color: var(--accent-hover);
        margin-bottom: var(--space-3);
    }

    .hero-title {
        font-size: var(--text-3xl);
        font-weight: var(--weight-bold);
        letter-spacing: var(--tracking-tight);
        line-height: var(--leading-tight);
        color: var(--text-primary);
        margin-bottom: var(--space-4);
    }

    .hero-sub {
        font-size: var(--text-base);
        color: var(--text-secondary);
        line-height: var(--leading-relaxed);
        max-width: 440px;
        margin-bottom: var(--space-6);
    }

    .hero-cta {
        padding: var(--space-3) var(--space-5);
        font-size: var(--text-base);
    }

    /* ── Projects section ── */
    .projects-section {
        padding: var(--space-8);
        flex: 1;
    }

    .projects-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: var(--space-5);
    }

    .section-title {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        font-size: var(--text-sm);
        font-weight: var(--weight-semibold);
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: var(--tracking-wide);
    }

    .count-badge {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-width: 18px;
        height: 18px;
        padding: 0 5px;
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-full);
        font-size: var(--text-xs);
        font-weight: var(--weight-semibold);
        color: var(--text-muted);
    }

    /* ── Project grid ── */
    .project-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
        gap: var(--space-4);
    }

    .project-card {
        display: flex;
        flex-direction: column;
        gap: var(--space-2);
        background: var(--surface-overlay);
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-xl);
        padding: var(--space-5);
        text-decoration: none;
        transition: all var(--transition-fast);
        cursor: pointer;
    }
    .project-card:hover {
        border-color: var(--accent-border);
        background: var(--surface-hover);
        transform: translateY(-2px);
        box-shadow: var(--shadow-md);
    }

    .project-card-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: var(--space-1);
    }

    .project-icon {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 32px;
        height: 32px;
        background: var(--accent-dim);
        border: 1px solid var(--accent-border);
        border-radius: var(--radius-md);
    }

    .card-header-right {
        display: flex;
        align-items: center;
        gap: var(--space-2);
    }

    .card-delete-btn {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 26px;
        height: 26px;
        padding: 0;
        background: none;
        border: 1px solid transparent;
        border-radius: var(--radius-md);
        color: var(--text-muted);
        cursor: pointer;
        opacity: 0;
        transition: all var(--transition-fast);
    }
    .project-card:hover .card-delete-btn {
        opacity: 1;
    }
    .card-delete-btn:hover {
        background: #ef444420;
        border-color: #ef444440;
        color: #ef4444;
    }
    .card-delete-btn:active {
        transform: scale(0.92);
    }

    :global(.project-arrow) {
        color: var(--text-muted);
        opacity: 0;
        transition: all var(--transition-fast);
        transform: translateX(-4px);
    }
    .project-card:hover :global(.project-arrow) {
        opacity: 1;
        transform: translateX(0);
        color: var(--accent-hover);
    }

    .modal-delete-title {
        color: #ef4444;
    }
    .modal-delete-body {
        font-size: var(--text-sm);
        color: var(--text-secondary);
        line-height: var(--leading-relaxed);
        margin: var(--space-2) 0 var(--space-4);
    }

    .project-name {
        font-size: var(--text-base);
        font-weight: var(--weight-semibold);
        color: var(--text-primary);
        letter-spacing: var(--tracking-tight);
    }

    .project-desc {
        font-size: var(--text-sm);
        color: var(--text-secondary);
        line-height: var(--leading-relaxed);
        display: -webkit-box;
        -webkit-line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
    }

    .project-meta {
        display: flex;
        align-items: center;
        gap: 5px;
        font-size: var(--text-xs);
        color: var(--text-muted);
        margin-top: var(--space-1);
    }

    /* ── Empty state ── */
    .empty-state {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: var(--space-3);
        padding: var(--space-16) var(--space-8);
        text-align: center;
    }

    .empty-illustration {
        margin-bottom: var(--space-2);
    }

    .empty-state-title {
        font-size: var(--text-xl);
        font-weight: var(--weight-semibold);
        color: var(--text-secondary);
        letter-spacing: var(--tracking-tight);
    }

    .empty-state-body {
        font-size: var(--text-sm);
        color: var(--text-muted);
        max-width: 360px;
        line-height: var(--leading-relaxed);
        margin-bottom: var(--space-2);
    }

    /* ── Skeleton loaders ── */
    .project-card.skeleton {
        gap: var(--space-3);
        pointer-events: none;
    }

    .skeleton-line {
        height: 12px;
        background: linear-gradient(
            90deg,
            var(--surface-border) 0%,
            var(--surface-hover) 50%,
            var(--surface-border) 100%
        );
        background-size: 200% 100%;
        border-radius: var(--radius-sm);
        animation: shimmer 1.5s infinite;
    }
    .skeleton-line.wide {
        width: 70%;
    }
    .skeleton-line.medium {
        width: 50%;
    }
    .skeleton-line.short {
        width: 35%;
    }

    @keyframes shimmer {
        0% {
            background-position: 200% 0;
        }
        100% {
            background-position: -200% 0;
        }
    }

    /* ── Optional label ── */
    .optional {
        font-size: var(--text-xs);
        color: var(--text-muted);
        font-weight: var(--weight-normal);
    }
</style>

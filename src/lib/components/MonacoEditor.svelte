<script lang="ts">
    import { onMount, onDestroy, createEventDispatcher } from "svelte";

    export let value: string = "";
    export let language: string = "python";
    export let height: string = "200px";
    export let readonly: boolean = false;

    const dispatch = createEventDispatcher<{ change: string }>();

    let containerEl: HTMLDivElement;
    let editor: import("monaco-editor").editor.IStandaloneCodeEditor | null = null;
    let loading = true;
    let loadError = "";

    onMount(async () => {
        try {
            const monaco = await import("monaco-editor");

            // Match the app's current theme
            const isDark =
                document.documentElement.getAttribute("data-theme") !== "light";

            editor = monaco.editor.create(containerEl, {
                value,
                language,
                theme: isDark ? "vs-dark" : "vs",
                readOnly: readonly,
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                automaticLayout: true,
                fontSize: 13,
                lineNumbers: "on",
                wordWrap: "on",
                padding: { top: 8, bottom: 8 },
                tabSize: 4,
            });

            editor.onDidChangeModelContent(() => {
                dispatch("change", editor?.getValue() ?? "");
            });

            loading = false;
        } catch (e) {
            loadError = `Failed to load editor: ${e}`;
            loading = false;
        }
    });

    // Sync value from parent without triggering a change event loop
    $: if (editor && editor.getValue() !== value) {
        const pos = editor.getPosition();
        editor.setValue(value);
        if (pos) editor.setPosition(pos);
    }

    onDestroy(() => {
        editor?.dispose();
    });
</script>

<div class="monaco-wrapper" style="height: {height};">
    {#if loading}
        <div class="monaco-overlay">Loading editor...</div>
    {:else if loadError}
        <div class="monaco-overlay error">{loadError}</div>
    {/if}
    <div bind:this={containerEl} class="monaco-container"></div>
</div>

<style>
    .monaco-wrapper {
        position: relative;
        border: 1px solid var(--surface-border);
        border-radius: var(--radius-md);
        overflow: hidden;
        background: var(--surface-raised);
    }

    .monaco-container {
        width: 100%;
        height: 100%;
    }

    .monaco-overlay {
        position: absolute;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: var(--text-xs);
        color: var(--text-muted);
        background: var(--surface-raised);
        z-index: 1;
    }

    .monaco-overlay.error {
        color: var(--color-error, #ef4444);
        padding: var(--space-3);
        text-align: center;
    }
</style>

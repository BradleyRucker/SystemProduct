<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { loadProject } from '$lib/store/model';
  import SubsystemRequirements from '$lib/panels/SubsystemRequirements.svelte';

  $: projectId = $page.params.id;
  const subsystemName = 'Requirements Extractor';

  onMount(async () => {
    await loadProject(projectId);
  });
</script>

<div class="subsystem-page">
  <header class="subsystem-header page-header">
    <div>
      <div class="page-eyebrow">Subsystem</div>
      <h1 class="page-title">Requirements Extractor</h1>
      <p class="page-subtitle">Ingest source documents and extract structured requirements.</p>
    </div>
    <a class="btn-link" href="/project/{projectId}/system">Return to System Overview</a>
  </header>

  <div class="subsystem-body">
    <SubsystemRequirements subsystem={subsystemName} />

    <section class="artifact-panel">
      <div class="panel-header">
        <h2>Subsystem Artifacts</h2>
        <div class="panel-meta">Docs, models, tests, and repo links</div>
      </div>
      <div class="artifact-list">
        <div class="artifact-card">
          <div class="artifact-title">Add documentation</div>
          <div class="artifact-body">Link design docs, white papers, or specs here.</div>
        </div>
        <div class="artifact-card">
          <div class="artifact-title">Add repository link</div>
          <div class="artifact-body">Connect GitHub or internal repos for this subsystem.</div>
        </div>
        <div class="artifact-card">
          <div class="artifact-title">Add tests or simulations</div>
          <div class="artifact-body">Track verification assets and results.</div>
        </div>
      </div>
    </section>
  </div>
</div>

<style>
  .subsystem-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface-base);
  }

  .subsystem-header {
    display: flex;
    justify-content: space-between;
    gap: var(--space-6);
    align-items: flex-end;
  }

  .btn-link {
    color: var(--accent-hover);
    text-decoration: none;
    font-size: var(--text-sm);
  }

  .btn-link:hover { color: var(--text-primary); }

  .subsystem-body {
    display: grid;
    grid-template-columns: minmax(320px, 1fr) 320px;
    gap: var(--space-4);
    padding: var(--space-4);
    height: 100%;
    overflow: auto;
  }

  .artifact-panel {
    background: var(--surface-raised);
    border: 1px solid var(--surface-border);
    border-radius: var(--radius-xl);
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    height: fit-content;
  }

  .panel-header {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .panel-header h2 {
    font-size: var(--text-base);
    margin: 0;
  }

  .panel-meta {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .artifact-list {
    display: grid;
    gap: var(--space-2);
  }

  .artifact-card {
    border: 1px solid var(--surface-border);
    border-radius: var(--radius-lg);
    padding: var(--space-3);
    background: var(--surface-overlay);
  }

  .artifact-title {
    font-size: var(--text-sm);
    font-weight: var(--weight-semibold);
    margin-bottom: var(--space-1);
  }

  .artifact-body {
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }

  @media (max-width: 1100px) {
    .subsystem-body {
      grid-template-columns: 1fr;
    }
  }
</style>

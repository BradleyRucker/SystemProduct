<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { ReqComment } from '$lib/types';
  import { createEventDispatcher } from 'svelte';

  export let nodeId: string;
  export let projectId: string;

  const dispatch = createEventDispatcher();

  let comments: ReqComment[] = [];
  let newBody = '';
  let replyTo: string | null = null;
  let replyBody = '';
  let loading = false;

  async function load() {
    comments = await invoke<ReqComment[]>('get_req_comments', { nodeId });
  }

  async function submitComment() {
    if (!newBody.trim()) return;
    loading = true;
    try {
      await invoke('add_req_comment', {
        projectId,
        nodeId,
        parentId: null,
        author: 'User',
        body: newBody.trim(),
      });
      newBody = '';
      await load();
      dispatch('changed');
    } finally {
      loading = false;
    }
  }

  async function submitReply(parentId: string) {
    if (!replyBody.trim()) return;
    loading = true;
    try {
      await invoke('add_req_comment', {
        projectId,
        nodeId,
        parentId,
        author: 'User',
        body: replyBody.trim(),
      });
      replyBody = '';
      replyTo = null;
      await load();
      dispatch('changed');
    } finally {
      loading = false;
    }
  }

  async function resolveComment(id: string) {
    await invoke('resolve_req_comment', { id, resolvedBy: 'User' });
    await load();
    dispatch('changed');
  }

  async function deleteComment(id: string) {
    await invoke('delete_req_comment', { id });
    await load();
    dispatch('changed');
  }

  function fmtDate(iso: string) {
    return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  }

  $: if (nodeId) load();

  const topLevel = () => comments.filter(c => !c.parent_id && !c.resolved_at);
  const resolved = () => comments.filter(c => !c.parent_id && c.resolved_at);
  const replies = (parentId: string) => comments.filter(c => c.parent_id === parentId);
</script>

<div class="comment-thread">
  {#each topLevel() as comment}
    <div class="comment" class:resolved={comment.resolved_at}>
      <div class="comment-header">
        <span class="comment-author">{comment.author}</span>
        <span class="comment-date">{fmtDate(comment.created_at)}</span>
        <div class="comment-actions">
          <button class="action-btn" on:click={() => replyTo = replyTo === comment.id ? null : comment.id}>Reply</button>
          <button class="action-btn resolve-btn" on:click={() => resolveComment(comment.id)}>Resolve</button>
          <button class="action-btn delete-btn" on:click={() => deleteComment(comment.id)}>&#x2715;</button>
        </div>
      </div>
      <div class="comment-body">{comment.body}</div>

      {#each replies(comment.id) as reply}
        <div class="reply">
          <div class="comment-header">
            <span class="comment-author">{reply.author}</span>
            <span class="comment-date">{fmtDate(reply.created_at)}</span>
            <button class="action-btn delete-btn" on:click={() => deleteComment(reply.id)}>&#x2715;</button>
          </div>
          <div class="comment-body">{reply.body}</div>
        </div>
      {/each}

      {#if replyTo === comment.id}
        <div class="reply-compose">
          <textarea bind:value={replyBody} placeholder="Write a reply..." rows="2"></textarea>
          <div class="compose-actions">
            <button class="btn-secondary" on:click={() => { replyTo = null; replyBody = ''; }}>Cancel</button>
            <button class="btn-primary" on:click={() => submitReply(comment.id)} disabled={loading}>Reply</button>
          </div>
        </div>
      {/if}
    </div>
  {/each}

  {#if resolved().length > 0}
    <details class="resolved-section">
      <summary>{resolved().length} resolved comment{resolved().length === 1 ? '' : 's'}</summary>
      {#each resolved() as comment}
        <div class="comment resolved">
          <div class="comment-header">
            <span class="comment-author">{comment.author}</span>
            <span class="comment-date">{fmtDate(comment.created_at)}</span>
            <span class="resolved-label">Resolved by {comment.resolved_by}</span>
          </div>
          <div class="comment-body">{comment.body}</div>
        </div>
      {/each}
    </details>
  {/if}

  <div class="new-comment">
    <textarea bind:value={newBody} placeholder="Add a comment..." rows="2"></textarea>
    <button class="btn-primary" on:click={submitComment} disabled={loading || !newBody.trim()}>
      {loading ? 'Posting...' : 'Comment'}
    </button>
  </div>
</div>

<style>
  .comment-thread { display: flex; flex-direction: column; gap: 0.75rem; }
  .comment {
    background: var(--surface-1, #1a1a2e);
    border: 1px solid var(--border, #2a2a3e);
    border-radius: 6px;
    padding: 0.6rem 0.75rem;
    font-size: 0.85rem;
  }
  .comment.resolved { opacity: 0.6; }
  .comment-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.35rem;
  }
  .comment-author { font-weight: 600; color: var(--text-1, #e2e8f0); }
  .comment-date { font-size: 0.75rem; color: var(--text-3, #64748b); }
  .comment-actions { margin-left: auto; display: flex; gap: 0.3rem; }
  .comment-body { color: var(--text-2, #94a3b8); line-height: 1.4; }
  .reply {
    margin: 0.5rem 0 0 1rem;
    padding: 0.4rem 0.6rem;
    border-left: 2px solid var(--border, #2a2a3e);
    background: var(--surface-2, #12121f);
    border-radius: 0 4px 4px 0;
  }
  .reply-compose {
    margin-top: 0.5rem;
    margin-left: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  textarea {
    width: 100%;
    background: var(--surface-2, #12121f);
    border: 1px solid var(--border, #2a2a3e);
    border-radius: 4px;
    color: var(--text-1, #e2e8f0);
    padding: 0.4rem 0.6rem;
    font-size: 0.85rem;
    resize: vertical;
    font-family: inherit;
    box-sizing: border-box;
  }
  .compose-actions { display: flex; gap: 0.4rem; justify-content: flex-end; }
  .new-comment { display: flex; flex-direction: column; gap: 0.4rem; }
  .action-btn {
    padding: 0.15rem 0.5rem;
    background: transparent;
    border: 1px solid var(--border, #2a2a3e);
    border-radius: 3px;
    color: var(--text-3, #64748b);
    cursor: pointer;
    font-size: 0.75rem;
  }
  .action-btn:hover { background: var(--surface-2, #12121f); color: var(--text-1, #e2e8f0); }
  .resolve-btn:hover { border-color: #22c55e; color: #22c55e; }
  .delete-btn:hover { border-color: #ef4444; color: #ef4444; }
  .btn-primary {
    padding: 0.3rem 0.75rem;
    background: var(--accent, #6366f1);
    border: none;
    border-radius: 4px;
    color: white;
    cursor: pointer;
    font-size: 0.85rem;
    align-self: flex-end;
  }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-secondary {
    padding: 0.3rem 0.75rem;
    background: transparent;
    border: 1px solid var(--border, #2a2a3e);
    border-radius: 4px;
    color: var(--text-2, #94a3b8);
    cursor: pointer;
    font-size: 0.85rem;
  }
  .resolved-section {
    font-size: 0.8rem;
    color: var(--text-3, #64748b);
    cursor: pointer;
  }
  .resolved-label {
    font-size: 0.75rem;
    color: #22c55e;
    margin-left: auto;
  }
</style>

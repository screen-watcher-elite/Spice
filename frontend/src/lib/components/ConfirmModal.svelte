<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { AlertTriangle, X } from 'lucide-svelte';

  interface Props {
    open: boolean;
    title: string;
    message: string;
    confirmLabel?: string;
    tone?: 'amber' | 'crimson' | 'emerald';
    onConfirm: () => void;
    onCancel: () => void;
  }

  let {
    open,
    title,
    message,
    confirmLabel = 'Confirm',
    tone = 'amber',
    onConfirm,
    onCancel,
  }: Props = $props();

  function handleKey(e: KeyboardEvent) {
    if (open && e.key === 'Escape') {
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={handleKey} />

{#if open}
  <div class="modal-backdrop" transition:fade={{ duration: 130 }}>
    <button
      type="button"
      class="backdrop-btn"
      aria-label="Close confirmation dialog"
      onclick={onCancel}
    ></button>

    <div
      class="modal-card panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="confirm-title"
      transition:fly={{ y: 12, duration: 170 }}
    >
      <div class="m-head">
        <div class="m-title-row">
          <span class="m-icon {tone}">
            <AlertTriangle size={16} />
          </span>
          <h3 id="confirm-title">{title}</h3>
        </div>
        <button type="button" class="btn btn-sm" onclick={onCancel} aria-label="Cancel">
          <X size={14} />
        </button>
      </div>

      <p class="m-body">{message}</p>

      <div class="m-actions">
        <button type="button" class="btn" onclick={onCancel}>Cancel</button>
        <button
          type="button"
          class="btn {tone === 'crimson'
            ? 'btn-crimson'
            : tone === 'emerald'
              ? 'btn-emerald'
              : 'btn-amber'}"
          onclick={onConfirm}
        >
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
    background: rgba(4, 7, 12, 0.72);
    backdrop-filter: blur(5px);
    display: grid;
    place-items: center;
  }
  .backdrop-btn {
    position: fixed;
    inset: 0;
    border: none;
    background: transparent;
    cursor: pointer;
  }
  .modal-card {
    position: relative;
    z-index: 1;
    width: min(440px, 92vw);
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 24px 48px rgba(0, 0, 0, 0.6);
  }
  .m-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .m-title-row {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .m-title-row h3 {
    font-size: 14.5px;
    font-weight: 600;
  }
  .m-icon {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: grid;
    place-items: center;
  }
  .m-icon.amber {
    background: var(--amber-dim);
    color: var(--amber);
  }
  .m-icon.crimson {
    background: var(--crimson-dim);
    color: var(--crimson);
  }
  .m-icon.emerald {
    background: var(--emerald-dim);
    color: var(--emerald);
  }
  .m-body {
    font-size: 13px;
    color: var(--text-1);
    line-height: 1.55;
  }
  .m-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>

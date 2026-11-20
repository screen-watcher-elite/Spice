<script lang="ts">
  import { fade, scale } from 'svelte/transition';
  import { AlertTriangle, ShieldCheck, ShieldAlert } from 'lucide-svelte';

  interface Props {
    open: boolean;
    title: string;
    message: string;
    confirmLabel?: string;
    tone?: 'amber' | 'crimson' | 'emerald';
    onConfirm: () => void;
    onCancel: () => void;
  }

  let { open, title, message, confirmLabel = 'Confirm', tone = 'amber', onConfirm, onCancel }: Props = $props();

  const toneClass = $derived(tone === 'crimson' ? 'danger' : tone === 'emerald' ? 'ok' : 'warn');

  function handleKey(e: KeyboardEvent) {
    if (open && e.key === 'Escape') onCancel();
  }
</script>

<svelte:window onkeydown={handleKey} />

{#if open}
  <div class="backdrop" transition:fade={{ duration: 160 }}>
    <button type="button" class="dismiss" aria-label="Close dialog" onclick={onCancel}></button>
    <div
      class="dialog glass {toneClass}"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="confirm-title"
      transition:scale={{ start: 0.96, duration: 220, opacity: 0 }}
    >
      <div class="halo" aria-hidden="true"></div>
      <div class="icon">
        {#if toneClass === 'danger'}<ShieldAlert size={18} strokeWidth={1.7} />
        {:else if toneClass === 'ok'}<ShieldCheck size={18} strokeWidth={1.7} />
        {:else}<AlertTriangle size={18} strokeWidth={1.7} />{/if}
      </div>
      <h3 id="confirm-title">{title}</h3>
      <p>{message}</p>
      <div class="actions">
        <button type="button" class="btn btn-ghost" onclick={onCancel}>Cancel</button>
        <button
          type="button"
          class="btn {toneClass === 'danger' ? 'btn-danger' : toneClass === 'ok' ? 'btn-solid' : 'btn-warn'}"
          onclick={onConfirm}
        >
          {confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    background: var(--backdrop-bg);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
  }
  .dismiss {
    position: fixed;
    inset: 0;
    border: none;
    background: transparent;
    cursor: default;
  }
  .dialog {
    --c: var(--warn);
    --c-rgb: var(--warn-rgb);
    position: relative;
    z-index: 1;
    width: min(440px, 92vw);
    padding: 26px 26px 20px;
    border-radius: var(--r-xl);
    overflow: hidden;
    box-shadow: var(--shadow-pop);
  }
  .dialog.danger { --c: var(--danger); --c-rgb: var(--danger-rgb); }
  .dialog.ok { --c: var(--accent); --c-rgb: var(--accent-rgb); }
  .halo {
    position: absolute;
    top: -120px;
    left: 50%;
    width: 320px;
    height: 200px;
    transform: translateX(-50%);
    background: radial-gradient(closest-side, rgba(var(--c-rgb), 0.22), transparent);
    pointer-events: none;
  }
  .icon {
    position: relative;
    width: 40px;
    height: 40px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: rgba(var(--c-rgb), 0.1);
    border: 1px solid rgba(var(--c-rgb), 0.3);
    margin-bottom: 16px;
  }
  h3 {
    position: relative;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 26px;
    line-height: 1.1;
    letter-spacing: -0.01em;
  }
  p {
    position: relative;
    margin-top: 10px;
    font-size: 13px;
    line-height: 1.6;
    color: var(--fg-2);
  }
  .actions {
    position: relative;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 24px;
  }
</style>

<script lang="ts">
  import { segmentPii } from '../ui';

  interface Props {
    text: string | undefined | null;
  }
  let { text }: Props = $props();

  const segments = $derived(segmentPii(text));
</script>

{#each segments as seg, i (i)}{#if seg.pii}<span class="pii" title={seg.full}>{seg.pii}</span>{:else}{seg.text}{/if}{/each}

<script lang="ts">
	import type { TrailStep } from '$lib/api/trail';
	import ShownName from './ShownName.svelte';

	/** One path step's text: `~` (announced "My Den") for the root, else its name, isolated for right-to-left names, and its number. */
	let { step }: { step: TrailStep } = $props();
</script>

{#if step.step === 'root'}
	<span aria-hidden="true">~</span><span class="visually-hidden">My Den</span>
{:else}
	<ShownName name={step.label} ordinal={step.ordinal} />
{/if}

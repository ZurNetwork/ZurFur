<script lang="ts">
	import type { TrailStep } from '$lib/api/trail';

	/** One path step's text: `~` (announced "My Den") for the root, else its name, isolated for right-to-left names. */
	let { step }: { step: TrailStep } = $props();
</script>

{#if step.step === 'root'}
	<span aria-hidden="true">~</span><span class="visually-hidden">My Den</span>
{:else}
	<bdi>{step.label}</bdi>
{/if}

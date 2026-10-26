<script lang="ts">
	import { splitUsage } from './usage-text';

	interface Props {
		/** A usage line; backticks mark code. It is rendered as text, never as markup. */
		readonly text: string;
	}

	let { text }: Props = $props();

	const segments = $derived(splitUsage(text));
</script>

{#each segments as segment, index (index)}{#if segment.kind === 'code'}<code>{segment.text}</code
		>{:else}{segment.text}{/if}{/each}

<script lang="ts">
	import '@fontsource-variable/inter';
	import '@fontsource-variable/jetbrains-mono';
	import '$lib/styles/tokens.css';
	import '$lib/styles/base.css';
	import favicon from '$lib/assets/favicon.svg';
	import type { Snippet } from 'svelte';

	/** The root: tokens, base styles, self-hosted fonts and the favicon. Each group brings its own layout. */
	let { children }: { children: Snippet } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}

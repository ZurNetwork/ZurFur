<script lang="ts">
	import favicon from '$lib/assets/favicon.svg';
	import SessionHeader from '$lib/components/SessionHeader.svelte';
	import type { LayoutData } from './$types';
	import type { Snippet } from 'svelte';

	let { data, children }: { data: LayoutData; children: Snippet } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<SessionHeader session={data.session} />

{@render children()}

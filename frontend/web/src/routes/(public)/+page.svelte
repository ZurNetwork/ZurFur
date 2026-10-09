<script lang="ts">
	import { resolve } from '$app/paths';
	import { denRootPath } from '$lib/api/den-route';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head>
	<title>Zurfur</title>
</svelte:head>

<h1>Zurfur</h1>

{#if data.session !== undefined}
	<p data-testid="signed-in-as">Signed in as {data.session.handle ?? data.session.did}.</p>
	{#if data.denServed}
		<p><a href={denRootPath()} data-testid="open-my-den">Open My Den</a></p>
	{:else}
		<p>Your commissions will appear here.</p>
	{/if}
{:else}
	<p>Art commissions on the AT Protocol.</p>
	<a href={resolve('/login')} data-testid="signin-cta">Sign in</a>
{/if}

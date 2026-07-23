<script lang="ts">
	import { resolve } from '$app/paths';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
</script>

<svelte:head>
	<title>Zurfur</title>
</svelte:head>

<h1>Zurfur</h1>

{#if data.session !== null}
	<p data-testid="signed-in-as">Signed in as {data.session.handle ?? data.session.did}.</p>
	<p>Your commissions will appear here (ZMVP-153).</p>
{:else}
	<p>Art commissions on the AT Protocol.</p>
	<a href={resolve('/login')} data-testid="signin-cta">Sign in</a>
{/if}

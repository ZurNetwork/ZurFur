<script lang="ts">
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const health = $derived(data.health);
</script>

<h1>Zurfur web — dev loop</h1>

<section>
	<h2>Backend health (fetched via <code>/api/health</code>)</h2>
	{#if health.reachable}
		<p data-testid="health-status">Backend is up (HTTP {health.status}).</p>
		<pre data-testid="health-body">{JSON.stringify(health.body, null, 2)}</pre>
	{:else}
		<p data-testid="health-status">Backend is not reachable.</p>
		<p data-testid="health-note">{health.note}</p>
	{/if}
</section>

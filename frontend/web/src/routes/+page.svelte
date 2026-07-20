<script lang="ts">
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();
	const health = $derived(data.health);
	// Three honest states: a network failure is `!reachable`; otherwise a response
	// arrived, and only a 2xx status means the backend is actually "up" — a non-2xx
	// is reachable but erroring.
	const isUp = $derived(health.status !== null && health.status >= 200 && health.status < 300);
</script>

<h1>Zurfur web — dev loop</h1>

<section>
	<h2>Backend health (fetched via <code>/api/health</code>)</h2>
	{#if !health.reachable}
		<p data-testid="health-status">Backend is not reachable.</p>
		{#if health.note}
			<p data-testid="health-note">{health.note}</p>
		{/if}
	{:else if isUp}
		<p data-testid="health-status">Backend is up (HTTP {health.status}).</p>
		<pre data-testid="health-body">{JSON.stringify(health.body, null, 2)}</pre>
	{:else}
		<p data-testid="health-status">Backend is reachable but erroring (HTTP {health.status}).</p>
		{#if health.note}
			<p data-testid="health-note">{health.note}</p>
		{/if}
	{/if}
</section>

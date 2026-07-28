<script lang="ts">
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head>
	<title>{data.account?.handle ?? 'Account'} — Zurfur</title>
</svelte:head>

<h1>{data.account?.handle ?? 'Account'}</h1>

{#if data.problem}
	<ProblemNote problem={data.problem} />
{:else}
	{data.account.handle} -> {data.account.name} as {data.account.role}
	{#if data.account.role === 'owner'}
		<form method="post" action="?/delete">
			<button>Delete</button>
		</form>
		{#if form?.problem}
			<ProblemNote problem={form.problem} />
		{/if}
	{/if}
{/if}

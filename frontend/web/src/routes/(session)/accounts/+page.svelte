<script lang="ts">
	import { resolve } from '$app/paths';
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import type { ActionData, PageData } from './$types';
	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head>
	<title>Accounts — Zurfur</title>
</svelte:head>

<h1>Accounts</h1>

{#if data.deleted === 'soft'}
	<p>The account was deactivated.</p>
{:else if data.deleted === 'hard'}
	<p>The account was deleted.</p>
{:else if data.deleted}
	<p>The account was removed.</p>
{/if}

{#if data.problem}
	<ProblemNote problem={data.problem} />
{:else}
	{#each data.accounts as account (account.id)}
		<p>
			<a href={resolve('/(session)/accounts/[id]', { id: account.id })}>{account.handle}</a> -> {account.name}
			as {account.role}
		</p>
	{:else}
		<p data-testid="accounts-empty">No accounts yet - found one below.</p>
	{/each}
{/if}

<form method="post">
	<label>Name <input name="name" /></label>
	<label>Handle <input name="handle" /></label>
	<button>Found Account</button>
</form>

{#if form?.problem}
	<ProblemNote problem={form.problem} />
{/if}

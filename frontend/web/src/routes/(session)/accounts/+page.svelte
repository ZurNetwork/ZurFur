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

<!-- The three arms are the whole DeleteOutcome vocabulary (narrowed in load);
     'unknown' stays deliberately non-committal — R8 forbids reading the
     fallback as confirming either deletion. -->
{#if data.deleted === 'soft'}
	<p>The account was deactivated.</p>
{:else if data.deleted === 'hard'}
	<p>The account was deleted.</p>
{:else if data.deleted === 'unknown'}
	<p>The account was removed. It may still exist.</p>
{/if}

{#if data.problem}
	<ProblemNote problem={data.problem} />
{:else if data.accounts.length === 0}
	<p data-testid="accounts-empty">No accounts yet - found one below.</p>
{:else}
	<ul>
		{#each data.accounts as account (account.id)}
			<li>
				<a href={resolve('/(session)/accounts/[id]', { id: account.id })}>{account.handle}</a> -> {account.name}
				as {account.role}
			</li>
		{/each}
	</ul>
{/if}

<!-- Explicit action: posting to the bare pathname keeps a stale ?deleted=
     flash from riding along into a validation re-render. -->
<form method="post" action={resolve('/accounts')}>
	<label>Name <input name="name" required value={form?.name ?? ''} /></label>
	<label>
		Handle
		<input name="handle" required placeholder="studio.zurfur.app" value={form?.handle ?? ''} />
	</label>
	<button>Found Account</button>
</form>

{#if form?.problem}
	<ProblemNote problem={form.problem} />
{/if}

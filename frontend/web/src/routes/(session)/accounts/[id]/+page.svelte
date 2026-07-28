<script lang="ts">
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head>
	<title>{data.account?.handle ?? 'Account'} — Zurfur</title>
</svelte:head>

<h1>{data.account?.handle ?? 'Account'}</h1>

<!-- Top level, above every data-driven branch: the action's own feedback must
     never depend on the reloaded data's shape (a rejected delete can demote
     the role or drop the row, closing the very branch that would render it). -->
{#if form?.problem}
	<ProblemNote problem={form.problem} />
{/if}

{#if data.problem}
	<ProblemNote problem={data.problem} />
{:else}
	{data.account.handle} -> {data.account.name} as {data.account.role}
	{#if data.account.role === 'owner'}
		<form method="post" action="?/delete">
			<label>
				Type <code>{data.account.handle}</code> to confirm
				<input name="confirm" required autocomplete="off" />
				{#if form?.form?.errors.confirm}
					<p role="alert">{form.form.errors.confirm[0]}</p>
				{/if}
			</label>
			<button>Delete</button>
		</form>
	{/if}
{/if}

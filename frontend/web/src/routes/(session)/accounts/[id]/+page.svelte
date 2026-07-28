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
			<!-- Error OUTSIDE the label (a label's subtree becomes the input's
			     accessible name) and linked via aria-describedby instead. -->
			<label for="confirm-handle">Type <code>{data.account.handle}</code> to confirm</label>
			<input
				id="confirm-handle"
				name="confirm"
				required
				autocomplete="off"
				aria-invalid={form?.form?.errors.confirm ? 'true' : undefined}
				aria-describedby={form?.form?.errors.confirm ? 'confirm-error' : undefined}
			/>
			{#if form?.form?.errors.confirm}
				<p id="confirm-error">{form.form.errors.confirm[0]}</p>
			{/if}
			<button>Delete</button>
		</form>
	{/if}
{/if}

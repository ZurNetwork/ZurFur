<script lang="ts">
	import { applyAction, enhance } from '$app/forms';
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import { denRootFallback } from '$lib/components/explorer/den-root';
	import { treeMemoryFromContext } from '$lib/components/explorer/tree-memory.svelte';
	import type { SubmitFunction } from '@sveltejs/kit';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	// A deleted Account leaves the tree's `accounts` folder, so the frame's
	// tree memory starts over (keeping `~` open) before the page moves on.
	const treeMemory = treeMemoryFromContext();

	/**
	 * Applies the delete's result in place: a failure shows its problem without
	 * the page's reads running again (so a delete that wasn't answered still
	 * says it may have gone through), an error shows the frame's error view,
	 * and a completed delete starts the tree over and moves on.
	 */
	const applyDeleteResult: SubmitFunction =
		() =>
		async ({ result }) => {
			if (result.type === 'redirect') treeMemory?.restart(denRootFallback());
			await applyAction(result);
		};
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
	{data.account.handle} -> <bdi>{data.account.name}</bdi> as {data.account.role}
	{#if data.account.role === 'owner'}
		<form method="post" action="?/delete" use:enhance={applyDeleteResult}>
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

<script lang="ts">
	import { resolve } from '$app/paths';
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import { superForm } from 'sveltekit-superforms';
	import type { PageData } from './$types';
	let { data }: { data: PageData } = $props();
	// superForm seeds from the initial load value by design and manages its
	// own reactivity from there.
	// svelte-ignore state_referenced_locally
	const { form, errors, enhance, constraints, message } = superForm(data.form);

	// Under use:enhance a failed submit re-renders WITHOUT re-running load or
	// changing the URL, so a stale ?deleted= flash would survive every failed
	// create. A failed submit is visible as field errors or a form message —
	// suppress the flash once either exists.
	const submitFailed = $derived(
		$message !== undefined ||
			Object.values($errors).some((fieldErrors) => fieldErrors !== undefined)
	);
</script>

<svelte:head>
	<title>Accounts — Zurfur</title>
</svelte:head>

<h1>Accounts</h1>

<!-- The three arms are the whole DeleteOutcome vocabulary (narrowed in load);
     'unknown' stays deliberately non-committal — R8 forbids reading the
     fallback as confirming either deletion. The 'soft' arm is real but
     CURRENTLY UNREACHABLE in a browser (⚠️ F2): the backend's evidence check
     is a stub-false (accounts.rs `account_has_facts`), so every live delete
     lands 'hard' until facts exist. -->
{#if !submitFailed}
	{#if data.deleted === 'soft'}
		<p>The account was deactivated.</p>
	{:else if data.deleted === 'hard'}
		<p>The account was deleted.</p>
	{:else if data.deleted === 'unknown'}
		<p>The account was removed. It may still exist.</p>
	{/if}
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

<!-- Explicit action: posting to the bare pathname keeps the no-JS fallback
     from carrying a stale ?deleted= flash; under use:enhance the same job is
     done by the submitFailed guard above. -->
<form method="post" action={resolve('/accounts')} use:enhance>
	<label
		>Name <input
			name="name"
			required
			bind:value={$form.name}
			aria-invalid={$errors.name ? 'true' : undefined}
			aria-describedby={$errors.name ? 'name-error' : undefined}
			{...$constraints.name}
		/></label
	>
	{#if $errors.name}
		<p role="alert" id="name-error" data-testid="name-error">{$errors.name}</p>
	{/if}
	<label>
		Handle
		<input
			name="handle"
			required
			placeholder="studio.zurfur.app"
			bind:value={$form.handle}
			aria-invalid={$errors.handle ? 'true' : undefined}
			aria-describedby={$errors.handle ? 'handle-error' : undefined}
			{...$constraints.handle}
		/>
	</label>
	{#if $errors.handle}
		<p role="alert" id="handle-error" data-testid="handle-error">{$errors.handle}</p>
	{/if}
	<button>Found Account</button>
</form>

{#if $message}
	<ProblemNote problem={$message} />
{/if}

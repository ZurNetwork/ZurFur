<script lang="ts">
	import { onMount } from 'svelte';
	import { parseSkin, SKIN_ATTRIBUTE, type Skin } from '../lib/styles/environment.ts';
	import { writeSkin } from './skin-store.ts';

	/** A skin, or following the OS. */
	type Choice = Skin | 'os';

	interface Option {
		readonly choice: Choice;
		readonly label: string;
	}

	const options: readonly Option[] = [
		{ choice: 'os', label: 'Follow the OS' },
		{ choice: 'dark', label: 'Paper dark' },
		{ choice: 'light', label: 'Paper light' }
	];

	const group = $props.id();

	let current: Choice = $state('os');

	onMount(() => {
		const onRoot = document.documentElement.getAttribute(SKIN_ATTRIBUTE) ?? undefined;
		current = parseSkin(onRoot) ?? 'os';
	});

	/** Wears the choice on the page root and stores it; a failed save still applies to this page view. */
	function choose(choice: Choice): void {
		current = choice;
		const root = document.documentElement;
		const skin = choice === 'os' ? undefined : choice;
		if (skin === undefined) root.removeAttribute(SKIN_ATTRIBUTE);
		else root.setAttribute(SKIN_ATTRIBUTE, skin);
		writeSkin(skin);
	}
</script>

<fieldset class="skin-switch">
	<legend>Skin</legend>
	{#each options as option (option.choice)}
		<label>
			<input
				type="radio"
				name={group}
				value={option.choice}
				checked={current === option.choice}
				onchange={() => {
					choose(option.choice);
				}}
			/>
			{option.label}
		</label>
	{/each}
</fieldset>

<style>
	.skin-switch {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
		margin: 0;
		padding: 0;
		border: none;
		color: var(--color-text);
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	legend {
		float: left;
		padding: 0;
		margin-inline-end: var(--space-2);
		color: var(--color-text-muted);
	}

	label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	input {
		margin: 0;
		accent-color: var(--color-accent);
	}
</style>

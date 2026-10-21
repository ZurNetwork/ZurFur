<script lang="ts">
	/**
	 * A labelled checkbox. `description` is read with it and shown under the
	 * label, in muted text while unticked (such as a value, or "Not sent");
	 * `note` is read after it and shown beneath it, always muted.
	 */
	let {
		checked = $bindable(false),
		label,
		description,
		note,
		name,
		testid
	}: {
		checked?: boolean;
		label: string;
		description?: string | undefined;
		note?: string | undefined;
		name?: string | undefined;
		testid?: string | undefined;
	} = $props();

	const ownId = $props.id();
	const boxId = `${ownId}-box`;
	const descriptionId = `${ownId}-description`;
	const noteId = `${ownId}-note`;

	/** The description and the note, whichever are shown, in reading order. */
	const describedBy = $derived.by(() => {
		const parts = [
			...(description === undefined ? [] : [descriptionId]),
			...(note === undefined ? [] : [noteId])
		];
		return parts.length === 0 ? undefined : parts.join(' ');
	});
</script>

<div class="checkbox" data-testid={testid}>
	<input
		class="checkbox__box"
		type="checkbox"
		id={boxId}
		{name}
		bind:checked
		aria-describedby={describedBy}
	/>
	<div class="checkbox__text">
		<label class="checkbox__label" for={boxId}>{label}</label>
		{#if description !== undefined}
			<span
				class="checkbox__description"
				class:checkbox__description--unticked={!checked}
				id={descriptionId}>{description}</span
			>
		{/if}
		{#if note !== undefined}
			<span class="checkbox__note" id={noteId}>{note}</span>
		{/if}
	</div>
</div>

<style>
	.checkbox {
		display: flex;
		align-items: flex-start;
		gap: var(--space-2);
		min-width: 0;
	}

	.checkbox__box {
		appearance: none;
		position: relative;
		flex: none;
		width: var(--space-5);
		height: var(--space-5);
		min-height: 0;
		margin: calc(var(--space-1) / 2) 0 0;
		padding: 0;
		border: var(--border-width) solid var(--color-control-edge);
		border-radius: var(--radius-sm);
		background: var(--color-surface-raised);
		cursor: pointer;
	}

	.checkbox__box:checked {
		border-color: var(--color-accent);
		background: var(--color-accent);
	}

	/* The tick: two edges of a box, turned. */
	.checkbox__box:checked::after {
		content: '';
		position: absolute;
		top: 12%;
		left: 32%;
		width: 30%;
		height: 55%;
		border: solid var(--color-on-accent);
		border-width: 0 2px 2px 0;
		transform: rotate(45deg);
	}

	.checkbox__text {
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.checkbox__label {
		display: block;
		margin: 0;
		color: var(--color-text);
		font-size: var(--text-sm);
		cursor: pointer;
	}

	.checkbox__description {
		color: var(--color-text);
		font-size: var(--text-sm);
		overflow-wrap: anywhere;
	}

	.checkbox__description--unticked {
		color: var(--color-text-muted);
	}

	.checkbox__note {
		margin-top: var(--space-1);
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}

	@media (forced-colors: active) {
		.checkbox__box {
			appearance: auto;
		}

		.checkbox__box:checked::after {
			content: none;
		}
	}
</style>

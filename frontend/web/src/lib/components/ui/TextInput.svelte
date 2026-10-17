<script lang="ts">
	/**
	 * A labelled text box, one line or several. Multi-line it is a textarea of
	 * at least six rows; it never caps its length, so the caller's rule can say
	 * why text is too long. An `error` marks it invalid and is read with it.
	 */
	let {
		value = $bindable(''),
		label,
		multiline = false,
		hint,
		error,
		required = false,
		name,
		id,
		testid
	}: {
		value?: string;
		label: string;
		multiline?: boolean;
		hint?: string | undefined;
		error?: string | undefined;
		required?: boolean;
		name?: string | undefined;
		id?: string | undefined;
		testid?: string | undefined;
	} = $props();

	const ownId = $props.id();
	const fieldId = $derived(id ?? `${ownId}-field`);
	const hintId = `${ownId}-hint`;
	const errorId = `${ownId}-error`;

	/** The hint and the error, whichever are shown, as the field's description. */
	const describedBy = $derived.by(() => {
		const parts = [
			...(hint === undefined ? [] : [hintId]),
			...(error === undefined ? [] : [errorId])
		];
		return parts.length === 0 ? undefined : parts.join(' ');
	});

	const invalid = $derived(error === undefined ? undefined : 'true');
	const ariaRequired = $derived(required ? 'true' : undefined);
</script>

<div class="text-input" data-testid={testid}>
	<label class="text-input__label" for={fieldId}>{label}</label>
	{#if hint !== undefined}
		<span class="text-input__hint" id={hintId}>{hint}</span>
	{/if}
	{#if multiline}
		<textarea
			class="text-input__field text-input__field--multi"
			id={fieldId}
			{name}
			rows="6"
			bind:value
			aria-invalid={invalid}
			aria-describedby={describedBy}
			aria-required={ariaRequired}></textarea>
	{:else}
		<input
			class="text-input__field"
			id={fieldId}
			{name}
			type="text"
			bind:value
			aria-invalid={invalid}
			aria-describedby={describedBy}
			aria-required={ariaRequired}
		/>
	{/if}
	{#if error !== undefined}
		<span class="text-input__error" id={errorId}>{error}</span>
	{/if}
</div>

<style>
	.text-input {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
	}

	.text-input__label {
		display: block;
		margin: 0;
		color: var(--color-text);
		font-size: var(--text-sm);
		font-weight: var(--text-weight-medium);
	}

	.text-input__hint {
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}

	/* 16 px, so a phone does not zoom in when the box takes focus. */
	.text-input__field {
		width: 100%;
		font-size: var(--text-md);
		line-height: var(--text-line-height);
	}

	.text-input__field--multi {
		resize: vertical;
	}

	.text-input__field[aria-invalid='true'] {
		border-color: var(--color-danger);
	}

	.text-input__error {
		color: var(--color-danger);
		font-size: var(--text-sm);
	}
</style>

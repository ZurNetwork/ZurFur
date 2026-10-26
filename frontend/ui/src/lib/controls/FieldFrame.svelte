<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from '../icons/Icon.svelte';
	import type { FieldIds } from './types';

	interface Props {
		readonly label: string;
		/** Keeps the label for screen readers only. */
		readonly labelHidden?: boolean;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		/** The control, handed the ids it points its label and description at. */
		readonly children: Snippet<[FieldIds]>;
	}

	let { label, labelHidden = false, hint, error, children }: Props = $props();

	const id = $props.id();
	const labelId = `${id}-label`;
	const hintId = `${id}-hint`;
	const errorId = `${id}-error`;

	/** The hint and the error, whichever are shown, in reading order. */
	const describedBy = $derived.by(() => {
		const shown = [
			...(hint === undefined ? [] : [hintId]),
			...(error === undefined ? [] : [errorId])
		];
		return shown.length === 0 ? undefined : shown.join(' ');
	});

	const ids: FieldIds = $derived({
		labelId,
		describedBy,
		invalid: error === undefined ? undefined : 'true'
	});
</script>

<!-- The label wraps the control, so a click on the words reaches it; the control names itself by `labelId`. -->
<label class="zf-field">
	<span class={['zf-field__label', labelHidden && 'zf-field__label--hidden']} id={labelId}
		>{label}</span
	>
	{@render children(ids)}
	{#if hint !== undefined}<span class="zf-field__hint" id={hintId}>{hint}</span>{/if}
	{#if error !== undefined}
		<span class="zf-field__error" id={errorId}><Icon name="circle-alert" size="sm" />{error}</span>
	{/if}
</label>

<script lang="ts">
	import Icon from '../icons/Icon.svelte';
	import type { IconName } from '../icons/icons';
	import FieldFrame from './FieldFrame.svelte';
	import type { FieldIds } from './types';

	interface Props {
		readonly label: string;
		/** Keeps the label for screen readers only; use it only where a visible heading names the field. */
		readonly labelHidden?: boolean;
		readonly value: string;
		/** Handed the text the field holds after an edit; the parent decides whether `value` follows. */
		readonly onChange: (text: string) => void;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		/** A leading glyph. */
		readonly icon?: IconName | undefined;
		/** Leading text, such as `@` before a handle. */
		readonly prefix?: string | undefined;
		/** For handles, DIDs and paths. */
		readonly mono?: boolean;
		/** A textarea instead of a single line. */
		readonly multiline?: boolean;
		readonly testid?: string | undefined;
		/** With a name, a form can post the field; without one it never does. */
		readonly name?: string | undefined;
		readonly type?: 'text' | 'search';
		readonly autocomplete?: 'off' | 'on' | undefined;
		readonly autocorrect?: 'off' | 'on' | undefined;
		readonly spellcheck?: boolean | undefined;
		readonly autocapitalize?: 'off' | 'sentences' | undefined;
		readonly inputmode?: 'text' | 'search' | 'numeric' | 'none' | undefined;
		readonly required?: boolean;
		readonly disabled?: boolean;
		readonly readonly?: boolean;
		readonly placeholder?: string | undefined;
	}

	let {
		label,
		labelHidden = false,
		value,
		onChange,
		hint,
		error,
		icon,
		prefix,
		mono = false,
		multiline = false,
		testid,
		name,
		type = 'text',
		autocomplete,
		autocorrect,
		spellcheck,
		autocapitalize,
		inputmode,
		required = false,
		disabled = false,
		readonly = false,
		placeholder
	}: Props = $props();

	/** Sets the textarea's `autocorrect`, which Svelte's textarea typings lack, as a plain attribute. */
	function autocorrection(element: HTMLTextAreaElement): void {
		if (autocorrect === undefined) element.removeAttribute('autocorrect');
		else element.setAttribute('autocorrect', autocorrect);
	}

	const classes = $derived([
		'zf-input',
		'slip',
		mono && 'zf-input--mono',
		multiline && 'zf-input--multi',
		error !== undefined && 'zf-input--invalid',
		disabled && 'zf-input--disabled'
	]);
</script>

<FieldFrame {label} {labelHidden} {hint} {error}>
	{#snippet children(ids: FieldIds)}
		<span class={classes}>
			{#if icon !== undefined}<span class="zf-input__icon"><Icon name={icon} /></span>{/if}
			{#if prefix !== undefined}<span class="zf-input__prefix" aria-hidden="true">{prefix}</span
				>{/if}
			{#if multiline}
				<textarea
					rows="4"
					bind:value={
						() => value,
						(next: string) => {
							onChange(next);
						}
					}
					aria-labelledby={ids.labelId}
					aria-describedby={ids.describedBy}
					aria-invalid={ids.invalid}
					data-testid={testid}
					{name}
					{autocomplete}
					{spellcheck}
					{autocapitalize}
					{inputmode}
					{required}
					{disabled}
					{readonly}
					{placeholder}
					{@attach autocorrection}></textarea>
			{:else}
				<input
					{type}
					bind:value={
						() => value,
						(next: string) => {
							onChange(next);
						}
					}
					aria-labelledby={ids.labelId}
					aria-describedby={ids.describedBy}
					aria-invalid={ids.invalid}
					data-testid={testid}
					{name}
					{autocomplete}
					{autocorrect}
					{spellcheck}
					{autocapitalize}
					{inputmode}
					{required}
					{disabled}
					{readonly}
					{placeholder}
				/>
			{/if}
		</span>
	{/snippet}
</FieldFrame>

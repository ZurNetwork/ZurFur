<script lang="ts">
	import { tick } from 'svelte';
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';

	interface Props {
		readonly label: string;
		readonly description?: string | undefined;
		/** `'mixed'` shows a dash: some of what the box stands for is on. */
		readonly checked: boolean | 'mixed';
		/** Handed the value a press asks for; the parent decides whether `checked` follows. */
		readonly onCheckedChange: (checked: boolean) => void;
		readonly disabled?: boolean;
		/** With a name, a form can post the box; without one it never does. */
		readonly name?: string | undefined;
		readonly value?: string | undefined;
		readonly testid?: string | undefined;
	}

	let {
		label,
		description,
		checked,
		onCheckedChange,
		disabled = false,
		name,
		value,
		testid
	}: Props = $props();

	const { moves, sounds } = useEnvironment();
	const id = $props.id();

	/** A change: plays the toggle cue and asks the parent, then shows whatever `checked` says next. */
	async function change(
		event: Event & { readonly currentTarget: HTMLInputElement }
	): Promise<void> {
		const input = event.currentTarget;
		sounds.play('toggle');
		onCheckedChange(input.checked);
		await tick();
		input.checked = checked === true;
		input.indeterminate = checked === 'mixed';
	}
</script>

<div class="zf-check">
	<input
		class="zf-check__input"
		type="checkbox"
		id="{id}-box"
		checked={checked === true}
		indeterminate={checked === 'mixed'}
		aria-describedby={description === undefined ? undefined : `${id}-desc`}
		data-testid={testid}
		onchange={change}
		{disabled}
		{name}
		{value}
	/>
	<!-- The box is a second label for the input, so a press on it lands on the box and reaches the input. -->
	<label class="zf-check__box" for="{id}-box" {@attach moves.squish()}>
		<span class="zf-check__tick"><Icon name="check" size="sm" weight="heavy" /></span>
		<span class="zf-check__dash"><Icon name="minus" size="sm" weight="heavy" /></span>
	</label>
	<span class="zf-check__text">
		<label class="zf-check__label" for="{id}-box">{label}</label>
		{#if description !== undefined}<span class="zf-check__desc" id="{id}-desc">{description}</span
			>{/if}
	</span>
</div>

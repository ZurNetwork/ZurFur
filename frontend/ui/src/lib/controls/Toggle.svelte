<script lang="ts">
	import { Switch } from 'bits-ui';
	import { useEnvironment } from '../environment/context';

	interface Props {
		readonly label: string;
		readonly description?: string | undefined;
		readonly checked: boolean;
		/** Handed the value a flip asks for; the parent decides whether `checked` follows. */
		readonly onCheckedChange: (checked: boolean) => void;
		readonly disabled?: boolean;
		/** With a name, a hidden checkbox carries the value in a form. */
		readonly name?: string | undefined;
		readonly testid?: string | undefined;
	}

	let {
		label,
		description,
		checked,
		onCheckedChange,
		disabled = false,
		name,
		testid
	}: Props = $props();

	const { moves, sounds } = useEnvironment();
	const id = $props.id();

	/** A flip: plays the toggle cue and asks the parent; the switch shows whatever `checked` says next. */
	function choose(next: boolean): void {
		sounds.play('toggle');
		onCheckedChange(next);
	}
</script>

<!-- The track, shared by both roots below: bits-ui's props on the button, its thumb inside. -->
{#snippet child({ props }: { readonly props: Record<string, unknown> })}
	<button
		{...props}
		class="zf-toggle__track"
		data-testid={testid}
		aria-describedby={description === undefined ? undefined : `${id}-desc`}
		{@attach moves.squish()}><Switch.Thumb class="zf-toggle__thumb placed glidex" /></button
	>
{/snippet}

<div class="zf-toggle">
	<!-- bits-ui's `name` takes no undefined, so the root without a name is its own branch. -->
	{#if name === undefined}
		<Switch.Root id="{id}-track" bind:checked={() => checked, choose} {disabled} {child} />
	{:else}
		<Switch.Root id="{id}-track" bind:checked={() => checked, choose} {disabled} {name} {child} />
	{/if}
	<span class="zf-toggle__text">
		<label class="zf-toggle__label" for="{id}-track">{label}</label>
		{#if description !== undefined}<span class="zf-toggle__desc" id="{id}-desc">{description}</span
			>{/if}
	</span>
</div>

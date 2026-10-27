<script lang="ts">
	import { Checkbox } from '@zurfur/ui';

	interface Props {
		readonly label: string;
		readonly description?: string | undefined;
		readonly checked: boolean | 'mixed';
		readonly onCheckedChange: (checked: boolean) => void;
		readonly disabled?: boolean | undefined;
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

	/** What the box shows: the story's value, until a change replaces it. */
	let shown = $derived(checked);

	/** Hands the change to the story's callback, then shows it. */
	function change(next: boolean): void {
		onCheckedChange(next);
		shown = next;
	}
</script>

<Checkbox
	{label}
	{description}
	checked={shown}
	onCheckedChange={change}
	{disabled}
	{name}
	{value}
	{testid}
/>

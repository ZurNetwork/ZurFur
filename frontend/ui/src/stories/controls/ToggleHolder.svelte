<script lang="ts">
	import { Toggle } from '@zurfur/ui';

	interface Props {
		readonly label: string;
		readonly description?: string | undefined;
		readonly checked: boolean;
		readonly onCheckedChange: (checked: boolean) => void;
		readonly disabled?: boolean | undefined;
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

	/** What the switch shows: the story's value, until a flip replaces it. */
	let shown = $derived(checked);

	/** Hands the flip to the story's callback, then shows it. */
	function flip(next: boolean): void {
		onCheckedChange(next);
		shown = next;
	}
</script>

<Toggle {label} {description} checked={shown} onCheckedChange={flip} {disabled} {name} {testid} />

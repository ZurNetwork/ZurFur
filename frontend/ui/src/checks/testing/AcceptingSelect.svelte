<script lang="ts" generics="V extends string">
	import Select from '../../lib/controls/Select.svelte';
	import type { SelectOption } from '../../lib/controls/types.ts';

	interface Props {
		readonly label: string;
		readonly options: readonly SelectOption<V>[];
		/** The value chosen to begin with, or nothing. */
		readonly initial: V | undefined;
		readonly placeholder?: string | undefined;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		readonly name?: string | undefined;
		readonly testid?: string | undefined;
		/** Told of every value asked for; this parent then accepts it. */
		readonly onChange: (value: V) => void;
	}

	let { label, options, initial, placeholder, hint, error, name, testid, onChange }: Props =
		$props();

	/** The parent's own state: a select is controlled, so whoever renders it holds `value`. */
	let chosen = $state<V>();
	const value = $derived(chosen ?? initial);

	function accept(next: V): void {
		onChange(next);
		chosen = next;
	}
</script>

<Select {label} {options} {value} {placeholder} {hint} {error} {name} {testid} onChange={accept} />

<script lang="ts" generics="Id extends string">
	import Menu from '../../lib/controls/Menu.svelte';
	import type { MenuItem, MenuTrigger } from '../../lib/controls/types.ts';

	interface Props {
		readonly trigger: MenuTrigger;
		readonly items: readonly MenuItem<Id>[];
		readonly testid?: string | undefined;
		/** Told of every open and close asked for; this parent then accepts it. */
		readonly onOpenChange: (open: boolean) => void;
		readonly onSelect: (id: Id) => void;
	}

	let { trigger, items, testid, onOpenChange, onSelect }: Props = $props();

	/** The parent's own state: a menu is controlled, so whoever renders it holds `open`. */
	let open = $state(false);

	function accept(next: boolean): void {
		onOpenChange(next);
		open = next;
	}
</script>

<Menu {trigger} {items} {open} {testid} onOpenChange={accept} {onSelect} />

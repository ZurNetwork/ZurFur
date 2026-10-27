<script lang="ts">
	import { Menu, type MenuItem, type MenuTrigger } from '@zurfur/ui';

	interface Props {
		readonly trigger: MenuTrigger;
		readonly items: readonly MenuItem<string>[];
		readonly open: boolean;
		readonly onOpenChange: (open: boolean) => void;
		readonly onSelect: (id: string) => void;
	}

	let { trigger, items, open, onOpenChange, onSelect }: Props = $props();

	/** Whether the menu is open: the story's value to begin with, then whatever is asked for. */
	let showing = $derived(open);

	/** Hands the request to the story's spy, then follows it. */
	function take(next: boolean): void {
		onOpenChange(next);
		showing = next;
	}
</script>

<Menu {trigger} {items} open={showing} onOpenChange={take} {onSelect} />

<script lang="ts">
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';
	import type { IconName } from '../icons/icons';
	import Tooltip from './Tooltip.svelte';
	import type { IconButtonSize, IconButtonVariant } from './types';

	interface Props {
		readonly icon: IconName;
		/** What it does, such as "Copy path": its accessible name and the words its tooltip shows. */
		readonly label: string;
		readonly variant?: IconButtonVariant;
		readonly size?: IconButtonSize;
		/** For an on/off tool: sets `aria-pressed` and the selected tint. */
		readonly pressed?: boolean | undefined;
		/** For a button that shows or hides something: sets `aria-expanded`. */
		readonly expanded?: boolean | undefined;
		/** The id of the element it shows, hides or changes: sets `aria-controls`. */
		readonly controls?: string | undefined;
		readonly disabled?: boolean;
		readonly onPress: () => void;
		readonly testid?: string | undefined;
	}

	let {
		icon,
		label,
		variant = 'quiet',
		size = 'md',
		pressed,
		expanded,
		controls,
		disabled = false,
		onPress,
		testid
	}: Props = $props();

	const { moves } = useEnvironment();

	const iconSize = $derived(size === 'sm' ? 'sm' : 'md');
	const classes = $derived([
		'zf-iconbtn',
		`zf-iconbtn--${variant}`,
		size === 'sm' && 'zf-iconbtn--sm',
		variant === 'secondary' ? 'placed' : 'pick'
	]);
</script>

<!-- The button itself; the tooltip hands it the props that show and hide the tip. -->
{#snippet child({ props }: { readonly props: Record<string, unknown> })}
	<button
		{...props}
		type="button"
		class={classes}
		aria-label={label}
		aria-pressed={pressed}
		aria-expanded={expanded}
		aria-controls={controls}
		data-testid={testid}
		{@attach moves.squish()}><Icon name={icon} size={iconSize} /></button
	>
{/snippet}

<Tooltip words={label} trigger={child} {onPress} {disabled} />

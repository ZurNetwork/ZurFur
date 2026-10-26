<script lang="ts">
	import type { Snippet } from 'svelte';
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';
	import type { IconName } from '../icons/icons';
	import { isLinkHref, type LinkHref } from './link-href';
	import type { ButtonSize, ButtonVariant } from './types';

	/** What every Button takes. */
	interface Common {
		/** The words: a verb and its object. */
		readonly children: Snippet;
		readonly variant?: ButtonVariant;
		readonly size?: ButtonSize;
		readonly icon?: IconName | undefined;
		readonly iconAfter?: IconName | undefined;
		readonly testid?: string | undefined;
	}

	/** A real button: an action, which can be busy or disabled. */
	interface Action extends Common {
		readonly href?: undefined;
		readonly onPress: () => void;
		readonly type?: 'button' | 'submit';
		/** Busy keeps focus and the words, and ignores presses until the action is done. */
		readonly busy?: boolean;
		readonly disabled?: boolean;
	}

	/** A link that looks like a button: it only goes somewhere. */
	interface Link extends Common {
		readonly href: LinkHref;
		readonly onPress?: undefined;
		readonly type?: undefined;
		readonly busy?: undefined;
		readonly disabled?: undefined;
	}

	type Props = Action | Link;

	let {
		children,
		variant = 'secondary',
		size = 'md',
		icon,
		iconAfter,
		testid,
		href,
		onPress,
		type = 'button',
		busy = false,
		disabled = false
	}: Props = $props();

	const { moves } = useEnvironment();

	/** The link's target, checked again here; a link that fails the check goes nowhere. */
	const checkedHref = $derived(href !== undefined && isLinkHref(href) ? href : undefined);
	const iconSize = $derived(size === 'sm' ? 'sm' : 'md');
	const classes = $derived([
		'zf-btn',
		`zf-btn--${variant}`,
		size === 'sm' && 'zf-btn--sm',
		variant === 'quiet' ? 'pick' : 'placed'
	]);

	/** A press: ignored while busy, otherwise handed to the parent. */
	function press(event: MouseEvent): void {
		if (busy) {
			event.preventDefault();
			return;
		}
		onPress?.();
	}
</script>

{#if href !== undefined}
	<a class={classes} href={checkedHref} data-testid={testid} {@attach moves.squish()}>
		{#if icon !== undefined}<Icon name={icon} size={iconSize} />{/if}
		{@render children()}
		{#if iconAfter !== undefined}<Icon name={iconAfter} size={iconSize} />{/if}
	</a>
{:else}
	<button
		class={classes}
		{type}
		{disabled}
		aria-busy={busy ? 'true' : undefined}
		aria-disabled={busy ? 'true' : undefined}
		data-testid={testid}
		onclick={press}
		{@attach moves.squish()}
	>
		{#if icon !== undefined}<Icon name={icon} size={iconSize} />{/if}
		{@render children()}
		{#if iconAfter !== undefined}<Icon name={iconAfter} size={iconSize} />{/if}
	</button>
{/if}

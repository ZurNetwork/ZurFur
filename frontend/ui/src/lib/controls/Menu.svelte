<script lang="ts" generics="Id extends string">
	import { DropdownMenu } from 'bits-ui';
	import type { Attachment } from 'svelte/attachments';
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';
	import { portalTarget } from './floating';
	import Tooltip from './Tooltip.svelte';
	import type { MenuItem, MenuTrigger } from './types';

	interface Props {
		/** What opens the menu: a button with words, or an icon named by its label. */
		readonly trigger: MenuTrigger;
		/** The rows, shown as given. */
		readonly items: readonly MenuItem<Id>[];
		readonly open: boolean;
		/** Handed the state an interaction asks for; the parent decides whether `open` follows. */
		readonly onOpenChange: (open: boolean) => void;
		/** Handed the id of the row chosen. */
		readonly onSelect: (id: Id) => void;
		readonly testid?: string | undefined;
	}

	let { trigger, items, open, onOpenChange, onSelect, testid }: Props = $props();

	const { moves, sounds } = useEnvironment();

	/** The trigger's element, once it is mounted. */
	let anchor: HTMLElement | undefined = $state();
	const portalTo = $derived(anchor === undefined ? undefined : portalTarget(anchor));
	/** Keeps the trigger's element while it is mounted, so the menu knows which skin it opens in. */
	const remember: Attachment<HTMLElement> = (element) => {
		anchor = element;
		return () => {
			anchor = undefined;
		};
	};
	const buttonVariant = $derived(
		trigger.kind === 'button' ? (trigger.variant ?? 'secondary') : 'quiet'
	);

	/** The open menu's own element, while it is mounted. */
	let menu: HTMLElement | undefined;
	/** Keeps the open menu's element, so opening can focus it without scrolling. */
	const keepMenu: Attachment<HTMLElement> = (element) => {
		menu = element;
		return () => {
			menu = undefined;
		};
	};

	/** Puts focus on the menu, unless it holds focus already, without scrolling any box around it. */
	function focusMenu(event: Event): void {
		event.preventDefault();
		if (menu === undefined || menu.contains(menu.ownerDocument.activeElement)) return;
		menu.focus({ preventScroll: true });
	}

	/** Leaves focus in the menu when bits-ui sets it up again while it stays open. */
	function keepFocusWhileOpen(event: Event): void {
		if (open) event.preventDefault();
	}

	/** An open or a close: plays its cue and asks the parent; the menu shows whatever `open` says next. */
	function ask(next: boolean): void {
		sounds.play(next ? 'open' : 'close');
		onOpenChange(next);
	}
</script>

<!-- The icon trigger: one button that is both the tip's trigger and the menu's. -->
{#snippet child({ props }: { readonly props: Record<string, unknown> })}
	<DropdownMenu.Trigger {...props}>
		{#snippet child({ props })}
			<button
				{...props}
				class="zf-iconbtn zf-iconbtn--quiet pick"
				aria-label={trigger.label}
				data-testid={testid}
				{@attach moves.squish()}
				{@attach remember}
				>{#if trigger.kind === 'icon'}<Icon name={trigger.icon} />{/if}</button
			>
		{/snippet}
	</DropdownMenu.Trigger>
{/snippet}

<DropdownMenu.Root bind:open={() => open, ask}>
	{#if trigger.kind === 'button'}
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				<button
					{...props}
					class={[
						'zf-btn',
						`zf-btn--${buttonVariant}`,
						buttonVariant === 'quiet' ? 'pick' : 'placed'
					]}
					data-testid={testid}
					{@attach moves.squish()}
					{@attach remember}
				>
					{#if trigger.icon !== undefined}<Icon name={trigger.icon} />{/if}
					<span>{trigger.label}</span>
					<Icon name="chevron-down" />
				</button>
			{/snippet}
		</DropdownMenu.Trigger>
	{:else}
		<Tooltip words={trigger.label} trigger={child} />
	{/if}
	{#if portalTo !== undefined}
		<DropdownMenu.Portal to={portalTo}>
			<DropdownMenu.Content
				align="start"
				onOpenAutoFocus={focusMenu}
				onCloseAutoFocus={keepFocusWhileOpen}
			>
				{#snippet child({ props, wrapperProps })}
					<div {...wrapperProps}>
						<div {...props} class="zf-menu" {@attach moves.note('in-place')} {@attach keepMenu}>
							{#each items as item (item.id)}
								<DropdownMenu.Item
									disabled={item.disabled ?? false}
									onpointerup={(event) => {
										event.preventDefault();
									}}
									onSelect={() => {
										onSelect(item.id);
									}}
								>
									{#snippet child({ props })}
										<div
											{...props}
											class={[
												'zf-menu__item',
												'pick',
												item.tone === 'danger' && 'zf-menu__item--danger'
											]}
										>
											{#if item.icon !== undefined}<Icon name={item.icon} />{/if}
											<span class="zf-menu__label">{item.label}</span>
										</div>
									{/snippet}
								</DropdownMenu.Item>
							{/each}
						</div>
					</div>
				{/snippet}
			</DropdownMenu.Content>
		</DropdownMenu.Portal>
	{/if}
</DropdownMenu.Root>

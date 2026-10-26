<script lang="ts" generics="Id extends string">
	import { DropdownMenu } from 'bits-ui';
	import type { Attachment } from 'svelte/attachments';
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';
	import { portalTarget } from './floating';
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

	/** An open or a close: plays its cue and asks the parent; the menu shows whatever `open` says next. */
	function ask(next: boolean): void {
		sounds.play(next ? 'open' : 'close');
		onOpenChange(next);
	}
</script>

<DropdownMenu.Root bind:open={() => open, ask}>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			{#if trigger.kind === 'button'}
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
			{:else}
				<button
					{...props}
					class="zf-iconbtn zf-iconbtn--quiet pick"
					aria-label={trigger.label}
					data-testid={testid}
					{@attach moves.squish()}
					{@attach remember}><Icon name={trigger.icon} /></button
				>
			{/if}
		{/snippet}
	</DropdownMenu.Trigger>
	{#if portalTo !== undefined}
		<DropdownMenu.Portal to={portalTo}>
			<DropdownMenu.Content>
				{#snippet child({ props, wrapperProps })}
					<div {...wrapperProps}>
						<div {...props} class="zf-menu" {@attach moves.note('in-place')}>
							{#each items as item (item.id)}
								<DropdownMenu.Item
									disabled={item.disabled ?? false}
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

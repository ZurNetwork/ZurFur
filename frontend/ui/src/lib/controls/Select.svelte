<script lang="ts" generics="V extends string">
	import { Select as Listbox } from 'bits-ui';
	import type { Attachment } from 'svelte/attachments';
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';
	import FieldFrame from './FieldFrame.svelte';
	import { portalTarget } from './floating';
	import type { FieldIds, SelectOption } from './types';

	interface Props {
		readonly label: string;
		readonly options: readonly SelectOption<V>[];
		/** The chosen option's value, or `undefined` while nothing is chosen. */
		readonly value: V | undefined;
		/** Handed the option's value; the parent decides whether `value` follows. */
		readonly onChange: (value: V) => void;
		/** Shown in muted ink while nothing is chosen. */
		readonly placeholder?: string | undefined;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		readonly disabled?: boolean;
		/** Marks the field required for screen readers; the browser does not block a form on it. */
		readonly required?: boolean;
		/** With a name and a chosen value, a form posts the choice; otherwise it posts nothing. */
		readonly name?: string | undefined;
		readonly testid?: string | undefined;
	}

	let {
		label,
		options,
		value,
		onChange,
		placeholder,
		hint,
		error,
		disabled = false,
		required = false,
		name,
		testid
	}: Props = $props();

	const { moves, sounds } = useEnvironment();
	const id = $props.id();
	const listId = `${id}-list`;

	/** The option `value` names, if any. */
	const chosen = $derived(options.find((option) => option.value === value));

	/** The options as bits-ui reads them, so typing a letter finds one while the list is closed. */
	const items = $derived(options.map((option) => ({ value: option.value, label: option.label })));

	/** The name the hidden input posts under: none until a value is chosen, as a native select's placeholder. */
	const postedName = $derived(value === undefined ? '' : (name ?? ''));

	const classes = $derived([
		'zf-select',
		'slip',
		!disabled && 'lift',
		error !== undefined && 'zf-select--invalid',
		disabled && 'zf-select--disabled'
	]);

	/** The trigger's element, once it is mounted. */
	let anchor: HTMLElement | undefined = $state();
	const portalTo = $derived(anchor === undefined ? undefined : portalTarget(anchor));
	/** Keeps the trigger's element while it is mounted, so the list knows which skin it opens in. */
	const remember: Attachment<HTMLElement> = (element) => {
		anchor = element;
		return () => {
			anchor = undefined;
		};
	};

	/** The open list's own element, while it is mounted. */
	let list: HTMLElement | undefined;
	/** Keeps the open list's element, so a highlighted row can be scrolled into it. */
	const keepList: Attachment<HTMLElement> = (element) => {
		list = element;
		return () => {
			list = undefined;
		};
	};

	/** Scrolls the list, and only the list, so the row at `index` shows whole. */
	function reveal(index: number): void {
		const row = list?.children.item(index);
		if (list === undefined || !(row instanceof HTMLElement)) return;
		const rowBox = row.getBoundingClientRect();
		const listBox = list.getBoundingClientRect();
		if (rowBox.top < listBox.top) list.scrollTop -= listBox.top - rowBox.top;
		else if (rowBox.bottom > listBox.bottom) list.scrollTop += rowBox.bottom - listBox.bottom;
	}

	/** A string bits-ui hands back: looked up in the options; unknown, empty or already shown, it asks nothing. */
	function choose(next: string): void {
		const asked = options.find((option) => option.value === next);
		if (next === '' || asked === undefined || asked.value === value) return;
		onChange(asked.value);
	}

	/** The option a press in the open list began on; forgotten at every open and close and after every release. */
	let pressed: V | undefined;

	function forget(): void {
		pressed = undefined;
	}

	/** A release on `option`: it chooses only when the press began on that same option. */
	function release(event: PointerEvent, option: V): void {
		const same = pressed === option;
		forget();
		if (!same) event.preventDefault();
	}

	/** Whether the list was last shown open, so each cue plays once per change. */
	let shownOpen = false;

	/** An open or a close: forgets any press and plays its cue. */
	function openChanged(next: boolean): void {
		forget();
		if (next === shownOpen) return;
		shownOpen = next;
		sounds.play(next ? 'open' : 'close');
	}
</script>

<FieldFrame {label} {hint} {error}>
	{#snippet children(ids: FieldIds)}
		<Listbox.Root
			type="single"
			bind:value={() => value ?? '', choose}
			onOpenChange={openChanged}
			{items}
			{disabled}
			name={postedName}
			autocomplete="off"
		>
			<Listbox.Trigger>
				{#snippet child({ props }: { readonly props: Record<string, unknown> })}
					<button
						{...props}
						class={classes}
						role="combobox"
						aria-controls={props['aria-expanded'] === 'true' ? listId : undefined}
						aria-labelledby={ids.labelId}
						aria-describedby={ids.describedBy}
						aria-invalid={ids.invalid}
						aria-required={required ? 'true' : undefined}
						data-testid={testid}
						{@attach remember}
					>
						<span class={['zf-select__value', chosen === undefined && 'zf-select__value--empty']}
							>{chosen?.label ?? placeholder ?? ''}</span
						>
						<span class="zf-select__chevron"><Icon name="chevron-down" /></span>
					</button>
				{/snippet}
			</Listbox.Trigger>
			{#if portalTo !== undefined}
				<Listbox.Portal to={portalTo}>
					<Listbox.Content id={listId} align="start">
						{#snippet child({ props, wrapperProps })}
							<div {...wrapperProps}>
								<div
									{...props}
									class="zf-menu zf-select__list"
									aria-labelledby={ids.labelId}
									onpointerdowncapture={forget}
									{@attach moves.note('in-place')}
									{@attach keepList}
								>
									{#each options as option, index (option.value)}
										<Listbox.Item
											value={option.value}
											label={option.label}
											onpointerdown={() => {
												pressed = option.value;
											}}
											onpointerup={(event) => {
												release(event, option.value);
											}}
											onpointercancel={forget}
											onHighlight={() => {
												reveal(index);
											}}
										>
											{#snippet child({ props, selected })}
												<div {...props} class="zf-menu__item zf-select__option pick">
													<span class="zf-menu__label">{option.label}</span>
													{#if selected}<Icon name="check" />{/if}
												</div>
											{/snippet}
										</Listbox.Item>
									{/each}
								</div>
							</div>
						{/snippet}
					</Listbox.Content>
				</Listbox.Portal>
			{/if}
		</Listbox.Root>
	{/snippet}
</FieldFrame>

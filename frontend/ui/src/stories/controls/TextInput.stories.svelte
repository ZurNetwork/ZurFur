<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, userEvent, waitFor } from 'storybook/test';
	import { TextInput } from '@zurfur/ui';
	import { ARTIST_DID, ARTIST_HANDLE, TEXT_WORDS } from '../fixtures/fields.ts';
	import type { TextInputArgs } from './args.ts';
	import TextInputHolder from './TextInputHolder.svelte';

	const { Story } = defineMeta({
		title: 'Controls/Text Input',
		component: TextInput,
		render: template,
		tags: ['autodocs'],
		args: {
			label: TEXT_WORDS.title,
			value: '',
			onChange: fn()
		},
		parameters: {
			docs: {
				description: {
					component:
						'A simple component: one labelled field for a line of text, or a few lines. It keeps no copy of the text, so what shows is always what its parent hands it. A hint helps, an error says what went wrong and how to go on.\n\n**Keyboard**\n\n- Tab → reaches the field\n- Type → edits the text\n- Esc → leaves the text as it is\n- Tab again → moves on'
				}
			}
		}
	});
</script>

{#snippet template(args: TextInputArgs)}
	<TextInputHolder
		label={args.label}
		labelHidden={args.labelHidden ?? false}
		value={args.value}
		onChange={args.onChange}
		hint={args.hint}
		error={args.error}
		icon={args.icon}
		prefix={args.prefix}
		mono={args.mono ?? false}
		multiline={args.multiline ?? false}
		type={args.type ?? 'text'}
		required={args.required ?? false}
		disabled={args.disabled ?? false}
		readonly={args.readonly ?? false}
		placeholder={args.placeholder}
	/>
{/snippet}

<Story name="Playground" args={{ placeholder: TEXT_WORDS.titleExample }} />

<Story name="With a Value" args={{ value: TEXT_WORDS.titleExample }} />

<Story name="With a Hint" args={{ hint: TEXT_WORDS.hint }} />

<Story
	name="With an Error"
	args={{ label: TEXT_WORDS.handle, value: 'artistt.test', error: TEXT_WORDS.error }}
/>

<Story name="With an Icon" args={{ label: TEXT_WORDS.search, icon: 'search', type: 'search' }} />

<Story
	name="With a Prefix"
	args={{ label: TEXT_WORDS.handle, prefix: '@', mono: true, value: ARTIST_HANDLE }}
/>

<Story name="Mono" args={{ label: TEXT_WORDS.did, mono: true, value: ARTIST_DID }} />

<Story
	name="Multiline"
	args={{ label: TEXT_WORDS.notes, multiline: true, value: TEXT_WORDS.notesExample }}
/>

<Story name="Hidden Label" args={{ label: TEXT_WORDS.search, labelHidden: true, icon: 'search' }} />

<Story name="Disabled" args={{ value: TEXT_WORDS.titleExample, disabled: true }} />

<Story name="Read Only" args={{ value: TEXT_WORDS.titleExample, readonly: true }} />

<Story
	name="Typing"
	play={async ({ canvas, args }) => {
		const field = canvas.getByRole('textbox', { name: TEXT_WORDS.title });
		await userEvent.type(field, 'Sketch');
		await expect(args.onChange).toHaveBeenCalledTimes(6);
		await expect(args.onChange).toHaveBeenLastCalledWith('Sketch');
		await expect(field).toHaveValue('Sketch');
	}}
/>

<Story
	name="Keyboard"
	play={async ({ canvas, args }) => {
		await userEvent.tab();
		const field = canvas.getByRole('textbox', { name: TEXT_WORDS.title });
		await expect(field).toHaveFocus();
		await userEvent.keyboard('Lineart');
		await expect(args.onChange).toHaveBeenLastCalledWith('Lineart');
		await userEvent.keyboard('{Backspace}');
		await expect(args.onChange).toHaveBeenLastCalledWith('Linear');
		await waitFor(() => expect(field).toHaveValue('Linear'));
		await userEvent.tab();
		await expect(field).not.toHaveFocus();
	}}
/>

<Story
	name="Focused"
	args={{ value: TEXT_WORDS.titleExample }}
	play={async ({ canvas }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('textbox', { name: TEXT_WORDS.title })).toHaveFocus();
	}}
/>

<Story
	name="Disabled Does Nothing"
	args={{ disabled: true }}
	play={async ({ canvas, args }) => {
		const field = canvas.getByRole('textbox', { name: TEXT_WORDS.title });
		await expect(field).toBeDisabled();
		await userEvent.type(field, 'Sketch');
		await expect(args.onChange).not.toHaveBeenCalled();
		await expect(field).toHaveValue('');
	}}
/>

<Story
	name="Read Only Does Nothing"
	args={{ readonly: true, value: TEXT_WORDS.titleExample }}
	play={async ({ canvas, args }) => {
		const field = canvas.getByRole('textbox', { name: TEXT_WORDS.title });
		await userEvent.type(field, 'Sketch');
		await expect(args.onChange).not.toHaveBeenCalled();
		await expect(field).toHaveValue(TEXT_WORDS.titleExample);
	}}
/>

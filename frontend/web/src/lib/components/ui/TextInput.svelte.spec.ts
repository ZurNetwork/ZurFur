import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import TextInput from './TextInput.svelte';

describe('TextInput', () => {
	it('is a textarea of at least six rows when multi-line, with no length cap, at 16px or more', async () => {
		render(TextInput, { label: 'What happened?', multiline: true });

		const field = page.getByRole('textbox', { name: 'What happened?' });
		await expect.element(field).toBeInTheDocument();
		const element = field.element();
		expect(element).toBeInstanceOf(HTMLTextAreaElement);
		if (!(element instanceof HTMLTextAreaElement)) return;
		expect(element.rows).toBeGreaterThanOrEqual(6);
		expect(element.hasAttribute('maxlength')).toBe(false);
		expect(element.maxLength).toBe(-1);
		expect(parseFloat(getComputedStyle(element).fontSize)).toBeGreaterThanOrEqual(16);
	});

	it('is a one-line text box otherwise, also at 16px or more', async () => {
		render(TextInput, { label: 'Page' });

		const field = page.getByRole('textbox', { name: 'Page' });
		await expect.element(field).toBeInTheDocument();
		const element = field.element();
		expect(element).toBeInstanceOf(HTMLInputElement);
		expect(element.hasAttribute('maxlength')).toBe(false);
		expect(parseFloat(getComputedStyle(element).fontSize)).toBeGreaterThanOrEqual(16);
	});

	it('takes long text whole', async () => {
		render(TextInput, { label: 'What happened?', multiline: true });

		const field = page.getByRole('textbox', { name: 'What happened?' });
		const long = 'a'.repeat(5000);
		await field.fill(long);
		await expect.element(field).toHaveValue(long);
	});

	it('is valid and undescribed without an error', async () => {
		render(TextInput, { label: 'What happened?', multiline: true });

		const field = page.getByRole('textbox', { name: 'What happened?' });
		await expect.element(field).not.toHaveAttribute('aria-invalid');
		await expect.element(field).not.toHaveAttribute('aria-describedby');
	});

	it('marks an error invalid and describes the field with the message', async () => {
		render(TextInput, {
			label: 'What happened?',
			multiline: true,
			error: 'Say what went wrong.'
		});

		const field = page.getByRole('textbox', { name: 'What happened?' });
		await expect.element(field).toHaveAttribute('aria-invalid', 'true');
		const message = page.getByText('Say what went wrong.');
		const messageId = message.element().id;
		expect(messageId).not.toBe('');
		await expect.element(field).toHaveAttribute('aria-describedby', messageId);
		await expect.element(field).toHaveAccessibleDescription('Say what went wrong.');
	});

	it('reads the hint and the error together', async () => {
		render(TextInput, {
			label: 'What happened?',
			multiline: true,
			hint: 'Steps help.',
			error: 'Say what went wrong.'
		});

		const field = page.getByRole('textbox', { name: 'What happened?' });
		await expect.element(field).toHaveAccessibleDescription('Steps help. Say what went wrong.');
	});

	it('reads the elements named in describedBy after the hint and the error', async () => {
		const count = document.createElement('span');
		count.id = 'outside-count';
		count.textContent = '3,600 / 4,000';
		document.body.append(count);
		render(TextInput, {
			label: 'What happened?',
			multiline: true,
			hint: 'Steps help.',
			error: 'Say what went wrong.',
			describedBy: 'outside-count'
		});

		const field = page.getByRole('textbox', { name: 'What happened?' });
		await expect
			.element(field)
			.toHaveAccessibleDescription('Steps help. Say what went wrong. 3,600 / 4,000');
		count.remove();
	});

	it('uses the id it is given, so a dialog can focus it', async () => {
		render(TextInput, { label: 'What happened?', multiline: true, id: 'bug-description' });

		await expect
			.element(page.getByRole('textbox', { name: 'What happened?' }))
			.toHaveAttribute('id', 'bug-description');
	});
});

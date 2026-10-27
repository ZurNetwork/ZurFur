import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import Spinner from './Spinner.svelte';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

describe('Spinner', () => {
	it('is a status named by its words', async () => {
		render(Spinner, { kind: 'status', label: 'Loading sketches' });
		const status = page.getByRole('status');
		await expect.element(status).toBeInTheDocument();
		await expect.element(status).toHaveTextContent('Loading sketches');
	});

	it('hides its ring from screen readers', () => {
		const { container } = render(Spinner, { kind: 'status', label: 'Loading sketches' });
		expect(container.querySelector('.zf-spinner')?.getAttribute('aria-hidden')).toBe('true');
	});

	it('keeps its words for screen readers only by default', () => {
		const { container } = render(Spinner, { kind: 'status', label: 'Loading sketches' });
		const label = container.querySelector('.zf-spinner-wrap__label');
		expect(label?.classList.contains('zf-sr')).toBe(true);
	});

	it('shows its words beside the ring when asked', () => {
		const { container } = render(Spinner, {
			kind: 'status',
			label: 'Loading sketches',
			labelShown: true
		});
		const label = container.querySelector('.zf-spinner-wrap__label');
		expect(label?.classList.contains('zf-sr')).toBe(false);
		expect(label?.textContent).toBe('Loading sketches');
	});

	it.each([false, true])(
		'shows its words written as an image tag as text, with no image (shown %j)',
		(labelShown) => {
			const markup = '<img src=x onerror=alert(1)>';
			const { container } = render(Spinner, { kind: 'status', label: markup, labelShown });
			expect(container.querySelector('img')).toBeNull();
			expect(container.querySelector('.zf-spinner-wrap__label')?.textContent).toBe(markup);
			expect(page.getByRole('status').element().textContent.trim()).toBe(markup);
		}
	);

	it('as a glyph is the ring alone: hidden, with no status role and no words', () => {
		const { container } = render(Spinner, { kind: 'glyph', testid: 'ring' });
		const ring = byTestId('ring');
		expect(ring.classList.contains('zf-spinner')).toBe(true);
		expect(ring.getAttribute('aria-hidden')).toBe('true');
		expect(page.getByRole('status').elements()).toEqual([]);
		expect(container.textContent.trim()).toBe('');
	});

	it('is the glyph for a kind it does not know', () => {
		render(Spinner, { kind: 'banner' as 'glyph', testid: 'ring' });
		expect(byTestId('ring').getAttribute('aria-hidden')).toBe('true');
		expect(page.getByRole('status').elements()).toEqual([]);
	});

	it('records the spin loop on the ring of the status form', () => {
		const calls: SeamCall[] = [];
		const { container } = render(
			Spinner,
			{ kind: 'status', label: 'Loading sketches' },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		const ring = container.querySelector('.zf-spinner');
		expect(ring).not.toBeNull();
		expect(calls).toEqual([{ kind: 'move', move: 'loop', args: ['spin'], element: ring }]);
	});

	it('records the spin loop on the glyph', () => {
		const calls: SeamCall[] = [];
		render(
			Spinner,
			{ kind: 'glyph', testid: 'ring' },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		expect(calls).toEqual([
			{ kind: 'move', move: 'loop', args: ['spin'], element: byTestId('ring') }
		]);
	});

	it('records nothing else: no sound, no other move', () => {
		const calls: SeamCall[] = [];
		render(
			Spinner,
			{ kind: 'status', label: 'Loading sketches' },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		expect(calls).toHaveLength(1);
	});

	it('draws without an environment, and stays still', () => {
		const { container } = render(Spinner, { kind: 'status', label: 'Loading sketches' });
		expect(container.querySelector('.zf-spinner')).not.toBeNull();
	});
});

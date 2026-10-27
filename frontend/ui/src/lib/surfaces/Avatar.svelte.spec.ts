import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Avatar from './Avatar.svelte';
import { isImageSrc, type ImageSrc } from './image-src.ts';
import type { AvatarSize, AvatarTone } from './types.ts';
import '../styles/index.ts';

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

/** The fixture picture Storybook and the specs serve, minted the one way. */
function fixturePicture(): ImageSrc {
	const address = '/fixtures/avatar.svg';
	if (!isImageSrc(address)) throw new Error('the fixture address stopped passing');
	return address;
}

/** The circle inside the element carrying `testid`, or that element when it is the circle. */
function circleOf(testid: string): HTMLElement {
	const found = byTestId(testid);
	if (found.classList.contains('zf-avatar')) return found;
	const circle = found.querySelector('.zf-avatar');
	if (!(circle instanceof HTMLElement)) throw new Error('no circle');
	return circle;
}

/** The text the circle shows, when the picture isn't there. */
function initialIn(testid: string): string {
	return circleOf(testid).querySelector('bdi')?.textContent ?? '';
}

const FSI = '\u2068';
const PDI = '\u2069';

/** Addresses no picture may load from, cast in as a lying brand would be. */
const hostile = [
	'https://x.example/a.png',
	'//x.example/a.png',
	'javascript:alert(1)',
	'data:image/svg+xml,<svg onload=alert(1)>',
	'/a/%2e%2e//evil.example/x',
	'/'
];

describe('Avatar', () => {
	it('beside its text, hides the picture with an empty alt and shows the name and handle', async () => {
		const shown = {
			name: 'Mika',
			handle: 'mika.test',
			picture: fixturePicture(),
			size: 'md',
			tone: 'neutral',
			textShown: true,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		const wrap = byTestId('avatar');
		const picture = wrap.querySelector('img');
		expect(picture?.getAttribute('alt')).toBe('');
		expect(picture?.getAttribute('src')).toBe('/fixtures/avatar.svg');
		expect(picture?.getAttribute('loading')).toBe('lazy');
		expect(picture?.getAttribute('decoding')).toBe('async');
		expect(circleOf('avatar').getAttribute('aria-hidden')).toBe('true');
		const words = [...wrap.querySelectorAll('.zf-avatar__text bdi')].map((bdi) => bdi.textContent);
		expect(words).toEqual(['Mika', '@mika.test']);
		expect(wrap.querySelector('.zf-handle')?.textContent).toBe('@mika.test');
		await expect.element(page.getByText('@mika.test')).toBeInTheDocument();
	});

	it('beside its text, shows no handle line when given none', () => {
		const shown = {
			name: 'Mika',
			size: 'md',
			tone: 'neutral',
			textShown: true,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		expect(byTestId('avatar').querySelector('.zf-handle')).toBeNull();
	});

	it('alone, is an image named by the name, isolated', async () => {
		const shown = {
			name: 'مها',
			picture: fixturePicture(),
			size: 'md',
			tone: 'neutral',
			textShown: false
		} as const;
		render(Avatar, shown);
		const circle = page.getByRole('img', { name: 'مها' });
		await expect.element(circle).toBeInTheDocument();
		await expect.element(circle).toHaveAttribute('aria-label', `${FSI}مها${PDI}`);
	});

	it('alone, drops bidi controls from the name so they cannot break the isolate', () => {
		const shown = {
			name: `Mi${PDI}\u202Eka`,
			size: 'md',
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		expect(circleOf('avatar').getAttribute('aria-label')).toBe(`${FSI}Mika${PDI}`);
	});

	it.each(hostile)('draws no picture for the lying brand %j', (address) => {
		const shown = {
			name: 'Mika',
			picture: address as ImageSrc,
			size: 'md',
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		expect(byTestId('avatar').querySelector('img')).toBeNull();
		expect(initialIn('avatar')).toBe('M');
	});

	it('draws no picture for a value that only says it is an address', () => {
		const pretender = { toString: () => '/fixtures/avatar.svg' };
		const shown = {
			name: 'Mika',
			picture: pretender as unknown as ImageSrc,
			size: 'md',
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		expect(byTestId('avatar').querySelector('img')).toBeNull();
	});

	it('shows the initial when the picture fails to load', async () => {
		const shown = {
			name: 'Mika',
			picture: fixturePicture(),
			size: 'md',
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		const picture = byTestId('avatar').querySelector('img');
		expect(picture).not.toBeNull();
		picture?.dispatchEvent(new Event('error'));
		await expect.poll(() => byTestId('avatar').querySelector('img')).toBeNull();
		expect(initialIn('avatar')).toBe('M');
	});

	it.each([
		['leading punctuation', '...¡Mika!', 'M'],
		['leading combining marks', '\u0301\u0302mika', 'm'],
		[
			'a whole emoji sequence',
			'\u{1f9d1}\u{1f3fd}\u200D\u{1f3a8} Mika',
			'\u{1f9d1}\u{1f3fd}\u200D\u{1f3a8}'
		],
		['a number', '7 Foxes', '7'],
		['a letter in another script', '«ミカ»', 'ミ']
	])('shows the first grapheme past %s', (_case, name, expected) => {
		const shown = {
			name,
			size: 'md',
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		expect(initialIn('avatar')).toBe(expected);
	});

	it('keeps a letter with stacked marks whole, clipped inside its circle', () => {
		const stacked = 'Z\u0351\u0352\u0353\u0354\u0355\u0356\u0357\u0358\u0359\u035A\u035B';
		const shown = {
			name: `${stacked}oe`,
			size: 'md',
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		const circle = circleOf('avatar');
		expect(initialIn('avatar')).toBe(stacked);
		expect(getComputedStyle(circle).overflow).toBe('hidden');
		expect(circle.querySelector('bdi')).not.toBeNull();
	});

	it.each(['', '...', '!?', ' '])('draws a blank, hidden circle for the name %j', (name) => {
		const shown = {
			name,
			size: 'md',
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		const circle = circleOf('avatar');
		expect(circle.getAttribute('aria-hidden')).toBe('true');
		expect(circle.getAttribute('role')).toBeNull();
		expect(circle.textContent.trim()).toBe('');
		expect(page.getByRole('img').elements()).toEqual([]);
	});

	it('shows a name written as markup as text, inside bdi, with no picture', async () => {
		const markup = '<img src=x onerror=alert(1)>';
		const shown = {
			name: markup,
			size: 'md',
			tone: 'neutral',
			textShown: true,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		const wrap = byTestId('avatar');
		expect(wrap.querySelector('img')).toBeNull();
		expect(wrap.querySelector('.zf-avatar__name')?.tagName).toBe('BDI');
		expect(wrap.querySelector('.zf-avatar__name')?.textContent).toBe(markup);
		await expect.element(page.getByText(markup)).toBeInTheDocument();
	});

	it.each(['sm', 'md', 'lg'] as const)('wears the %s size', (size) => {
		const shown = {
			name: 'Mika',
			size,
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		expect(circleOf('avatar').classList.contains(`zf-avatar--${size}`)).toBe(true);
	});

	it.each(['xl', 'MD', '', 'toString'])('wears md for the size %j', (stranger) => {
		const shown = {
			name: 'Mika',
			size: stranger as AvatarSize,
			tone: 'neutral',
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		const sizes = [...circleOf('avatar').classList].filter((name) =>
			['zf-avatar--sm', 'zf-avatar--md', 'zf-avatar--lg'].includes(name)
		);
		expect(sizes).toEqual(['zf-avatar--md']);
	});

	it.each(['neutral', 'accent', 'iris', 'crown', 'rose', 'taupe'] as const)(
		'wears the %s tone',
		(tone) => {
			const shown = { name: 'Mika', size: 'md', tone, textShown: false, testid: 'avatar' } as const;
			render(Avatar, shown);
			expect(circleOf('avatar').classList.contains(`zf-avatar--${tone}`)).toBe(true);
		}
	);

	it.each(['danger', 'Accent', '', 'toString'])('wears neutral for the tone %j', (stranger) => {
		const shown = {
			name: 'Mika',
			size: 'md',
			tone: stranger as AvatarTone,
			textShown: false,
			testid: 'avatar'
		} as const;
		render(Avatar, shown);
		const classes = [...circleOf('avatar').classList];
		expect(classes).toContain('zf-avatar--neutral');
		expect(classes).not.toContain(`zf-avatar--${stranger}`);
	});

	it('shows no word of its own', () => {
		const shown = { name: 'Mika', size: 'md', tone: 'neutral', textShown: false } as const;
		const { container } = render(Avatar, shown);
		expect(container.textContent.trim()).toBe('M');
	});
});

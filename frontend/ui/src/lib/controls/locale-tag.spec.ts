import { describe, expect, it } from 'vitest';
import { isLocaleTag } from './locale-tag.ts';

const tags = ['en-GB', 'en', 'de-DE', 'pt-BR', 'zh-Hant-TW', 'ja-JP', 'sr-Latn', 'en-gb'];

const refused = [
	'en_GB!',
	'en_GB',
	'',
	' ',
	'en-GB ',
	'en--GB',
	'__proto__',
	'x',
	'en-GB-',
	'<img src=x onerror=alert(1)>',
	'a'.repeat(10_000)
];

describe('isLocaleTag', () => {
	it.each(tags)('accepts %j', (text) => {
		expect(isLocaleTag(text)).toBe(true);
	});

	it.each(refused.map((text) => [text.slice(0, 40), text]))('refuses %j', (_, text) => {
		expect(isLocaleTag(text)).toBe(false);
	});

	it('never throws, whatever the text', () => {
		const junk = [
			...refused,
			'\u0000',
			'\uD800',
			'en-GB,de-DE',
			'en-u-ca-',
			`en-${'x'.repeat(100_000)}`,
			Array.from({ length: 2000 }, () => 'en').join('-')
		];
		expect(() => junk.map((text) => isLocaleTag(text))).not.toThrow();
	});
});

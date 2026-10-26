import { describe, expect, it } from 'vitest';
import { splitUsage, type UsageSegment } from './usage-text';

const text = (value: string): UsageSegment => ({ kind: 'text', text: value });
const code = (value: string): UsageSegment => ({ kind: 'code', text: value });

describe('splitUsage', () => {
	it('keeps plain text as one segment', () => {
		expect(splitUsage('The open item name, once per view.')).toEqual([
			text('The open item name, once per view.')
		]);
	});

	it('splits out code spans in order', () => {
		expect(splitUsage('Use `space-2` between a label and its `input`.')).toEqual([
			text('Use '),
			code('space-2'),
			text(' between a label and its '),
			code('input'),
			text('.')
		]);
	});

	it('handles a line that starts and ends with code', () => {
		expect(splitUsage('`one` and `two`')).toEqual([code('one'), text(' and '), code('two')]);
		expect(splitUsage('`only`')).toEqual([code('only')]);
	});

	it('leaves the rest as text after an unmatched backtick', () => {
		expect(splitUsage('see `a` then `b and more')).toEqual([
			text('see '),
			code('a'),
			text(' then `b and more')
		]);
		expect(splitUsage('`never closed')).toEqual([text('`never closed')]);
	});

	it('keeps an empty pair of backticks as text', () => {
		expect(splitUsage('a``b')).toEqual([text('a``b')]);
	});

	it('gives nothing for an empty line', () => {
		expect(splitUsage('')).toEqual([]);
	});

	it('never turns markup into anything but text', () => {
		const line = '<img src=x onerror=alert(1)> and `<b>bold</b>`';
		expect(splitUsage(line)).toEqual([
			text('<img src=x onerror=alert(1)> and '),
			code('<b>bold</b>')
		]);
	});
});

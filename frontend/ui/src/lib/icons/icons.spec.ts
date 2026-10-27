import { describe, expect, it } from 'vitest';
import {
	ICON_NAMES,
	ICONS,
	STROKE_HEAVY,
	STROKE_REGULAR,
	isIconName,
	type IconShape
} from './icons.ts';

const bundled = [
	'archive',
	'at-sign',
	'check',
	'chevron-down',
	'chevron-right',
	'circle-alert',
	'circle-check',
	'circle-dashed',
	'circle-dot',
	'circle-ellipsis',
	'circle-pause',
	'circle',
	'clock',
	'copy',
	'ellipsis',
	'eye-off',
	'eye',
	'file-symlink',
	'file',
	'folder-open',
	'folder-symlink',
	'folder',
	'globe',
	'hourglass',
	'image',
	'import',
	'inbox',
	'info',
	'list',
	'lock',
	'menu',
	'message-circle',
	'minus',
	'package-check',
	'plus',
	'search',
	'send',
	'trash-2',
	'x'
];
const extras = ['link', 'box', 'chevron-left', 'corner-down-right', 'bug', 'calendar', 'flag'];

/** Path data is path commands, numbers and separators, and starts by moving. */
const pathData =
	/^[Mm][\s,]*(?:[MmLlHhVvCcSsQqTtAaZz]|[-+]?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?|[\s,])*$/;

function shapesOf(name: (typeof ICON_NAMES)[number]): readonly IconShape[] {
	return ICONS[name];
}

describe('the icon registry', () => {
	it('holds exactly the 39 bundled names and the 7 extras', () => {
		expect(bundled).toHaveLength(39);
		expect([...ICON_NAMES].sort()).toEqual([...bundled, ...extras].sort());
		expect(ICON_NAMES).toHaveLength(46);
	});

	it('draws every icon with at least one shape', () => {
		for (const name of ICON_NAMES) {
			expect(shapesOf(name).length, name).toBeGreaterThan(0);
		}
	});

	it('writes every path as path data', () => {
		const paths = ICON_NAMES.flatMap((name) =>
			shapesOf(name).flatMap((shape) => (shape.shape === 'path' ? [{ name, d: shape.d }] : []))
		);
		expect(paths.length).toBeGreaterThan(46);
		for (const { name, d } of paths) {
			expect(pathData.test(d), `${name}: ${d}`).toBe(true);
		}
	});

	it('is frozen down to each shape', () => {
		expect(Object.isFrozen(ICONS)).toBe(true);
		for (const name of ICON_NAMES) {
			expect(Object.isFrozen(ICONS[name]), name).toBe(true);
			for (const shape of ICONS[name]) expect(Object.isFrozen(shape), name).toBe(true);
		}
	});

	it('names the two stroke widths', () => {
		expect(STROKE_REGULAR).toBe(1.75);
		expect(STROKE_HEAVY).toBe(2.5);
	});
});

describe('isIconName', () => {
	it('accepts every name in the registry', () => {
		for (const name of ICON_NAMES) expect(isIconName(name), name).toBe(true);
	});

	it.each(['', 'Folder', 'folder ', '__proto__', 'constructor', 'toString', 'hasOwnProperty'])(
		'refuses %j',
		(text) => {
			expect(isIconName(text)).toBe(false);
		}
	);
});

describe('the path data check', () => {
	it.each(['', 'L1 2', 'M1 2 foo', 'M1 2 <script>', 'M1 2 X3'])('refuses %j', (d) => {
		expect(pathData.test(d)).toBe(false);
	});

	it('accepts a real path', () => {
		expect(pathData.test('M10.1 2.182a10 10 0 0 1 3.8 0Z')).toBe(true);
	});
});

/**
 * The icon set: 46 Lucide icons (lucide-static 1.53.0, published 2026-10-08,
 * ISC License, Copyright (c) 2026 Lucide Icons and Contributors), copied as
 * shape data so they ship inline and self-hosted. Each is drawn on a 24-unit
 * grid with round caps and joins; the stroke width comes from the Icon.
 */

/** The stroke width of every icon at rest. */
export const STROKE_REGULAR = 1.75;

/** The stroke width of the checkmark inside a checked control. */
export const STROKE_HEAVY = 2.5;

/** One SVG shape of an icon. */
export type IconShape =
	| { readonly shape: 'path'; readonly d: string }
	| { readonly shape: 'circle'; readonly cx: number; readonly cy: number; readonly r: number }
	| {
			readonly shape: 'line';
			readonly x1: number;
			readonly y1: number;
			readonly x2: number;
			readonly y2: number;
	  }
	| { readonly shape: 'polyline'; readonly points: string }
	| {
			readonly shape: 'rect';
			readonly x: number;
			readonly y: number;
			readonly width: number;
			readonly height: number;
			readonly rx: number;
			readonly ry: number;
	  };

/** Freezes an icon's shapes, and each shape, so the registry cannot be changed at run time. */
function shapes(drawn: readonly IconShape[]): readonly IconShape[] {
	return Object.freeze(drawn.map((shape) => Object.freeze({ ...shape })));
}

/** Every icon by name. */
export const ICONS = Object.freeze({
	archive: shapes([
		{ shape: 'rect', x: 2, y: 3, width: 20, height: 5, rx: 1, ry: 1 },
		{ shape: 'path', d: 'M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8' },
		{ shape: 'path', d: 'M10 12h4' }
	]),
	'at-sign': shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 4 },
		{ shape: 'path', d: 'M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8' }
	]),
	check: shapes([{ shape: 'path', d: 'M20 6 9 17l-5-5' }]),
	'chevron-down': shapes([{ shape: 'path', d: 'm6 9 6 6 6-6' }]),
	'chevron-right': shapes([{ shape: 'path', d: 'm9 18 6-6-6-6' }]),
	'circle-alert': shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'line', x1: 12, y1: 8, x2: 12, y2: 12 },
		{ shape: 'line', x1: 12, y1: 16, x2: 12.01, y2: 16 }
	]),
	'circle-check': shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'path', d: 'm16 9-5.5 5.5L8 12' }
	]),
	'circle-dashed': shapes([
		{ shape: 'path', d: 'M10.1 2.182a10 10 0 0 1 3.8 0' },
		{ shape: 'path', d: 'M13.9 21.818a10 10 0 0 1-3.8 0' },
		{ shape: 'path', d: 'M17.609 3.721a10 10 0 0 1 2.69 2.7' },
		{ shape: 'path', d: 'M2.182 13.9a10 10 0 0 1 0-3.8' },
		{ shape: 'path', d: 'M20.279 17.609a10 10 0 0 1-2.7 2.69' },
		{ shape: 'path', d: 'M21.818 10.1a10 10 0 0 1 0 3.8' },
		{ shape: 'path', d: 'M3.721 6.391a10 10 0 0 1 2.7-2.69' },
		{ shape: 'path', d: 'M6.391 20.279a10 10 0 0 1-2.69-2.7' }
	]),
	'circle-dot': shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 1 },
		{ shape: 'circle', cx: 12, cy: 12, r: 10 }
	]),
	'circle-ellipsis': shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'path', d: 'M17 12h.01' },
		{ shape: 'path', d: 'M12 12h.01' },
		{ shape: 'path', d: 'M7 12h.01' }
	]),
	'circle-pause': shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'line', x1: 10, y1: 15, x2: 10, y2: 9 },
		{ shape: 'line', x1: 14, y1: 15, x2: 14, y2: 9 }
	]),
	circle: shapes([{ shape: 'circle', cx: 12, cy: 12, r: 10 }]),
	clock: shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'path', d: 'M12 6v6l4 2' }
	]),
	copy: shapes([
		{ shape: 'rect', x: 8, y: 8, width: 14, height: 14, rx: 2, ry: 2 },
		{ shape: 'path', d: 'M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2' }
	]),
	ellipsis: shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 1 },
		{ shape: 'circle', cx: 19, cy: 12, r: 1 },
		{ shape: 'circle', cx: 5, cy: 12, r: 1 }
	]),
	'eye-off': shapes([
		{
			shape: 'path',
			d: 'M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49'
		},
		{ shape: 'path', d: 'M14.084 14.158a3 3 0 0 1-4.242-4.242' },
		{
			shape: 'path',
			d: 'M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143'
		},
		{ shape: 'path', d: 'm2 2 20 20' }
	]),
	eye: shapes([
		{
			shape: 'path',
			d: 'M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0'
		},
		{ shape: 'circle', cx: 12, cy: 12, r: 3 }
	]),
	'file-symlink': shapes([
		{
			shape: 'path',
			d: 'M4 11V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.706.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h7'
		},
		{ shape: 'path', d: 'M14 2v5a1 1 0 0 0 1 1h5' },
		{ shape: 'path', d: 'm10 18 3-3-3-3' }
	]),
	file: shapes([
		{
			shape: 'path',
			d: 'M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z'
		},
		{ shape: 'path', d: 'M14 2v5a1 1 0 0 0 1 1h5' }
	]),
	'folder-open': shapes([
		{
			shape: 'path',
			d: 'm6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2'
		}
	]),
	'folder-symlink': shapes([
		{
			shape: 'path',
			d: 'M2 9.35V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H20a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h7'
		},
		{ shape: 'path', d: 'm8 16 3-3-3-3' }
	]),
	folder: shapes([
		{
			shape: 'path',
			d: 'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z'
		}
	]),
	globe: shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'path', d: 'M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20' },
		{ shape: 'path', d: 'M2 12h20' }
	]),
	hourglass: shapes([
		{ shape: 'path', d: 'M5 22h14' },
		{ shape: 'path', d: 'M5 2h14' },
		{
			shape: 'path',
			d: 'M17 22v-4.172a2 2 0 0 0-.586-1.414L12 12l-4.414 4.414A2 2 0 0 0 7 17.828V22'
		},
		{ shape: 'path', d: 'M7 2v4.172a2 2 0 0 0 .586 1.414L12 12l4.414-4.414A2 2 0 0 0 17 6.172V2' }
	]),
	image: shapes([
		{ shape: 'rect', x: 3, y: 3, width: 18, height: 18, rx: 2, ry: 2 },
		{ shape: 'circle', cx: 9, cy: 9, r: 2 },
		{ shape: 'path', d: 'm21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21' }
	]),
	import: shapes([
		{ shape: 'path', d: 'M12 3v12' },
		{ shape: 'path', d: 'm8 11 4 4 4-4' },
		{
			shape: 'path',
			d: 'M8 5H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-4'
		}
	]),
	inbox: shapes([
		{ shape: 'polyline', points: '22 12 16 12 14 15 10 15 8 12 2 12' },
		{
			shape: 'path',
			d: 'M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z'
		}
	]),
	info: shapes([
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'path', d: 'M12 16v-4' },
		{ shape: 'path', d: 'M12 8h.01' }
	]),
	list: shapes([
		{ shape: 'path', d: 'M3 5h.01' },
		{ shape: 'path', d: 'M3 12h.01' },
		{ shape: 'path', d: 'M3 19h.01' },
		{ shape: 'path', d: 'M8 5h13' },
		{ shape: 'path', d: 'M8 12h13' },
		{ shape: 'path', d: 'M8 19h13' }
	]),
	lock: shapes([
		{ shape: 'rect', x: 3, y: 11, width: 18, height: 11, rx: 2, ry: 2 },
		{ shape: 'path', d: 'M7 11V7a5 5 0 0 1 10 0v4' }
	]),
	menu: shapes([
		{ shape: 'path', d: 'M4 5h16' },
		{ shape: 'path', d: 'M4 12h16' },
		{ shape: 'path', d: 'M4 19h16' }
	]),
	'message-circle': shapes([
		{
			shape: 'path',
			d: 'M2.992 16.342a2 2 0 0 1 .094 1.167l-1.065 3.29a1 1 0 0 0 1.236 1.168l3.413-.998a2 2 0 0 1 1.099.092 10 10 0 1 0-4.777-4.719'
		}
	]),
	minus: shapes([{ shape: 'path', d: 'M5 12h14' }]),
	'package-check': shapes([
		{ shape: 'path', d: 'M12 22V12' },
		{ shape: 'path', d: 'm16 17 2 2 4-4' },
		{
			shape: 'path',
			d: 'M21 11.127V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.729l7 4a2 2 0 0 0 2 .001l1.32-.753'
		},
		{ shape: 'path', d: 'M3.29 7 12 12l8.71-5' },
		{ shape: 'path', d: 'm7.5 4.27 8.997 5.148' }
	]),
	plus: shapes([
		{ shape: 'path', d: 'M5 12h14' },
		{ shape: 'path', d: 'M12 5v14' }
	]),
	search: shapes([
		{ shape: 'path', d: 'm21 21-4.34-4.34' },
		{ shape: 'circle', cx: 11, cy: 11, r: 8 }
	]),
	send: shapes([
		{
			shape: 'path',
			d: 'M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z'
		},
		{ shape: 'path', d: 'm21.854 2.147-10.94 10.939' }
	]),
	'trash-2': shapes([
		{ shape: 'path', d: 'M10 11v6' },
		{ shape: 'path', d: 'M14 11v6' },
		{ shape: 'path', d: 'M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6' },
		{ shape: 'path', d: 'M3 6h18' },
		{ shape: 'path', d: 'M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2' }
	]),
	x: shapes([
		{ shape: 'path', d: 'M18 6 6 18' },
		{ shape: 'path', d: 'm6 6 12 12' }
	]),
	link: shapes([
		{ shape: 'path', d: 'M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71' },
		{ shape: 'path', d: 'M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71' }
	]),
	box: shapes([
		{
			shape: 'path',
			d: 'M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z'
		},
		{ shape: 'path', d: 'm3.3 7 8.7 5 8.7-5' },
		{ shape: 'path', d: 'M12 22V12' }
	]),
	'chevron-left': shapes([{ shape: 'path', d: 'm15 18-6-6 6-6' }]),
	'corner-down-right': shapes([
		{ shape: 'path', d: 'm15 10 5 5-5 5' },
		{ shape: 'path', d: 'M4 4v7a4 4 0 0 0 4 4h12' }
	]),
	bug: shapes([
		{ shape: 'path', d: 'M12 20v-9' },
		{ shape: 'path', d: 'M14 7a4 4 0 0 1 4 4v3a6 6 0 0 1-12 0v-3a4 4 0 0 1 4-4z' },
		{ shape: 'path', d: 'M14.12 3.88 16 2' },
		{ shape: 'path', d: 'M21 21a4 4 0 0 0-3.81-4' },
		{ shape: 'path', d: 'M21 5a4 4 0 0 1-3.55 3.97' },
		{ shape: 'path', d: 'M22 13h-4' },
		{ shape: 'path', d: 'M3 21a4 4 0 0 1 3.81-4' },
		{ shape: 'path', d: 'M3 5a4 4 0 0 0 3.55 3.97' },
		{ shape: 'path', d: 'M6 13H2' },
		{ shape: 'path', d: 'm8 2 1.88 1.88' },
		{ shape: 'path', d: 'M9 7.13V6a3 3 0 1 1 6 0v1.13' }
	]),
	calendar: shapes([
		{ shape: 'path', d: 'M8 2v3' },
		{ shape: 'path', d: 'M16 2v3' },
		{ shape: 'rect', x: 3, y: 3, width: 18, height: 18, rx: 2, ry: 2 },
		{ shape: 'path', d: 'M3 9h18' }
	]),
	flag: shapes([
		{
			shape: 'path',
			d: 'M4 22V4a1 1 0 0 1 .4-.8A6 6 0 0 1 8 2c3 0 5 2 7.333 2q2 0 3.067-.8A1 1 0 0 1 20 4v10a1 1 0 0 1-.4.8A6 6 0 0 1 16 16c-3 0-5-2-8-2a6 6 0 0 0-4 1.528'
		}
	])
});

/** The name of an icon in the registry. */
export type IconName = keyof typeof ICONS;

/** Whether the text names an icon in the registry (its own names only, never inherited ones). */
export function isIconName(text: string): text is IconName {
	return Object.hasOwn(ICONS, text);
}

/** Every icon name, in the registry's order. */
export const ICON_NAMES: readonly IconName[] = Object.freeze(Object.keys(ICONS).filter(isIconName));

/**
 * The app's icon set: a few Lucide icons (lucide-static 1.53.0, ISC License,
 * Copyright (c) 2026 Lucide Icons and Contributors), copied as path data so
 * they ship inline and self-hosted. Each is drawn on a 24-unit grid with a
 * 2-unit round stroke.
 */

/** One SVG shape of an icon. */
export type IconShape =
	| { readonly shape: 'path'; readonly d: string }
	| { readonly shape: 'circle'; readonly cx: number; readonly cy: number; readonly r: number }
	| {
			readonly shape: 'rect';
			readonly x: number;
			readonly y: number;
			readonly width: number;
			readonly height: number;
			readonly rx: number;
	  };

/** Every icon by name. */
export const ICONS = {
	folder: [
		{
			shape: 'path',
			d: 'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z'
		}
	],
	'folder-open': [
		{
			shape: 'path',
			d: 'm6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2'
		}
	],
	file: [
		{
			shape: 'path',
			d: 'M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z'
		},
		{ shape: 'path', d: 'M14 2v5a1 1 0 0 0 1 1h5' }
	],
	link: [
		{ shape: 'path', d: 'M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71' },
		{ shape: 'path', d: 'M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71' }
	],
	box: [
		{
			shape: 'path',
			d: 'M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z'
		},
		{ shape: 'path', d: 'm3.3 7 8.7 5 8.7-5' },
		{ shape: 'path', d: 'M12 22V12' }
	],
	'chevron-right': [{ shape: 'path', d: 'm9 18 6-6-6-6' }],
	'chevron-left': [{ shape: 'path', d: 'm15 18-6-6 6-6' }],
	menu: [
		{ shape: 'path', d: 'M4 5h16' },
		{ shape: 'path', d: 'M4 12h16' },
		{ shape: 'path', d: 'M4 19h16' }
	],
	x: [
		{ shape: 'path', d: 'M18 6 6 18' },
		{ shape: 'path', d: 'm6 6 12 12' }
	],
	lock: [
		{ shape: 'rect', x: 3, y: 11, width: 18, height: 11, rx: 2 },
		{ shape: 'path', d: 'M7 11V7a5 5 0 0 1 10 0v4' }
	],
	eye: [
		{
			shape: 'path',
			d: 'M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0'
		},
		{ shape: 'circle', cx: 12, cy: 12, r: 3 }
	],
	globe: [
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'path', d: 'M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20' },
		{ shape: 'path', d: 'M2 12h20' }
	],
	'corner-down-right': [
		{ shape: 'path', d: 'm15 10 5 5-5 5' },
		{ shape: 'path', d: 'M4 4v7a4 4 0 0 0 4 4h12' }
	],
	info: [
		{ shape: 'circle', cx: 12, cy: 12, r: 10 },
		{ shape: 'path', d: 'M12 16v-4' },
		{ shape: 'path', d: 'M12 8h.01' }
	],
	archive: [
		{ shape: 'rect', x: 2, y: 3, width: 20, height: 5, rx: 1 },
		{ shape: 'path', d: 'M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8' },
		{ shape: 'path', d: 'M10 12h4' }
	]
} as const satisfies Record<string, readonly IconShape[]>;

/** The name of an icon in {@link ICONS}. */
export type IconName = keyof typeof ICONS;

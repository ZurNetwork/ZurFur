import { afterAll, describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32, deflateSync } from 'node:zlib';
import { checkReferences, main, pngFindings, type ReferenceRule } from './references.ts';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** Every fixture folder lives under one temporary folder, removed after the run. */
const scratch = mkdtempSync(path.join(tmpdir(), 'ui-references-'));

afterAll(() => {
	rmSync(scratch, { recursive: true, force: true });
});

/** One PNG chunk: its length, type, data and CRC. */
function chunk(type: string, data: Buffer): Buffer {
	const typeAndData = Buffer.concat([Buffer.from(type, 'latin1'), data]);
	const length = Buffer.alloc(4);
	length.writeUInt32BE(data.length);
	const crc = Buffer.alloc(4);
	crc.writeUInt32BE(crc32(typeAndData));
	return Buffer.concat([length, typeAndData, crc]);
}

const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** An IHDR chunk for an 8-bit greyscale picture of the given size. */
function header(width: number, height: number): Buffer {
	const data = Buffer.alloc(13);
	data.writeUInt32BE(width, 0);
	data.writeUInt32BE(height, 4);
	data.writeUInt8(8, 8);
	return chunk('IHDR', data);
}

/** A valid 2×2 greyscale PNG, with `extra` chunks before its pixels. */
function png(
	extra: readonly Buffer[] = [],
	size: { width: number; height: number } = { width: 2, height: 2 }
): Buffer {
	const rows = Buffer.from([0, 0x10, 0x20, 0, 0x30, 0x40]);
	const pixels = chunk('IDAT', deflateSync(rows));
	return Buffer.concat([
		signature,
		header(size.width, size.height),
		...extra,
		pixels,
		chunk('IEND', Buffer.alloc(0))
	]);
}

let fixtureCount = 0;

/** Writes a reference folder holding `files` (relative path to bytes) and returns its path. */
function referenceFolder(files: Readonly<Record<string, Buffer>>): string {
	fixtureCount += 1;
	const folder = path.join(scratch, `refs-${String(fixtureCount)}`);
	mkdirSync(folder, { recursive: true });
	for (const [relative, bytes] of Object.entries(files)) {
		const file = path.join(folder, relative);
		mkdirSync(path.dirname(file), { recursive: true });
		writeFileSync(file, bytes);
	}
	return folder;
}

const clean = {
	'controls-button--primary/dark.png': png(),
	'controls-button--primary/light.png': png(),
	'controls-button--docs/dark.png': png()
};

function rulesOf(folder: string): readonly ReferenceRule[] {
	return checkReferences(folder).findings.map((finding) => finding.rule);
}

interface Run {
	readonly code: number;
	readonly out: string;
	readonly err: string;
}

function run(argv: readonly string[]): Run {
	let out = '';
	let err = '';
	const code = main(
		argv,
		{
			write: (text: string) => {
				out += text;
			}
		},
		{
			write: (text: string) => {
				err += text;
			}
		}
	);
	return { code, out, err };
}

describe('the reference-picture check', () => {
	it('passes a clean folder', () => {
		const folder = referenceFolder(clean);
		expect(checkReferences(folder)).toEqual({ findings: [], errors: [] });
		expect(run([folder])).toEqual({ code: 0, out: '', err: '' });
	});

	it('passes an empty folder', () => {
		expect(checkReferences(referenceFolder({}))).toEqual({ findings: [], errors: [] });
	});

	const refused: readonly (readonly [string, Readonly<Record<string, Buffer>>, ReferenceRule])[] = [
		['a picture of another skin', { 'a/os.png': png() }, 'layout'],
		['a picture with no folder', { 'dark.png': png() }, 'layout'],
		['a folder named off the id pattern', { 'Controls-Button/dark.png': png() }, 'layout'],
		['a folder named with a dot', { 'a.b/dark.png': png() }, 'layout'],
		['a nested folder', { 'a/b/dark.png': png() }, 'layout'],
		['a file that is not a picture', { 'a/dark.png.txt': Buffer.from('x') }, 'layout'],
		['a picture in upper case', { 'a/DARK.png': png() }, 'layout'],
		['a file that is not a PNG', { 'a/dark.png': Buffer.from('GIF89a') }, 'not-png'],
		['an empty file', { 'a/dark.png': Buffer.alloc(0) }, 'not-png'],
		['a PNG with no IEND', { 'a/dark.png': png().subarray(0, -12) }, 'not-png'],
		['a truncated PNG', { 'a/dark.png': png().subarray(0, 40) }, 'not-png'],
		[
			'a PNG whose first chunk is not IHDR',
			{ 'a/dark.png': Buffer.concat([signature, chunk('IEND', Buffer.alloc(0))]) },
			'not-png'
		],
		[
			'a PNG wider than 4096 px',
			{ 'a/dark.png': png([], { width: 4097, height: 10 }) },
			'too-wide'
		],
		[
			'a PNG taller than 4096 px',
			{ 'a/dark.png': png([], { width: 10, height: 4097 }) },
			'too-wide'
		],
		[
			'a file over 1 MiB',
			{ 'a/dark.png': Buffer.concat([png(), Buffer.alloc(1024 * 1024)]) },
			'too-large'
		],
		[
			'a tEXt chunk',
			{ 'a/dark.png': png([chunk('tEXt', Buffer.from('Author\0someone'))]) },
			'metadata'
		],
		[
			'a zTXt chunk',
			{ 'a/dark.png': png([chunk('zTXt', Buffer.from('Comment\0\0x'))]) },
			'metadata'
		],
		[
			'an iTXt chunk',
			{ 'a/dark.png': png([chunk('iTXt', Buffer.from('Title\0\0\0\0\0x'))]) },
			'metadata'
		],
		['a tIME chunk', { 'a/dark.png': png([chunk('tIME', Buffer.alloc(7))]) }, 'metadata'],
		['an eXIf chunk', { 'a/dark.png': png([chunk('eXIf', Buffer.from('MM\0*'))]) }, 'metadata'],
		[
			'bytes after IEND',
			{ 'a/dark.png': Buffer.concat([png(), Buffer.from('trailer')]) },
			'trailing-bytes'
		]
	];

	it.each(refused)('refuses %s', (_name, files, rule) => {
		const folder = referenceFolder(files);
		expect(rulesOf(folder)).toEqual([rule]);
		expect(run([folder]).code).toBe(1);
	});

	it('exactly 4096 px a side and exactly 1 MiB pass', () => {
		const square = png([], { width: 4096, height: 4096 });
		expect(pngFindings('a/dark.png', square)).toEqual([]);
		const padded = Buffer.concat([
			signature,
			header(2, 2),
			chunk('zzZz', Buffer.alloc(1024 * 1024 - png().length - 12)),
			png().subarray(signature.length + 25)
		]);
		expect(padded.length).toBe(1024 * 1024);
		expect(checkReferences(referenceFolder({ 'a/dark.png': padded }))).toEqual({
			findings: [],
			errors: []
		});
	});

	it('refuses a folder with no picture', () => {
		const folder = referenceFolder({});
		mkdirSync(path.join(folder, 'controls-button--primary'));
		expect(rulesOf(folder)).toEqual(['layout']);
	});

	it('refuses a link to a picture, and to a folder, without following either', () => {
		const target = referenceFolder({ 'b/dark.png': png() });
		const folder = referenceFolder({ 'a/light.png': png() });
		symlinkSync(path.join(target, 'b', 'dark.png'), path.join(folder, 'a', 'dark.png'));
		symlinkSync(path.join(target, 'b'), path.join(folder, 'b'));
		expect([...rulesOf(folder)].sort()).toEqual(['link', 'link']);
	});

	it('refuses a reference folder that is a link, as unreadable', () => {
		const target = referenceFolder(clean);
		const link = path.join(scratch, 'refs-link');
		symlinkSync(target, link);
		expect(run([link]).code).toBe(2);
	});

	it('refuses a FIFO', () => {
		const folder = referenceFolder({ 'a/light.png': png() });
		const made = spawnSync('mkfifo', [path.join(folder, 'a', 'dark.png')]);
		expect(made.status).toBe(0);
		expect(rulesOf(folder)).toEqual(['special-file']);
	});

	it('exits 2 on a missing folder or bad usage', () => {
		expect(run([path.join(scratch, 'missing')]).code).toBe(2);
		expect(run([]).code).toBe(2);
		expect(run(['a', 'b']).code).toBe(2);
		expect(run(['--update']).code).toBe(2);
	});

	it('runs from the command line', () => {
		const folder = referenceFolder({ 'a/dark.png': Buffer.from('not a png') });
		const result = spawnSync(process.execPath, ['src/checks/references.ts', folder], {
			cwd: uiRoot,
			encoding: 'utf8'
		});
		expect(result.status).toBe(1);
		expect(result.stdout).toContain('[not-png] no PNG signature');
	});

	it('passes the real reference pictures (no folder yet means nothing to check)', () => {
		const real = path.join(uiRoot, 'shots', '__screenshots__');
		const report = existsSync(real) ? checkReferences(real) : { findings: [], errors: [] };
		expect(report).toEqual({ findings: [], errors: [] });
	});
});

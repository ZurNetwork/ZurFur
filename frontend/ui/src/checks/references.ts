/**
 * The reference-picture check: the folder holds exactly `<entry id>/<dark|light>.png`
 * (ids of a-z, 0-9 and -), read without following links. Each picture is a PNG of at
 * most 1 MiB and 4096 px a side with only IHDR, IDAT and IEND chunks, every CRC right,
 * one IDAT run that is one zlib stream of exactly IHDR's size, an empty IEND,
 * nothing after it.
 *
 * Run by Node from `frontend/ui`: `node src/checks/references.ts shots/__screenshots__`.
 * Exit 0 clean, 1 findings, 2 unreadable input or bad usage.
 */
import { lstatSync, readdirSync, readFileSync, realpathSync, type Dirent } from 'node:fs';
import { Buffer } from 'node:buffer';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { crc32, inflateSync } from 'node:zlib';

export type ReferenceRule =
	| 'link'
	| 'special-file'
	| 'layout'
	| 'too-large'
	| 'not-png'
	| 'too-wide'
	| 'metadata'
	| 'bad-crc'
	| 'pixel-stream'
	| 'trailing-bytes';

export interface ReferenceFinding {
	readonly rule: ReferenceRule;
	readonly path: string;
	readonly found: string;
}

/** What one run found: findings (exit 1) and inputs it could not read (exit 2). */
export interface ReferenceReport {
	readonly findings: readonly ReferenceFinding[];
	readonly errors: readonly string[];
}

/** The largest picture, in bytes. */
export const maxPictureBytes = 1024 * 1024;

/** The largest picture side, in pixels. */
export const maxPictureSide = 4096;

/** The picture names, one per skin. */
const pictureNames: ReadonlySet<string> = new Set(['dark.png', 'light.png']);

/** The only chunks a picture may hold; any other is refused by name. */
const allowedChunks: ReadonlySet<string> = new Set(['IHDR', 'IDAT', 'IEND']);

/** Samples per pixel by IHDR colour type, with the bit depths each allows. */
const colourTypes: ReadonlyMap<
	number,
	{ readonly samples: number; readonly depths: readonly number[] }
> = new Map([
	[0, { samples: 1, depths: [1, 2, 4, 8, 16] }],
	[2, { samples: 3, depths: [8, 16] }],
	[3, { samples: 1, depths: [1, 2, 4, 8] }],
	[4, { samples: 2, depths: [8, 16] }],
	[6, { samples: 4, depths: [8, 16] }]
]);

/** The Adam7 passes: x start, y start, x step, y step. */
const adam7Passes: readonly (readonly [number, number, number, number])[] = [
	[0, 0, 8, 8],
	[4, 0, 8, 8],
	[0, 4, 4, 8],
	[2, 0, 4, 4],
	[0, 2, 2, 4],
	[1, 0, 2, 2],
	[0, 1, 1, 2]
];

const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** True when `name` may name a picture's folder. */
export function isEntryIdName(name: string): boolean {
	return /^[a-z0-9-]+$/.test(name);
}

/** The image a valid IHDR describes. */
interface Header {
	readonly width: number;
	readonly height: number;
	readonly bitsPerPixel: number;
	readonly interlaced: boolean;
}

/** IHDR's data parsed, or what is wrong with it. */
function parseHeader(data: Buffer): Header | string {
	const width = data.readUInt32BE(0);
	const height = data.readUInt32BE(4);
	const depth = data.readUInt8(8);
	const colour = colourTypes.get(data.readUInt8(9));
	const interlace = data.readUInt8(12);
	if (width === 0 || height === 0) return 'an IHDR with no pixels';
	if (colour?.depths.includes(depth) !== true) {
		return 'an IHDR with an unknown colour type or bit depth';
	}
	if (data.readUInt8(10) !== 0 || data.readUInt8(11) !== 0 || interlace > 1) {
		return 'an IHDR with an unknown compression, filter or interlace method';
	}
	return { width, height, bitsPerPixel: colour.samples * depth, interlaced: interlace === 1 };
}

/** The bytes of filtered rows the header implies: one filter byte per row, per pass. */
function rowBytes(header: Header): number {
	const passBytes = (width: number, height: number): number =>
		width === 0 || height === 0 ? 0 : height * (1 + Math.ceil((width * header.bitsPerPixel) / 8));
	if (!header.interlaced) return passBytes(header.width, header.height);
	return adam7Passes.reduce(
		(total, [xStart, yStart, xStep, yStep]) =>
			total +
			passBytes(
				Math.max(0, Math.ceil((header.width - xStart) / xStep)),
				Math.max(0, Math.ceil((header.height - yStart) / yStep))
			),
		0
	);
}

/** A chunk type as printed: itself when four ASCII letters, else its bytes in hex. */
function chunkName(bytes: Buffer, start: number): string {
	const type = bytes.toString('latin1', start, start + 4);
	return /^[A-Za-z]{4}$/.test(type) ? type : `0x${bytes.toString('hex', start, start + 4)}`;
}

/** What `inflateSync` returns with `info`: the bytes, and the engine with the input it read. */
interface Inflated {
	readonly buffer: Buffer;
	readonly engine: { readonly bytesWritten: number };
}

function isInflated(value: unknown): value is Inflated {
	return (
		value instanceof Object &&
		'buffer' in value &&
		Buffer.isBuffer(value.buffer) &&
		'engine' in value &&
		value.engine instanceof Object &&
		'bytesWritten' in value.engine &&
		typeof value.engine.bytesWritten === 'number'
	);
}

/** What is wrong with the IDAT data; undefined when it is one zlib stream of exactly `expected` bytes. */
function streamProblem(stream: Buffer, expected: number): string | undefined {
	let result: unknown;
	try {
		result = inflateSync(stream, { info: true, maxOutputLength: expected });
	} catch (error) {
		return `IDAT data that does not inflate to ${String(expected)} bytes (${reasonOf(error)})`;
	}
	if (!isInflated(result)) return 'IDAT data the inflater gave no account of';
	if (result.buffer.length !== expected) {
		return `IDAT data inflating to ${String(result.buffer.length)} bytes, not ${String(expected)}`;
	}
	const consumed = result.engine.bytesWritten;
	if (consumed !== stream.length) {
		return `${String(stream.length - consumed)} byte(s) of IDAT data after the zlib stream`;
	}
	return undefined;
}

/** The findings in one picture's bytes. */
export function pngFindings(file: string, bytes: Buffer): readonly ReferenceFinding[] {
	const finding = (rule: ReferenceRule, found: string): ReferenceFinding => ({
		rule,
		path: file,
		found
	});
	if (bytes.length < pngSignature.length || !bytes.subarray(0, 8).equals(pngSignature)) {
		return [finding('not-png', 'no PNG signature')];
	}
	const findings: ReferenceFinding[] = [];
	const idatParts: Buffer[] = [];
	let header: Header | undefined;
	let streamReported = false;
	let idatRunEnded = false;
	let offset = pngSignature.length;
	while (offset < bytes.length) {
		if (offset + 12 > bytes.length) return [...findings, finding('not-png', 'a truncated chunk')];
		const length = bytes.readUInt32BE(offset);
		const name = chunkName(bytes, offset + 4);
		const dataStart = offset + 8;
		const next = dataStart + length + 4;
		if (next > bytes.length) return [...findings, finding('not-png', `a truncated ${name} chunk`)];
		const isFirst = offset === pngSignature.length;
		if (isFirst !== (name === 'IHDR')) {
			const found = isFirst ? 'the first chunk is not IHDR' : 'a second IHDR chunk';
			return [...findings, finding('not-png', found)];
		}
		if (
			crc32(bytes.subarray(offset + 4, dataStart + length)) !==
			bytes.readUInt32BE(dataStart + length)
		) {
			findings.push(finding('bad-crc', `chunk ${name}, whose CRC does not match`));
		}
		if (!allowedChunks.has(name)) {
			findings.push(finding('metadata', `chunk ${name}, not IHDR, IDAT or IEND`));
		}
		if (name === 'IEND' && length !== 0) {
			findings.push(
				finding('metadata', `chunk IEND, holding ${String(length)} byte(s), not empty`)
			);
		}
		if (name === 'IHDR') {
			if (length !== 13) {
				return [...findings, finding('not-png', 'an IHDR chunk not 13 bytes long')];
			}
			const width = bytes.readUInt32BE(dataStart);
			const height = bytes.readUInt32BE(dataStart + 4);
			if (width > maxPictureSide || height > maxPictureSide) {
				findings.push(
					finding(
						'too-wide',
						`${String(width)}×${String(height)} px, over ${String(maxPictureSide)} a side`
					)
				);
			} else {
				const parsed = parseHeader(bytes.subarray(dataStart, dataStart + 13));
				if (typeof parsed === 'string') return [...findings, finding('not-png', parsed)];
				header = parsed;
			}
		}
		if (name === 'IDAT') {
			if (idatRunEnded && !streamReported) {
				streamReported = true;
				findings.push(finding('pixel-stream', 'IDAT chunks not in one run'));
			}
			idatParts.push(bytes.subarray(dataStart, dataStart + length));
		} else if (idatParts.length > 0) {
			idatRunEnded = true;
		}
		offset = next;
		if (name === 'IEND') {
			if (header !== undefined && !streamReported) {
				const problem =
					idatParts.length === 0
						? 'no IDAT chunk'
						: streamProblem(Buffer.concat(idatParts), rowBytes(header));
				if (problem !== undefined) findings.push(finding('pixel-stream', problem));
			}
			if (offset < bytes.length) {
				findings.push(
					finding('trailing-bytes', `${String(bytes.length - offset)} byte(s) after IEND`)
				);
			}
			return findings;
		}
	}
	return [...findings, finding('not-png', 'no IEND chunk')];
}

function reasonOf(error: unknown): string {
	return error instanceof Error ? error.message : 'unreadable';
}

function entriesOf(folder: string, errors: string[]): readonly Dirent[] {
	try {
		return readdirSync(folder, { withFileTypes: true });
	} catch (error) {
		errors.push(`${folder}: ${reasonOf(error)}`);
		return [];
	}
}

/** A link or anything but a plain file or folder, as a finding; undefined for those two. */
function kindFinding(entryPath: string, entry: Dirent): ReferenceFinding | undefined {
	if (entry.isSymbolicLink()) {
		return { rule: 'link', path: entryPath, found: 'a link, which the check does not follow' };
	}
	if (entry.isFile() || entry.isDirectory()) return undefined;
	return { rule: 'special-file', path: entryPath, found: 'neither a plain file nor a folder' };
}

function pictureFindings(file: string, errors: string[]): readonly ReferenceFinding[] {
	let size;
	try {
		size = lstatSync(file).size;
	} catch (error) {
		errors.push(`${file}: ${reasonOf(error)}`);
		return [];
	}
	if (size > maxPictureBytes) {
		return [{ rule: 'too-large', path: file, found: `${String(size)} bytes, over 1 MiB` }];
	}
	try {
		return pngFindings(file, readFileSync(file));
	} catch (error) {
		errors.push(`${file}: ${reasonOf(error)}`);
		return [];
	}
}

function folderFindings(folder: string, errors: string[]): readonly ReferenceFinding[] {
	const errorsBefore = errors.length;
	const entries = entriesOf(folder, errors);
	if (entries.length === 0 && errors.length === errorsBefore) {
		return [{ rule: 'layout', path: folder, found: 'a folder with no picture' }];
	}
	return entries.flatMap((entry): readonly ReferenceFinding[] => {
		const entryPath = path.join(folder, entry.name);
		const refused = kindFinding(entryPath, entry);
		if (refused !== undefined) return [refused];
		if (entry.isDirectory() || !pictureNames.has(entry.name)) {
			return [{ rule: 'layout', path: entryPath, found: 'not dark.png or light.png' }];
		}
		return pictureFindings(entryPath, errors);
	});
}

/** Checks a reference folder: its layout, and every picture in it. */
export function checkReferences(folder: string): ReferenceReport {
	let stat;
	try {
		stat = lstatSync(folder);
	} catch (error) {
		return { findings: [], errors: [`${folder}: ${reasonOf(error)}`] };
	}
	if (!stat.isDirectory()) {
		return { findings: [], errors: [`${folder}: not a folder (a link is not followed)`] };
	}
	const errors: string[] = [];
	const findings = entriesOf(folder, errors).flatMap((entry): readonly ReferenceFinding[] => {
		const entryPath = path.join(folder, entry.name);
		const refused = kindFinding(entryPath, entry);
		if (refused !== undefined) return [refused];
		if (!entry.isDirectory() || !isEntryIdName(entry.name)) {
			return [{ rule: 'layout', path: entryPath, found: 'not a folder named by an entry id' }];
		}
		return folderFindings(entryPath, errors);
	});
	return { findings, errors };
}

/** Where the check writes: a stream's `write`, so a spec can collect the text. */
interface Output {
	write(text: string): unknown;
}

/** Formats one finding as the line the check prints. */
export function formatFinding(finding: ReferenceFinding): string {
	return `${finding.path}: [${finding.rule}] ${finding.found}`;
}

/** Runs the check as the CLI does and returns its exit code; it never throws. */
export function main(argv: readonly string[], out: Output, err: Output): number {
	try {
		const [folder, ...extra] = argv;
		if (folder === undefined || extra.length > 0 || folder.startsWith('--')) {
			err.write('references: usage: node src/checks/references.ts <reference folder>\n');
			return 2;
		}
		const { findings, errors } = checkReferences(folder);
		for (const finding of findings) out.write(`${formatFinding(finding)}\n`);
		for (const error of errors) err.write(`references: ${error}\n`);
		if (errors.length > 0) return 2;
		if (findings.length > 0) {
			err.write(`references: ${String(findings.length)} finding(s)\n`);
			return 1;
		}
		return 0;
	} catch (error) {
		err.write(`references: internal error: ${reasonOf(error)}\n`);
		return 2;
	}
}

/** True when Node runs this file as its entry point, not when a spec imports it. */
function isEntryPoint(): boolean {
	const entry = process.argv[1];
	if (entry === undefined) return false;
	try {
		const self = realpathSync(fileURLToPath(import.meta.url));
		return pathToFileURL(realpathSync(entry)).href === pathToFileURL(self).href;
	} catch {
		return false;
	}
}

if (isEntryPoint()) process.exitCode = main(process.argv.slice(2), process.stdout, process.stderr);

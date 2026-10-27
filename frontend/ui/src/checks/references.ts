/**
 * The reference-picture check: the screenshot run's reference folder holds
 * exactly `<entry id>/<dark|light>.png`, read without following links, each
 * id lower-case letters, digits and hyphens. Each picture is a plain PNG of at
 * most 1 MiB and 4096 px a side, with no text or time chunk and nothing after
 * its end, so no picture carries anything but pixels.
 *
 * Run by Node from `frontend/ui`: `node src/checks/references.ts shots/__screenshots__`.
 * Exit 0 clean, 1 findings, 2 unreadable input or bad usage.
 */
import { lstatSync, readdirSync, readFileSync, realpathSync, type Dirent } from 'node:fs';
import { Buffer } from 'node:buffer';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export type ReferenceRule =
	| 'link'
	| 'special-file'
	| 'layout'
	| 'too-large'
	| 'not-png'
	| 'too-wide'
	| 'metadata'
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

/** The chunks that carry text or a time rather than pixels. */
const refusedChunks: ReadonlySet<string> = new Set(['tEXt', 'zTXt', 'iTXt', 'tIME', 'eXIf']);

const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** True when `name` may name a picture's folder. */
export function isEntryIdName(name: string): boolean {
	return /^[a-z0-9-]+$/.test(name);
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
	let offset = pngSignature.length;
	let isFirst = true;
	while (offset < bytes.length) {
		if (offset + 12 > bytes.length) return [...findings, finding('not-png', 'a truncated chunk')];
		const length = bytes.readUInt32BE(offset);
		const type = bytes.toString('latin1', offset + 4, offset + 8);
		const dataStart = offset + 8;
		const next = dataStart + length + 4;
		if (next > bytes.length) return [...findings, finding('not-png', `a truncated ${type} chunk`)];
		if (isFirst) {
			if (type !== 'IHDR' || length !== 13) {
				return [...findings, finding('not-png', 'the first chunk is not IHDR')];
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
			}
			isFirst = false;
		}
		if (refusedChunks.has(type)) findings.push(finding('metadata', `a ${type} chunk`));
		offset = next;
		if (type === 'IEND') {
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

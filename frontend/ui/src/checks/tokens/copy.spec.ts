import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterAll, describe, expect, it } from 'vitest';
import { MASK, SAMPLE_SUBSTITUTIONS, substituteSamples } from './copy';

/** Made-up originals, not anyone's identifiers. */
const FIRST = '@someone.example';
const SECOND = 'did:plc:zzzzzzzzzzzzzzzzzzzzzzzz';

function source(first: string, second: string, usage = 'u'): string {
	const document = {
		type: { groups: [{ styles: [] }, { styles: [{ sample: first, usage }, { sample: second }] }] }
	};
	return JSON.stringify(document, null, '\t');
}

describe('substituteSamples', () => {
	it('swaps the two listed values and leaves every other byte alone', () => {
		const outcome = substituteSamples(source(FIRST, SECOND));
		const expected = source(
			SAMPLE_SUBSTITUTIONS[0]?.replacement ?? '',
			SAMPLE_SUBSTITUTIONS[1]?.replacement ?? ''
		);
		expect(outcome.kind === 'substituted' && outcome.text).toBe(expected);
	});

	it('hashes the source with the values masked, whatever the values are', () => {
		const first = substituteSamples(source(FIRST, SECOND));
		const other = substituteSamples(source('@another.example', 'did:plc:yyyyyyyyyyyyyyyyyyyyyyyy'));
		const expected = createHash('sha256').update(source(MASK, MASK)).digest('hex');
		expect([
			first.kind === 'substituted' && first.maskedSha256,
			other.kind === 'substituted' && other.maskedSha256
		]).toEqual([expected, expected]);
	});

	it('changes the hash when a byte outside the listed values changes', () => {
		const plain = substituteSamples(source(FIRST, SECOND));
		const edited = substituteSamples(source(FIRST, SECOND, 'changed'));
		expect(plain.kind === 'substituted' && plain.maskedSha256).not.toBe(
			edited.kind === 'substituted' && edited.maskedSha256
		);
	});

	it('refuses when an original occurs a second time, without quoting it', () => {
		const outcome = substituteSamples(source(FIRST, SECOND, FIRST));
		expect(outcome.kind).toBe('refused');
		expect(JSON.stringify(outcome)).not.toContain(FIRST);
	});

	it('refuses when a listed path leads nowhere', () => {
		expect(substituteSamples(JSON.stringify({ type: {} })).kind).toBe('refused');
	});

	it('refuses text that is not JSON, without echoing it', () => {
		const outcome = substituteSamples(`{ ${FIRST} `);
		expect(outcome.kind).toBe('refused');
		expect(JSON.stringify(outcome)).not.toContain(FIRST);
	});
});

describe('the command line', () => {
	const script = fileURLToPath(new URL('./copy.ts', import.meta.url));
	const committedPath = fileURLToPath(new URL('../../lib/styles/tokens.json', import.meta.url));
	const scratch = mkdtempSync(path.join(os.tmpdir(), 'ui-copy-'));
	afterAll(() => {
		rmSync(scratch, { recursive: true, force: true });
	});

	function run(...args: string[]): { readonly status: number | null; readonly output: string } {
		const result = spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });
		return { status: result.status, output: `${result.stdout}${result.stderr}` };
	}

	it('exits 2 for a source it cannot read', () => {
		expect(run('--check', path.join(scratch, 'missing.json')).status).toBe(2);
	});

	it('exits 2 without a source', () => {
		expect(run('--check').status).toBe(2);
	});

	it('refuses, with exit 1, a source whose masked hash differs, and writes nothing', () => {
		const file = path.join(scratch, 'other.json');
		writeFileSync(file, source(FIRST, SECOND));
		const before = readFileSync(committedPath, 'utf8');
		const checked = run('--check', file);
		const copied = run(file);
		expect([checked.status, copied.status]).toEqual([1, 1]);
		expect(checked.output + copied.output).not.toContain(FIRST);
		expect(checked.output).toContain('SOURCE_MASKED_SHA256');
		expect(readFileSync(committedPath, 'utf8')).toBe(before);
	});
});

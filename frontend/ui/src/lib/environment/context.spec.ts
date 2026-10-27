import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
import { STILL_ENVIRONMENT } from './context.ts';
import { NO_MOVES } from './moves.ts';
import { NO_SOUNDS } from './sounds.ts';

const seamFiles = ['moves.ts', 'sounds.ts', 'context.ts'];

/** The packages a seam file may import from. */
const allowedPackages = ['svelte', 'svelte/attachments'];

/** The seam's own files, as they import each other. */
const allowedRelative = ['./moves', './sounds'];

/** The packages a seam file may import only types from. */
const typeOnlyPackages = ['svelte/attachments'];

interface SeamImport {
	readonly specifier: string;
	readonly typeOnly: boolean;
}

/** Every import and re-export a seam file makes, read from its syntax. */
function importsOf(file: string): readonly SeamImport[] {
	const text = readFileSync(new URL(file, import.meta.url), 'utf8');
	const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
	return source.statements.flatMap((statement): SeamImport[] => {
		if (ts.isImportDeclaration(statement) && ts.isStringLiteral(statement.moduleSpecifier)) {
			const typeOnly = statement.importClause?.phaseModifier === ts.SyntaxKind.TypeKeyword;
			return [{ specifier: statement.moduleSpecifier.text, typeOnly }];
		}
		if (
			ts.isExportDeclaration(statement) &&
			statement.moduleSpecifier !== undefined &&
			ts.isStringLiteral(statement.moduleSpecifier)
		) {
			return [{ specifier: statement.moduleSpecifier.text, typeOnly: statement.isTypeOnly }];
		}
		return [];
	});
}

describe('the still environment', () => {
	it('is frozen, and so are its moves and sounds', () => {
		expect(Object.isFrozen(STILL_ENVIRONMENT)).toBe(true);
		expect(Object.isFrozen(NO_MOVES)).toBe(true);
		expect(Object.isFrozen(NO_SOUNDS)).toBe(true);
	});

	it('is made of the no-op moves and sounds', () => {
		expect(STILL_ENVIRONMENT.moves).toBe(NO_MOVES);
		expect(STILL_ENVIRONMENT.sounds).toBe(NO_SOUNDS);
		expect(Object.keys(STILL_ENVIRONMENT).sort()).toEqual(['moves', 'sounds']);
	});

	it('attaches moves that do nothing and return nothing', () => {
		const element = { style: {} };
		const attachments = [
			NO_MOVES.squish(),
			NO_MOVES.note('in-place'),
			NO_MOVES.note('edge-start'),
			NO_MOVES.note('edge-end'),
			NO_MOVES.note('below')
		];
		for (const attachment of attachments) {
			expect(attachment(Object.freeze(element) as unknown as HTMLElement)).toBeUndefined();
		}
		expect(Object.keys(NO_MOVES).sort()).toEqual(['note', 'squish']);
	});

	it('plays nothing, for every cue', () => {
		const cues = ['open', 'close', 'toggle'] as const;
		for (const cue of cues) {
			expect(() => {
				NO_SOUNDS.play(cue);
			}).not.toThrow();
		}
		expect(Object.keys(NO_SOUNDS)).toEqual(['play']);
	});
});

describe('the seam files', () => {
	it.each(seamFiles)('%s imports only svelte, svelte/attachments and the seam itself', (file) => {
		const refused = importsOf(file).filter(
			({ specifier }) =>
				!allowedPackages.includes(specifier) && !allowedRelative.includes(specifier)
		);
		expect(refused).toEqual([]);
	});

	it.each(seamFiles)('%s imports only types from svelte/attachments', (file) => {
		const valueImports = importsOf(file).filter(
			({ specifier, typeOnly }) => typeOnlyPackages.includes(specifier) && !typeOnly
		);
		expect(valueImports).toEqual([]);
	});

	it('reads at least one import, so a parse that finds none cannot pass', () => {
		expect(seamFiles.flatMap((file) => importsOf(file)).length).toBeGreaterThan(0);
	});
});

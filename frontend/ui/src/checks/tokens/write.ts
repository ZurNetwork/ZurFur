import { readFileSync, writeFileSync } from 'node:fs';
import { describeRefusal, generateTokensCss } from './generate.ts';

const stylesFolder = new URL('../../lib/styles/', import.meta.url);

/** Reads one JSON file of the styles folder as untyped data. */
function readJson(name: string): unknown {
	const parsed: unknown = JSON.parse(readFileSync(new URL(name, stylesFolder), 'utf8'));
	return parsed;
}

/** Runs the command line; returns the exit code. */
function main(): number {
	let tokens: unknown;
	let additions: unknown;
	try {
		tokens = readJson('tokens.json');
		additions = readJson('tokens.additions.json');
	} catch {
		console.error('cannot read tokens.json or tokens.additions.json as JSON');
		return 2;
	}
	const outcome = generateTokensCss(tokens, additions);
	if (outcome.kind === 'refused') {
		for (const refusal of outcome.refusals) console.error(describeRefusal(refusal));
		return 1;
	}
	writeFileSync(new URL('tokens.css', stylesFolder), outcome.css);
	console.log('wrote src/lib/styles/tokens.css');
	return 0;
}

process.exitCode = main();

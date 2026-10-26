import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { fileURLToPath } from 'node:url';

function compilerOptionsOf(relative: string): unknown {
	const path = fileURLToPath(new URL(relative, import.meta.url));
	const read = ts.readConfigFile(path, (file) => ts.sys.readFile(file));
	expect(read.error).toBeUndefined();
	const config: unknown = read.config;
	expect(config).toHaveProperty('compilerOptions');
	return Reflect.get(Object(config), 'compilerOptions');
}

describe('tsconfig parity', () => {
	it("keeps ui's compilerOptions equal to web's", () => {
		const ui = compilerOptionsOf('../../tsconfig.json');
		const web = compilerOptionsOf('../../../web/tsconfig.json');
		expect(ui).toEqual(web);
	});
});

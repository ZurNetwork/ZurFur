// Tests for the npm licence rule, offline: no network, and the clock is a fixed date.
//   node --test frontend/licences.test.mjs

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { GuardError } from './install-check.mjs';
import {
	isScopedPackage,
	judgeLicence,
	loadLicenceList,
	NEVER_ALLOW,
	packageNameProblem,
	parseLicenceExpression
} from './licences.mjs';

const FRONTEND = path.dirname(fileURLToPath(import.meta.url));
const MODULE_FILE = path.join(FRONTEND, 'licences.mjs');
const REAL_LIST_FILE = path.join(FRONTEND, '..', 'npm-licences.json');

/** The only files licences.mjs may import: it stays pure and never imports lock-review.mjs, which imports it. */
const LICENCES_IMPORTS = Object.freeze(['licences.mjs', 'install-check.mjs']);

const TODAY = '2026-10-26';
const HASH = 'a'.repeat(64);

/** A provenance block read today. */
function provenance(extra = {}) {
	return { reason: 'read in the fixture', readBy: 'Fixture', readOn: TODAY, ...extra };
}

function scopedEntry(extra = {}) {
	return { licence: 'MPL-2.0', packages: ['@axe-core/playwright', 'axe-core'], ...provenance(), ...extra };
}

function exceptionEntry(extra = {}) {
	return { package: 'svelte-toolbelt@0.10.6', registry: null, terms: 'MIT', ...provenance(), ...extra };
}

/** The fixture list as a document; `overrides` replaces top-level keys. */
function fixtureDocument(overrides = {}) {
	return {
		allow: ['Apache-2.0', 'ISC', 'MIT'],
		scoped: [
			scopedEntry(),
			{ licence: 'OFL-1.1', packages: ['@fontsource-variable/inter', '@fortawesome/fontawesome-free'], ...provenance() }
		],
		exceptions: [
			exceptionEntry(),
			exceptionEntry({ package: 'odd-pkg@1.2.3', registry: 'SEE LICENSE IN LICENSE' }),
			{
				package: 'ref-pkg@2.0.0',
				registry: 'SEE LICENSE IN LICENCE.txt',
				terms: 'LicenseRef-ref-pkg',
				textSha256: HASH,
				...provenance()
			}
		],
		...overrides
	};
}

/** A document in the list's canonical layout. */
function canonical(document) {
	return `${JSON.stringify(document, null, 2)}\n`;
}

const LIST = loadLicenceList(canonical(fixtureDocument()), { today: TODAY });

/** Judges a manifest for `name@version` against the fixture list. */
function judge(manifest, { name = 'left-pad', version = '1.3.0', list = LIST } = {}) {
	return judgeLicence(list, { name, version, manifest });
}

/** Judges a manifest whose only field is `license: expression`. */
function judgeExpression(expression, options) {
	return judge({ license: expression }, options);
}

describe('the licence expression parser', () => {
	test('builds a frozen tree with AND tighter than OR', () => {
		const tree = parseLicenceExpression('MIT OR ISC AND WTFPL');
		const expected = {
			kind: 'or',
			left: { kind: 'licence', id: 'MIT' },
			right: { kind: 'and', left: { kind: 'licence', id: 'ISC' }, right: { kind: 'licence', id: 'WTFPL' } }
		};
		assert.deepEqual(tree, expected);
		assert.ok(Object.isFrozen(tree) && Object.isFrozen(tree.right) && Object.isFrozen(tree.right.left));
	});

	test('reads +, WITH, LicenseRef and parentheses', () => {
		const withTree = parseLicenceExpression('Apache-2.0 WITH LLVM-exception');
		assert.deepEqual(withTree, { kind: 'with', licence: { kind: 'licence', id: 'Apache-2.0' }, exception: 'LLVM-exception' });
		assert.deepEqual(parseLicenceExpression('Apache-2.0+'), { kind: 'plus', id: 'Apache-2.0' });
		assert.deepEqual(parseLicenceExpression('LicenseRef-x'), { kind: 'ref', id: 'LicenseRef-x' });
		assert.deepEqual(parseLicenceExpression('(MIT)'), { kind: 'licence', id: 'MIT' });
		assert.notEqual(parseLicenceExpression('(MIT OR ISC) AND (Apache-2.0)'), null);
	});

	const unreadable = {
		'lower-case or': 'MIT or ISC',
		'a trailing operator': 'MIT OR',
		'a leading operator': 'AND MIT',
		'an operator alone': 'OR',
		'an unclosed parenthesis': '(MIT',
		'an unopened parenthesis': 'MIT)',
		'empty parentheses': '()',
		'a trailing newline': 'MIT OR ISC\n',
		'a tab': 'MIT\tOR ISC',
		'a no-break space': 'MIT OR ISC',
		'a leading space': ' MIT',
		'a trailing space': 'MIT ',
		'two spaces': 'MIT  OR ISC',
		'a space inside parentheses': '( MIT )',
		'a parenthesis touching an operator': 'MIT OR(ISC)',
		'a DocumentRef': 'DocumentRef-spdx-tool-1.2:LicenseRef-x',
		'a bare DocumentRef id': 'DocumentRef-x',
		'+ after a LicenseRef': 'LicenseRef-x+',
		'an empty LicenseRef': 'LicenseRef-',
		'a LicenseRef with an exception': 'LicenseRef-x WITH LLVM-exception',
		'a LicenseRef as the exception': 'MIT WITH LicenseRef-x',
		'WITH after parentheses': '(MIT OR ISC) WITH LLVM-exception',
		'two WITHs': 'MIT WITH a WITH b',
		'a double +': 'MIT++',
		'a bare +': '+',
		'two ids side by side': 'MIT ISC',
		'free text': 'SEE LICENSE IN LICENSE',
		'a URL': 'https://example.com/licence',
		'the empty string': ''
	};
	for (const [label, text] of Object.entries(unreadable)) {
		test(`reads nothing from ${label}`, () => {
			assert.equal(parseLicenceExpression(text), null);
		});
	}

	test('256 characters parse and 257 are unread', () => {
		const at256 = `MIT OR ${'a'.repeat(249)}`;
		assert.equal(at256.length, 256);
		assert.notEqual(parseLicenceExpression(at256), null);
		assert.equal(parseLicenceExpression(`${at256}a`), null);
	});

	test('eight nested parentheses parse and nine are unread', () => {
		assert.notEqual(parseLicenceExpression(`${'('.repeat(8)}MIT${')'.repeat(8)}`), null);
		assert.equal(parseLicenceExpression(`${'('.repeat(9)}MIT${')'.repeat(9)}`), null);
	});

	test('returns null for any non-string input, without throwing', () => {
		const inputs = [undefined, null, 0, 1n, true, {}, [], ['MIT'], Symbol('MIT'), new String('MIT'), () => 'MIT'];
		for (const input of inputs) assert.equal(parseLicenceExpression(input), null);
	});

	test('reads constructor as a plain id, and nothing from __proto__ (no _ in an idstring)', () => {
		assert.deepEqual(parseLicenceExpression('constructor'), { kind: 'licence', id: 'constructor' });
		assert.equal(parseLicenceExpression('__proto__'), null);
	});
});

describe('the verdict: precedence', () => {
	test('MIT OR ISC AND WTFPL passes (AND binds tighter than OR)', () => {
		assert.deepEqual(judgeExpression('MIT OR ISC AND WTFPL'), { pass: true, via: 'allow', licence: 'MIT OR ISC AND WTFPL' });
	});

	test('WTFPL AND ISC OR MIT passes (AND binds tighter than OR)', () => {
		assert.equal(judgeExpression('WTFPL AND ISC OR MIT').pass, true);
	});

	test('Apache-2.0 WITH LLVM-exception OR MIT passes', () => {
		assert.equal(judgeExpression('Apache-2.0 WITH LLVM-exception OR MIT').pass, true);
	});

	test('MIT AND Apache-2.0 WITH LLVM-exception fails', () => {
		const verdict = judgeExpression('MIT AND Apache-2.0 WITH LLVM-exception');
		assert.equal(verdict.pass, false);
		assert.match(verdict.problem, /"Apache-2\.0 WITH LLVM-exception" is never allowed/);
	});

	test('AND fails when either side fails', () => {
		assert.equal(judgeExpression('MIT AND ISC').pass, true);
		assert.equal(judgeExpression('MIT AND WTFPL').pass, false);
		assert.equal(judgeExpression('WTFPL AND MIT').pass, false);
	});

	test('the verdict is frozen', () => {
		assert.ok(Object.isFrozen(judgeExpression('MIT')));
		assert.ok(Object.isFrozen(judgeExpression('WTFPL')));
	});
});

describe('the verdict: never passes alone', () => {
	const failing = {
		'Apache-2.0+': '"Apache-2.0+" is never allowed: an "or later" licence',
		'LicenseRef-x': '"LicenseRef-x" is never allowed: a LicenseRef',
		mit: '"mit" is not allowed',
		WTFPL: '"WTFPL" is not allowed',
		constructor: '"constructor" is not allowed',
		toString: '"toString" is not allowed',
		hasOwnProperty: '"hasOwnProperty" is not allowed'
	};
	for (const [expression, problem] of Object.entries(failing)) {
		test(`${expression} fails`, () => {
			assert.deepEqual(judgeExpression(expression), { pass: false, licence: expression, problem });
		});
	}

	const unreadable = {
		'MIT or ISC': 'MIT or ISC',
		'MIT OR': 'MIT OR',
		'the text __proto__': '__proto__',
		'(MIT': '(MIT',
		'a trailing newline': 'MIT OR ISC\n',
		'a tab': 'MIT\tOR ISC',
		'257 characters': `MIT OR ${'a'.repeat(250)}`,
		'nine nested levels': `${'('.repeat(9)}MIT${')'.repeat(9)}`
	};
	for (const [label, expression] of Object.entries(unreadable)) {
		test(`${label} is unknown`, () => {
			const verdict = judgeExpression(expression);
			assert.equal(verdict.pass, false);
			assert.equal(verdict.licence, expression);
			assert.equal(verdict.problem, `${JSON.stringify(expression)} is not an SPDX expression this rule can read`);
		});
	}

	test('256 characters and eight nested levels still pass', () => {
		assert.equal(judgeExpression(`MIT OR ${'a'.repeat(249)}`).pass, true);
		assert.equal(judgeExpression(`${'('.repeat(8)}MIT${')'.repeat(8)}`).pass, true);
	});

	test('a compound failure names each failing licence', () => {
		const verdict = judgeExpression('WTFPL OR Apache-2.0+');
		const problem = '"WTFPL OR Apache-2.0+" is not met: "WTFPL" is not allowed; "Apache-2.0+" is never allowed: an "or later" licence';
		assert.equal(verdict.problem, problem);
	});

	test('a plain allowed id passes via allow', () => {
		assert.deepEqual(judgeExpression('MIT'), { pass: true, via: 'allow', licence: 'MIT' });
	});
});

describe('the verdict: unknown licences', () => {
	const unknown = {
		'no field': [{}, '(no licence field)', 'no licence field'],
		'a null field': [{ license: null }, 'null', 'the licence field is not a string (null)'],
		'a {type, url} field': [
			{ license: { type: 'MIT', url: 'https://example.com' } },
			'{"type":"MIT","url":"https://example.com"}',
			'the licence field is not a string ({"type":"MIT","url":"https://example.com"})'
		],
		'an array field': [{ license: ['MIT'] }, '["MIT"]', 'the licence field is not a string (["MIT"])'],
		'only a legacy licenses list': [{ licenses: [{ type: 'MIT' }] }, '(no licence field)', 'no licence field, only a legacy "licenses" list'],
		'UNLICENSED': [{ license: 'UNLICENSED' }, 'UNLICENSED', '"UNLICENSED" is not allowed'],
		'SEE LICENSE IN x': [{ license: 'SEE LICENSE IN x' }, 'SEE LICENSE IN x', '"SEE LICENSE IN x" is not an SPDX expression this rule can read']
	};
	for (const [label, [manifest, licence, problem]] of Object.entries(unknown)) {
		test(`${label} fails`, () => {
			assert.deepEqual(judge(manifest), { pass: false, licence, problem });
		});
	}

	test('an unreadable licence names its value in the finding', () => {
		const license = 'https://www.highcharts.com/license';
		const verdict = judge({ license }, { name: 'highcharts', version: '11.4.8' });
		const problem = '"https://www.highcharts.com/license" is not an SPDX expression this rule can read';
		assert.deepEqual(verdict, { pass: false, licence: license, problem });
	});

	test('a non-string licence names its value in the finding', () => {
		const license = { type: 'MIT', url: 'https://github.com/kriskowal/q/raw/master/LICENSE' };
		const verdict = judge({ license }, { name: 'q', version: '1.5.1' });
		const shown = '{"type":"MIT","url":"https://github.com/kriskowal/q/raw/master/LICENSE"}';
		const problem = `the licence field is not a string (${shown})`;
		assert.deepEqual(verdict, { pass: false, licence: shown, problem });
	});

	test('a licence inherited through the prototype counts as missing', () => {
		Object.prototype.license = 'MIT';
		try {
			assert.deepEqual(judge({}), { pass: false, licence: '(no licence field)', problem: 'no licence field' });
			assert.equal(judge({}, { name: 'svelte-toolbelt', version: '0.10.6' }).via, 'exception');
		} finally {
			delete Object.prototype.license;
		}
	});

	test('a manifest with another prototype is refused', () => {
		const manifest = Object.create({ license: 'MIT' });
		assert.deepEqual(judge(manifest), { pass: false, licence: '(no licence field)', problem: 'the registry manifest is not an object' });
	});

	test('a manifest that is not an object fails', () => {
		assert.equal(judge(undefined).pass, false);
		assert.equal(judge('MIT').pass, false);
	});
});

describe('the verdict: scoped licences', () => {
	test('MPL-2.0 passes for axe-core via its scoped entry', () => {
		assert.deepEqual(judgeExpression('MPL-2.0', { name: 'axe-core' }), { pass: true, via: 'scoped', licence: 'MPL-2.0' });
	});

	test('MPL-2.0 fails for a package its scoped entry does not name', () => {
		const verdict = judgeExpression('MPL-2.0', { name: 'not-axe' });
		assert.deepEqual(verdict, { pass: false, licence: 'MPL-2.0', problem: '"MPL-2.0" is allowed only for the packages its scoped entry names' });
	});

	test('a scoped licence AND an allowed one passes via scoped', () => {
		assert.equal(judgeExpression('MPL-2.0 AND MIT', { name: 'axe-core' }).via, 'scoped');
	});

	test("OFL-1.1 fails for @fortawesome/fontawesome-free's (CC-BY-4.0 AND OFL-1.1 AND MIT)", () => {
		const expression = '(CC-BY-4.0 AND OFL-1.1 AND MIT)';
		const verdict = judgeExpression(expression, { name: '@fortawesome/fontawesome-free' });
		assert.equal(verdict.pass, false);
		assert.equal(verdict.problem, `${JSON.stringify(expression)} is not met: "CC-BY-4.0" is not allowed`);
		const without = loadLicenceList(canonical(fixtureDocument({ scoped: [scopedEntry()] })), { today: TODAY });
		const unscoped = judgeExpression(expression, { name: '@fortawesome/fontawesome-free', list: without });
		assert.match(unscoped.problem, /"OFL-1\.1" is not allowed/);
	});

	test('isScopedPackage names exactly the scoped packages', () => {
		assert.equal(isScopedPackage(LIST, 'axe-core'), true);
		assert.equal(isScopedPackage(LIST, '@fontsource-variable/inter'), true);
		assert.equal(isScopedPackage(LIST, 'not-axe'), false);
		assert.equal(isScopedPackage(LIST, 'constructor'), false);
	});
});

describe('the verdict: exceptions', () => {
	test('an exception with registry null admits a version with no licence field', () => {
		const verdict = judge({}, { name: 'svelte-toolbelt', version: '0.10.6' });
		assert.deepEqual(verdict, { pass: true, via: 'exception', licence: '(no licence field)' });
	});

	test('another version is not admitted', () => {
		const verdict = judge({}, { name: 'svelte-toolbelt', version: '0.10.7' });
		assert.deepEqual(verdict, { pass: false, licence: '(no licence field)', problem: 'no licence field' });
	});

	test('a matching registry string is admitted', () => {
		const verdict = judge({ license: 'SEE LICENSE IN LICENSE' }, { name: 'odd-pkg', version: '1.2.3' });
		assert.equal(verdict.via, 'exception');
	});

	test('a changed registry string is not admitted', () => {
		const verdict = judge({ license: 'SEE LICENSE IN LICENCE.md' }, { name: 'odd-pkg', version: '1.2.3' });
		assert.equal(verdict.pass, false);
		assert.match(verdict.problem, /^"SEE LICENSE IN LICENCE\.md" is not an SPDX expression this rule can read \(the exception for odd-pkg@1\.2\.3 names registry "SEE LICENSE IN LICENSE"\)$/);
	});

	test('registry null matches only a missing field, not an explicit null or another non-string', () => {
		for (const license of [null, { type: 'MIT' }, ['MIT']]) {
			const verdict = judge({ license }, { name: 'svelte-toolbelt', version: '0.10.6' });
			assert.equal(verdict.pass, false, JSON.stringify(license));
		}
	});

	test('a string registry does not match a missing field', () => {
		assert.equal(judge({}, { name: 'odd-pkg', version: '1.2.3' }).pass, false);
	});

	test('a LicenseRef exception admits its version', () => {
		assert.equal(judge({ license: 'SEE LICENSE IN LICENCE.txt' }, { name: 'ref-pkg', version: '2.0.0' }).via, 'exception');
	});
});

describe('the verdict: bundled code', () => {
	const failing = {
		'a non-empty array': ['left-pad'],
		true: true,
		'an object': { 'left-pad': '1.3.0' },
		'a string': 'left-pad',
		null: null,
		'a number': 1
	};
	for (const field of ['bundleDependencies', 'bundledDependencies']) {
		for (const [label, value] of Object.entries(failing)) {
			test(`${field} as ${label} fails, even with a matching exception`, () => {
				const verdict = judge({ [field]: value }, { name: 'svelte-toolbelt', version: '0.10.6' });
				assert.equal(verdict.pass, false);
				assert.match(verdict.problem, new RegExp(`^bundles packages no lockfile lists \\("${field}" is `));
				assert.equal(judge({ license: 'MIT', [field]: value }).pass, false);
			});
		}
		for (const [label, value] of Object.entries({ false: false, '[]': [] })) {
			test(`${field} as ${label} passes`, () => {
				assert.equal(judge({ license: 'MIT', [field]: value }).pass, true);
				assert.equal(judge({ [field]: value }, { name: 'svelte-toolbelt', version: '0.10.6' }).pass, true);
			});
		}
	}

	test('no bundle field passes', () => {
		assert.equal(judge({ license: 'MIT' }).pass, true);
	});
});

describe('the list loader', () => {
	test('loads the fixture into Sets and Maps, frozen', () => {
		assert.ok(Object.isFrozen(LIST));
		assert.deepEqual([...LIST.allow], ['Apache-2.0', 'ISC', 'MIT']);
		assert.deepEqual([...LIST.scoped.keys()], ['MPL-2.0', 'OFL-1.1']);
		assert.deepEqual([...LIST.exceptions.keys()], ['svelte-toolbelt@0.10.6', 'odd-pkg@1.2.3', 'ref-pkg@2.0.0']);
		assert.ok(Object.isFrozen(LIST.exceptions.get('svelte-toolbelt@0.10.6')));
	});

	test('NEVER_ALLOW is exactly MPL-2.0 and OFL-1.1, frozen', () => {
		assert.deepEqual(NEVER_ALLOW, ['MPL-2.0', 'OFL-1.1']);
		assert.ok(Object.isFrozen(NEVER_ALLOW));
	});

	test('accepts exception terms that pass only through the package\'s scoped entry', () => {
		const document = fixtureDocument({ exceptions: [exceptionEntry({ package: 'axe-core@4.0.0', terms: 'MPL-2.0' })] });
		assert.doesNotThrow(() => loadLicenceList(canonical(document), { today: TODAY }));
	});

	test('accepts a readOn of today', () => {
		assert.doesNotThrow(() => loadLicenceList(canonical(fixtureDocument()), { today: TODAY }));
	});

	test('refuses a today that is not a real date', () => {
		assert.throws(() => loadLicenceList(canonical(fixtureDocument()), { today: '2026-02-30' }), /today "2026-02-30" is not a real date/);
		assert.throws(() => loadLicenceList(canonical(fixtureDocument()), {}), /today undefined is not a YYYY-MM-DD date/);
	});

	const base = canonical(fixtureDocument());
	const withDuplicateKey = base.replace('"readBy": "Fixture",', '"readBy": "Fixture",\n      "readBy": "Fixture",');
	const oversized = canonical(fixtureDocument({ scoped: [scopedEntry({ reason: 'é'.repeat(33_000) })] }));
	const refused = {
		'more than 64 KiB of UTF-8': [oversized, /bytes, over 64 KiB/],
		'text that is not JSON': ['{"allow": [', /not valid JSON/],
		'a one-line layout': [`${JSON.stringify(fixtureDocument())}\n`, /canonical form/],
		'no trailing newline': [base.trimEnd(), /canonical form/],
		'a duplicate key': [withDuplicateKey, /canonical form/],
		'a top level that is an array': [canonical([]), /top level is not an object/],
		'a missing top-level key': [canonical({ allow: [], scoped: [] }), /top level must be exactly/],
		'an extra top-level key': [canonical({ ...fixtureDocument(), note: 'x' }), /top level must be exactly/],
		'allow that is not an array': [canonical(fixtureDocument({ allow: 'MIT' })), /allow is not an array/],
		'a non-string id in allow': [canonical(fixtureDocument({ allow: [1] })), /allow\[0\]: 1 is not a plain licence id/],
		'MIT+ in allow': [canonical(fixtureDocument({ allow: ['MIT+'] })), /allow\[0\]: "MIT\+" is not a plain licence id/],
		'an expression in allow': [canonical(fixtureDocument({ allow: ['MIT OR ISC'] })), /not a plain licence id/],
		'an operator in allow': [canonical(fixtureDocument({ allow: ['AND'] })), /an operator/],
		'a lower-case operator in allow': [canonical(fixtureDocument({ allow: ['with'] })), /an operator/],
		'a LicenseRef in allow': [canonical(fixtureDocument({ allow: ['LicenseRef-x'] })), /a LicenseRef or DocumentRef/],
		'a DocumentRef in allow': [canonical(fixtureDocument({ allow: ['DocumentRef-x'] })), /a LicenseRef or DocumentRef/],
		'an unsorted allow': [canonical(fixtureDocument({ allow: ['MIT', 'ISC'] })), /allow: "ISC" is out of order or repeated/],
		'a repeated allow id': [canonical(fixtureDocument({ allow: ['ISC', 'MIT', 'MIT'] })), /allow: "MIT" is out of order or repeated/],
		'MPL-2.0 in allow': [canonical(fixtureDocument({ allow: ['Apache-2.0', 'ISC', 'MIT', 'MPL-2.0'] })), /"MPL-2\.0" is never allowed/],
		'OFL-1.1 in allow': [canonical(fixtureDocument({ allow: ['Apache-2.0', 'ISC', 'MIT', 'OFL-1.1'] })), /"OFL-1\.1" is never allowed/],
		'mpl-2.0 in allow': [canonical(fixtureDocument({ allow: ['Apache-2.0', 'ISC', 'MIT', 'mpl-2.0'] })), /"mpl-2\.0" is never allowed \(MPL-2\.0\)/],
		'scoped that is not an array': [canonical(fixtureDocument({ scoped: {} })), /scoped is not an array/],
		'a scoped entry with an extra key': [canonical(fixtureDocument({ scoped: [scopedEntry({ note: 'x' })] })), /scoped\[0\]: unexpected "note"/],
		'a scoped entry missing a key': [
			canonical(fixtureDocument({ scoped: [(({ readBy, ...rest }) => rest)(scopedEntry())] })),
			/scoped\[0\]: missing readBy/
		],
		'a scoped licence that is in allow': [canonical(fixtureDocument({ scoped: [scopedEntry({ licence: 'MIT' })] })), /licence "MIT" is already in allow/],
		'a scoped licence that is not a plain id': [
			canonical(fixtureDocument({ scoped: [scopedEntry({ licence: 'MPL-2.0+' })] })),
			/licence "MPL-2\.0\+" is not a plain licence id/
		],
		'empty scoped packages': [canonical(fixtureDocument({ scoped: [scopedEntry({ packages: [] })] })), /packages is not a non-empty array/],
		'unsorted scoped packages': [
			canonical(fixtureDocument({ scoped: [scopedEntry({ packages: ['axe-core', '@axe-core/playwright'] })] })),
			/scoped\[0\]\.packages: "@axe-core\/playwright" is out of order/
		],
		'a scoped package failing the naming rule': [
			canonical(fixtureDocument({ scoped: [scopedEntry({ packages: ['_axe'] })] })),
			/scoped\[0\]\.packages\[0\]: package name "_axe": starts with \. or _/
		],
		'a scoped package pattern': [
			canonical(fixtureDocument({ scoped: [scopedEntry({ packages: ['lightningcss*'] })] })),
			/package name "lightningcss\*" is a pattern/
		],
		'a (licence, package) pair in two entries': [
			canonical(fixtureDocument({ scoped: [scopedEntry(), scopedEntry({ packages: ['axe-core'] })] })),
			/scoped\[1\]: a second scoped entry for MPL-2\.0 names "axe-core"/
		],
		'an empty scoped reason': [canonical(fixtureDocument({ scoped: [scopedEntry({ reason: '' })] })), /scoped\[0\]: reason is empty/],
		'a blank scoped readBy': [canonical(fixtureDocument({ scoped: [scopedEntry({ readBy: ' ' })] })), /scoped\[0\]: readBy is empty/],
		'a scoped readOn tomorrow': [
			canonical(fixtureDocument({ scoped: [scopedEntry({ readOn: '2026-10-27' })] })),
			/readOn "2026-10-27" is after today \(2026-10-26\)/
		],
		'a scoped readOn of 2026-02-30': [canonical(fixtureDocument({ scoped: [scopedEntry({ readOn: '2026-02-30' })] })), /readOn "2026-02-30" is not a real date/],
		'exceptions that is not an array': [canonical(fixtureDocument({ exceptions: {} })), /exceptions is not an array/],
		'an exception with an extra key': [canonical(fixtureDocument({ exceptions: [exceptionEntry({ note: 'x' })] })), /exceptions\[0\]: unexpected "note"/],
		'an exception missing a key': [
			canonical(fixtureDocument({ exceptions: [(({ terms, ...rest }) => rest)(exceptionEntry())] })),
			/exceptions\[0\]: missing terms/
		],
		'an exception package with a range': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ package: 'svelte-toolbelt@^0.10.6' })] })),
			/is not an exact name@x\.y\.z/
		],
		'an exception package without a version': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ package: 'svelte-toolbelt' })] })),
			/is not an exact name@x\.y\.z/
		],
		'an exception package name failing the rule': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ package: '_x@1.0.0' })] })),
			/package name "_x": starts with \. or _/
		],
		'an exception registry that is a number': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ registry: 1 })] })),
			/registry is neither a string nor null/
		],
		'a readable registry, GPL-3.0-only, with LicenseRef terms': [
			canonical(
				fixtureDocument({
					exceptions: [exceptionEntry({ package: 'x@1.0.0', registry: 'GPL-3.0-only', terms: 'LicenseRef-y', textSha256: HASH })]
				})
			),
			/registry "GPL-3\.0-only" is an expression the rule can read/
		],
		'a readable registry, MIT': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ registry: 'MIT' })] })),
			/registry "MIT" is an expression the rule can read/
		],
		'a readable registry, UNLICENSED': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ registry: 'UNLICENSED' })] })),
			/registry "UNLICENSED" is an expression the rule can read/
		],
		'unreadable terms': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ terms: 'SEE LICENSE IN LICENSE' })] })),
			/terms "SEE LICENSE IN LICENSE" is not an SPDX expression/
		],
		'terms GPL-3.0-only, which the list does not allow': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ terms: 'GPL-3.0-only' })] })),
			/terms "GPL-3\.0-only" does not pass the list for svelte-toolbelt/
		],
		'scoped terms for a package the scope does not name': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ terms: 'MPL-2.0' })] })),
			/terms "MPL-2\.0" does not pass the list for svelte-toolbelt/
		],
		'a LicenseRef without a hash': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ terms: 'LicenseRef-x' })] })),
			/a LicenseRef in terms needs textSha256/
		],
		'a hash that is not lower-case hex': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ terms: 'LicenseRef-x', textSha256: 'A'.repeat(64) })] })),
			/a LicenseRef in terms needs textSha256/
		],
		'a hash without a LicenseRef': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ textSha256: HASH })] })),
			/textSha256 is only for terms holding a LicenseRef/
		],
		'a package with two exceptions': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry(), exceptionEntry({ reason: 'again' })] })),
			/exceptions\[1\]: a second exception for svelte-toolbelt@0\.10\.6/
		],
		'an empty exception reason': [canonical(fixtureDocument({ exceptions: [exceptionEntry({ reason: '' })] })), /exceptions\[0\]: reason is empty/],
		'a blank exception readBy': [canonical(fixtureDocument({ exceptions: [exceptionEntry({ readBy: '\t' })] })), /exceptions\[0\]: readBy is empty/],
		'an exception readOn tomorrow': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ readOn: '2026-10-27' })] })),
			/exceptions\[0\]: readOn "2026-10-27" is after today/
		],
		'an exception readOn of 2026-02-30': [
			canonical(fixtureDocument({ exceptions: [exceptionEntry({ readOn: '2026-02-30' })] })),
			/exceptions\[0\]: readOn "2026-02-30" is not a real date/
		]
	};
	for (const [label, [text, message]] of Object.entries(refused)) {
		test(`refuses ${label}`, () => {
			assert.throws(
				() => loadLicenceList(text, { today: TODAY }),
				(error) => error instanceof GuardError && message.test(error.message)
			);
		});
	}

	test('the oversized fixture is under 64 Ki characters, so the limit counts bytes', () => {
		assert.ok(oversized.length < 65_536);
	});
});

describe('the real list', () => {
	const text = fs.readFileSync(REAL_LIST_FILE, 'utf8');
	const today = new Date().toISOString().slice(0, 10);

	test('npm-licences.json loads with today\'s UTC date', () => {
		assert.doesNotThrow(() => loadLicenceList(text, { today }));
	});

	test('npm-licences.json is canonical', () => {
		assert.equal(`${JSON.stringify(JSON.parse(text), null, 2)}\n`, text);
	});

	test('npm-licences.json allows no NEVER_ALLOW id', () => {
		const list = loadLicenceList(text, { today });
		const never = NEVER_ALLOW.map((id) => id.toLowerCase());
		assert.deepEqual([...list.allow].filter((id) => never.includes(id.toLowerCase())), []);
	});

	test('every exception key in npm-licences.json is an exact name@x.y.z', () => {
		const list = loadLicenceList(text, { today });
		assert.ok(list.exceptions.size > 0);
		for (const key of list.exceptions.keys()) {
			const at = key.lastIndexOf('@');
			assert.ok(at > 0, key);
			assert.equal(packageNameProblem(key.slice(0, at)), undefined, key);
			assert.match(key.slice(at + 1), /^\d+\.\d+\.\d+$/, key);
		}
	});
});

describe('packageNameProblem', () => {
	test('passes npm names and refuses the rest', () => {
		assert.equal(packageNameProblem('left-pad'), undefined);
		assert.equal(packageNameProblem('@scope/pkg'), undefined);
		assert.equal(packageNameProblem('_pkg'), 'starts with . or _');
		assert.equal(packageNameProblem('@scope'), 'a scoped name is not @scope/name');
		assert.equal(packageNameProblem('a b'), 'characters that are not URL-safe');
		assert.equal(packageNameProblem('node_modules'), 'a reserved name');
		assert.equal(packageNameProblem(''), 'empty');
	});
});

describe('the imports', () => {
	/** Every module specifier `source` imports or re-exports, static or dynamic. */
	function specifiers(source) {
		return [...source.matchAll(/\b(?:from|import)\s*\(?\s*(['"])([^'"]+)\1/g)].map((match) => match[2]);
	}

	test('licences.mjs imports only install-check.mjs, and reads no file or network', () => {
		const source = fs.readFileSync(MODULE_FILE, 'utf8');
		assert.doesNotMatch(source, /\bimport\s*\(\s*[^'"\s]/, 'a computed dynamic import');
		assert.doesNotMatch(source, /\bcreateRequire\b|\brequire\s*\(/, 'a CommonJS require');
		assert.doesNotMatch(source, /\bfetch\s*\(|\bprocess\./, 'a fetch or a process read');
		const found = specifiers(source);
		assert.ok(found.length > 0, 'licences.mjs has imports');
		for (const specifier of found) {
			assert.ok(specifier.startsWith('./'), `a package or built-in import ${specifier}`);
			const target = path.relative(FRONTEND, path.resolve(FRONTEND, specifier));
			assert.ok(LICENCES_IMPORTS.includes(target), `licences.mjs imports ${specifier}, outside ${LICENCES_IMPORTS.join(', ')}`);
		}
	});
});

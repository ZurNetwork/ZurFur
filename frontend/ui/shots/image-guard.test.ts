// The image guard: each refusal on its own, all of them at once, and a clean pass.
//   node --test shots/image-guard.test.ts

import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { imageGuardRefusals, readImageFacts, type ImageFacts } from './image-guard.ts';

const digest = 'cf0daee9b994042e011bc29f20cdff1a9f682a039b43fcd738f7d8a9d3bcd9d6';
const pin = `mcr.microsoft.com/playwright:v1.61.1-noble@sha256:${digest}`;
const marker = JSON.stringify({
	driverVersion: '1.61.1',
	dockerImageName: 'mcr.microsoft.com/playwright:v1.61.1-noble'
});

/** The facts inside the pinned image. */
const inside: ImageFacts = { pin, marker, installed: '1.61.1' };

/** The refusals for `inside` with `change` applied. */
function refusalsWith(change: Partial<ImageFacts>): readonly string[] {
	const facts: ImageFacts = { ...inside, ...change };
	return imageGuardRefusals(facts);
}

describe('the image guard', () => {
	test('passes inside the pinned image', () => {
		assert.deepEqual(imageGuardRefusals(inside), []);
	});

	const offPattern: readonly (readonly [string, string | undefined])[] = [
		['unset', undefined],
		['empty', ''],
		['no digest', 'mcr.microsoft.com/playwright:v1.61.1-noble'],
		['a short digest', `mcr.microsoft.com/playwright:v1.61.1-noble@sha256:${digest.slice(1)}`],
		[
			'an upper-case digest',
			`mcr.microsoft.com/playwright:v1.61.1-noble@sha256:${digest.toUpperCase()}`
		],
		['another registry', `docker.io/playwright:v1.61.1-noble@sha256:${digest}`],
		['another distribution', `mcr.microsoft.com/playwright:v1.61.1-jammy@sha256:${digest}`],
		['a digest only', `mcr.microsoft.com/playwright@sha256:${digest}`],
		['a trailing space', `${pin} `]
	];

	for (const [name, raw] of offPattern) {
		test(`refuses a pin that is ${name}`, () => {
			const refusals = refusalsWith({ pin: raw });
			assert.equal(refusals.length, 1, refusals.join('\n'));
			assert.match(refusals[0] ?? '', /^ZURFUR_GALLERY_IMAGE is /);
		});
	}

	test('refuses a missing marker: outside the image', () => {
		assert.deepEqual(refusalsWith({ marker: undefined }), [
			'the image marker /ms-playwright/.docker-info is missing: not the Playwright image'
		]);
	});

	for (const text of [
		'',
		'not json',
		'[]',
		'{"driverVersion": 1}',
		'{"driverVersion": "1.61.1"}'
	]) {
		test(`refuses an unparsable marker ${JSON.stringify(text)}`, () => {
			assert.deepEqual(refusalsWith({ marker: text }), [
				'the image marker /ms-playwright/.docker-info is unparsable: not the Playwright image'
			]);
		});
	}

	test("refuses a marker whose driverVersion is not the pin's version", () => {
		const other = JSON.stringify({
			driverVersion: '1.61.0',
			dockerImageName: 'mcr.microsoft.com/playwright:v1.61.1-noble'
		});
		const refusals = refusalsWith({ marker: other });
		assert.equal(refusals.length, 1);
		assert.match(refusals[0] ?? '', /driverVersion "1\.61\.0" is not the pin's 1\.61\.1/);
	});

	test("refuses a marker whose dockerImageName is not the pin's name and tag", () => {
		const other = JSON.stringify({
			driverVersion: '1.61.1',
			dockerImageName: 'mcr.microsoft.com/playwright:v1.61.1-jammy'
		});
		const refusals = refusalsWith({ marker: other });
		assert.equal(refusals.length, 1);
		assert.match(refusals[0] ?? '', /dockerImageName "[^"]+jammy" is not the pin's/);
	});

	test("refuses an installed @playwright/test that is not the pin's version", () => {
		for (const installed of ['1.61.0', '1.62.0', undefined]) {
			const refusals = refusalsWith({ installed });
			assert.equal(refusals.length, 1, String(installed));
			assert.match(
				refusals[0] ?? '',
				/the installed @playwright\/test is .+, not the pin's 1\.61\.1/
			);
		}
	});

	test('names every refusal at once', () => {
		const refusals = imageGuardRefusals({ pin: undefined, marker: undefined, installed: '1.61.1' });
		assert.equal(refusals.length, 2);
	});

	test('reads the installed version on this machine', () => {
		assert.match(readImageFacts().installed ?? '', /^\d+\.\d+\.\d+$/);
	});
});

// The screenshot server: it serves its folder and nothing outside it, on loopback only.
//   node --test shots/serve.test.mjs

import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { connect } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { after, before, describe, test } from 'node:test';
import { contentTypeOf, parsePort, parseTarget, serve } from './serve.mjs';

const scratch = mkdtempSync(path.join(tmpdir(), 'ui-shots-serve-'));
const root = path.join(scratch, 'root');
const outside = path.join(scratch, 'outside');

/** @type {import('node:http').Server | undefined} */
let server;
let port = 0;

before(async () => {
	mkdirSync(path.join(root, 'assets'), { recursive: true });
	mkdirSync(outside, { recursive: true });
	writeFileSync(path.join(root, 'index.html'), '<!doctype html><title>index</title>');
	writeFileSync(path.join(root, 'assets', 'a.js'), 'export const a = 1;');
	writeFileSync(path.join(root, 'assets', 'font.WOFF2'), 'woff2');
	writeFileSync(path.join(outside, 'secret.txt'), 'secret');
	symlinkSync(path.join(outside, 'secret.txt'), path.join(root, 'leak.txt'));
	symlinkSync(outside, path.join(root, 'leak-folder'));
	server = await serve(root, 0);
	const address = server.address();
	assert.ok(address !== null && typeof address === 'object');
	port = address.port;
});

after(() => {
	server?.close();
	server?.closeAllConnections();
	rmSync(scratch, { recursive: true, force: true });
});

/**
 * Sends one raw HTTP/1.1 request, its target exactly as given, and reads the
 * status code and body of the answer.
 * @param {string} method
 * @param {string} target
 * @returns {Promise<{ status: number, headers: string, body: string }>}
 */
function request(method, target) {
	return new Promise((resolve, reject) => {
		const socket = connect({ host: '127.0.0.1', port }, () => {
			socket.write(`${method} ${target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n`);
		});
		let text = '';
		socket.setEncoding('utf8');
		socket.on('data', (chunk) => {
			text += chunk;
		});
		socket.on('error', reject);
		socket.on('end', () => {
			const [head = '', body = ''] = text.split('\r\n\r\n');
			const status = Number(/^HTTP\/1\.1 (\d{3})/.exec(head)?.[1]);
			resolve({ status, headers: head.toLowerCase(), body });
		});
	});
}

describe('the screenshot server', () => {
	test('serves a file with its content type and nosniff', async () => {
		const answer = await request('GET', '/assets/a.js?v=1#top');
		assert.equal(answer.status, 200);
		assert.equal(answer.body, 'export const a = 1;');
		assert.match(answer.headers, /content-type: text\/javascript; charset=utf-8/);
		assert.match(answer.headers, /x-content-type-options: nosniff/);
	});

	test('serves index.html at /', async () => {
		const answer = await request('GET', '/');
		assert.equal(answer.status, 200);
		assert.match(answer.body, /<title>index<\/title>/);
		assert.match(answer.headers, /content-type: text\/html/);
	});

	test('answers HEAD with headers and no body', async () => {
		const answer = await request('HEAD', '/index.html');
		assert.equal(answer.status, 200);
		assert.equal(answer.body, '');
		assert.match(answer.headers, /content-length: 35/);
	});

	test('answers 404 for a missing file and for a folder (no listing)', async () => {
		assert.equal((await request('GET', '/missing.js')).status, 404);
		assert.equal((await request('GET', '/assets/')).status, 404);
		assert.equal((await request('GET', '/assets')).status, 404);
	});

	test('answers 405 for any other method', async () => {
		for (const method of ['POST', 'PUT', 'DELETE', 'OPTIONS', 'TRACE']) {
			const answer = await request(method, '/index.html');
			assert.equal(answer.status, 405, method);
			assert.match(answer.headers, /allow: get, head/);
		}
	});

	/** @type {ReadonlyArray<readonly [string, string]>} */
	const traversals = [
		['a .. segment', '/../outside/secret.txt'],
		['a nested .. segment', '/assets/../../outside/secret.txt'],
		['%2e%2e', '/%2e%2e/outside/secret.txt'],
		['%2E%2E in upper case', '/%2E%2E/outside/secret.txt'],
		['..%2f', '/..%2foutside%2fsecret.txt'],
		['a backslash', '/..\\outside\\secret.txt'],
		['an encoded backslash', '/..%5coutside%5csecret.txt'],
		['an encoded NUL', '/index.html%00.js'],
		['an absolute-form target', 'http://127.0.0.1/../outside/secret.txt'],
		['an authority-form target', '127.0.0.1:80'],
		['a symlink to a file outside the folder', '/leak.txt'],
		['a symlink to a folder outside the folder', '/leak-folder/secret.txt']
	];

	for (const [name, target] of traversals) {
		test(`refuses ${name}`, async () => {
			const answer = await request('GET', target);
			assert.ok([400, 403].includes(answer.status), `${target}: ${String(answer.status)}`);
			assert.doesNotMatch(answer.body, /secret/);
		});
	}

	test('listens on 127.0.0.1 only', () => {
		const address = server?.address();
		assert.ok(address !== null && typeof address === 'object');
		assert.equal(address.address, '127.0.0.1');
		assert.equal(address.family, 'IPv4');
	});
});

describe('the server parts', () => {
	test('parseTarget decodes once and refuses what leaves the folder', () => {
		assert.deepEqual(parseTarget('/'), { kind: 'file', relative: 'index.html' });
		assert.deepEqual(parseTarget('/a%20b.js?x'), { kind: 'file', relative: 'a b.js' });
		assert.deepEqual(parseTarget('/%252e%252e/x'), { kind: 'file', relative: '%2e%2e/x' });
		assert.deepEqual(parseTarget('/%zz'), { kind: 'refused', status: 400 });
		assert.deepEqual(parseTarget('x'), { kind: 'refused', status: 400 });
		assert.deepEqual(parseTarget('/a/../b'), { kind: 'refused', status: 403 });
	});

	test('contentTypeOf reads the extension in any letter case', () => {
		assert.equal(contentTypeOf('a/FONT.WOFF2'), 'font/woff2');
		assert.equal(contentTypeOf('a.mjs'), 'text/javascript; charset=utf-8');
		assert.equal(contentTypeOf('a.bin'), 'application/octet-stream');
		assert.equal(contentTypeOf('README'), 'application/octet-stream');
	});

	test('parsePort takes a plain port from 1 to 65535', () => {
		assert.equal(parsePort('5199'), 5199);
		for (const raw of [undefined, '', '0', '65536', '-1', '0x10', ' 80', '80 ', '1e3']) {
			assert.equal(parsePort(raw), undefined, String(raw));
		}
	});
});

/**
 * A static file server for the screenshot run: it serves one folder on
 * 127.0.0.1 only, GET and HEAD only, and never anything outside the folder.
 * Node built-ins only. Run as `node serve.mjs <folder> <port>`.
 */
import { createReadStream, realpathSync, statSync } from 'node:fs';
import { realpath, stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

/** The one address the server binds. */
export const loopback = '127.0.0.1';

/** Content types by lower-case extension, for what Storybook's build holds. */
const contentTypes = new Map([
	['.html', 'text/html; charset=utf-8'],
	['.js', 'text/javascript; charset=utf-8'],
	['.mjs', 'text/javascript; charset=utf-8'],
	['.css', 'text/css; charset=utf-8'],
	['.json', 'application/json; charset=utf-8'],
	['.svg', 'image/svg+xml'],
	['.png', 'image/png'],
	['.jpg', 'image/jpeg'],
	['.webp', 'image/webp'],
	['.avif', 'image/avif'],
	['.gif', 'image/gif'],
	['.woff2', 'font/woff2'],
	['.woff', 'font/woff'],
	['.ttf', 'font/ttf'],
	['.ico', 'image/x-icon'],
	['.map', 'application/json; charset=utf-8'],
	['.txt', 'text/plain; charset=utf-8'],
	['.md', 'text/markdown; charset=utf-8']
]);

/**
 * The content type of a file, by its extension in any letter case.
 * @param {string} file
 * @returns {string}
 */
export function contentTypeOf(file) {
	return contentTypes.get(path.extname(file).toLowerCase()) ?? 'application/octet-stream';
}

/**
 * @typedef {{ readonly kind: 'file', readonly relative: string }
 *   | { readonly kind: 'refused', readonly status: 400 | 403 }} Target
 */

/**
 * Reads a request target into a path relative to the served folder: only the
 * origin form (a leading `/`), the query and fragment dropped, decoded once,
 * with no NUL, no backslash and no `..` segment. `/` names `index.html`.
 * @param {string} target
 * @returns {Target}
 */
export function parseTarget(target) {
	if (!target.startsWith('/')) return { kind: 'refused', status: 400 };
	const [rawPath = ''] = target.split(/[?#]/, 1);
	let decoded;
	try {
		decoded = decodeURIComponent(rawPath);
	} catch {
		return { kind: 'refused', status: 400 };
	}
	const segments = decoded.split('/');
	const isRefused =
		decoded.includes('\0') || decoded.includes('\\') || segments.some((part) => part === '..');
	if (isRefused) return { kind: 'refused', status: 403 };
	const relative = decoded === '/' ? 'index.html' : decoded.slice(1);
	return { kind: 'file', relative };
}

/**
 * True when `child` is `parent` or lies inside it; both are real paths.
 * @param {string} parent
 * @param {string} child
 * @returns {boolean}
 */
function isInside(parent, child) {
	return child === parent || child.startsWith(parent + path.sep);
}

/**
 * @typedef {{ readonly kind: 'file', readonly file: string, readonly size: number }
 *   | { readonly kind: 'status', readonly status: 400 | 403 | 404 }} Lookup
 */

/**
 * Finds the file a request target names under `root` (a real path): its real
 * path must stay inside `root`, and it must be a plain file, never a folder.
 * @param {string} root
 * @param {string} target
 * @returns {Promise<Lookup>}
 */
export async function lookup(root, target) {
	const parsed = parseTarget(target);
	if (parsed.kind === 'refused') return { kind: 'status', status: parsed.status };
	const joined = path.join(root, parsed.relative);
	if (!isInside(root, joined)) return { kind: 'status', status: 403 };
	let real;
	try {
		real = await realpath(joined);
	} catch {
		return { kind: 'status', status: 404 };
	}
	if (!isInside(root, real)) return { kind: 'status', status: 403 };
	const found = await stat(real).catch(() => undefined);
	if (found === undefined || !found.isFile()) return { kind: 'status', status: 404 };
	return { kind: 'file', file: real, size: found.size };
}

/** The headers every answer carries. */
const commonHeaders = { 'X-Content-Type-Options': 'nosniff', 'Cache-Control': 'no-store' };

/**
 * Makes the server for `folder` (not yet listening). The folder's real path is
 * read once, here.
 * @param {string} folder
 * @returns {import('node:http').Server}
 */
export function createStaticServer(folder) {
	const root = realpathSync(folder);
	if (!statSync(root).isDirectory()) throw new Error(`${folder}: not a folder`);
	return createServer((request, response) => {
		const method = request.method ?? '';
		if (method !== 'GET' && method !== 'HEAD') {
			response.writeHead(405, { ...commonHeaders, Allow: 'GET, HEAD' }).end();
			return;
		}
		void lookup(root, request.url ?? '').then((found) => {
			if (found.kind === 'status') {
				response.writeHead(found.status, commonHeaders).end();
				return;
			}
			response.writeHead(200, {
				...commonHeaders,
				'Content-Type': contentTypeOf(found.file),
				'Content-Length': String(found.size)
			});
			if (method === 'HEAD') {
				response.end();
				return;
			}
			createReadStream(found.file)
				.on('error', () => response.destroy())
				.pipe(response);
		});
	});
}

/**
 * Serves `folder` on 127.0.0.1 at `port` (0 picks a free one) and resolves
 * once the server listens.
 * @param {string} folder
 * @param {number} port
 * @returns {Promise<import('node:http').Server>}
 */
export function serve(folder, port) {
	const server = createStaticServer(folder);
	return new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen({ host: loopback, port }, () => {
			server.off('error', reject);
			resolve(server);
		});
	});
}

/**
 * Reads a TCP port: a plain decimal integer from 1 to 65535, else undefined.
 * @param {string | undefined} raw
 * @returns {number | undefined}
 */
export function parsePort(raw) {
	if (raw === undefined || !/^[0-9]{1,5}$/.test(raw)) return undefined;
	const port = Number(raw);
	return port >= 1 && port <= 65535 ? port : undefined;
}

const entry = process.argv[1];
const isRunDirectly = entry !== undefined && path.resolve(entry) === fileURLToPath(import.meta.url);

if (isRunDirectly) {
	const [folder, rawPort, ...extra] = process.argv.slice(2);
	const port = parsePort(rawPort);
	if (folder === undefined || port === undefined || extra.length > 0) {
		process.stderr.write('usage: node serve.mjs <folder> <port>\n');
		process.exit(2);
	}
	const server = await serve(folder, port);
	process.stdout.write(`serving ${folder} at http://${loopback}:${String(port)}\n`);
	for (const signal of ['SIGINT', 'SIGTERM']) {
		process.on(signal, () => {
			server.close();
			server.closeAllConnections();
		});
	}
}

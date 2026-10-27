/**
 * The gallery's one port rule: `ZURFUR_GALLERY_PORT`, or 5190. Run as
 * `node gallery-port.ts`, it prints the port. Node built-ins only.
 */
import { argv, env, stdout } from 'node:process';
import { fileURLToPath } from 'node:url';

const fallbackGalleryPort = 5190;

/**
 * Reads the gallery's port from the raw `ZURFUR_GALLERY_PORT` text.
 * Only a plain decimal integer from 1 to 65535 counts; anything else, unset
 * included, falls back to 5190.
 */
export function galleryPort(raw: string | undefined): number {
	if (raw === undefined || !/^[0-9]+$/.test(raw)) return fallbackGalleryPort;
	const port = Number(raw);
	const isTcpPort = Number.isInteger(port) && port >= 1 && port <= 65535;
	return isTcpPort ? port : fallbackGalleryPort;
}

const isRunDirectly = argv[1] === fileURLToPath(import.meta.url);

if (isRunDirectly) stdout.write(`${String(galleryPort(env.ZURFUR_GALLERY_PORT))}\n`);

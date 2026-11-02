/**
 * The screenshot run's guard: pictures are made only inside the pinned
 * Playwright image. It refuses unless `ZURFUR_GALLERY_IMAGE` holds the pin,
 * the image's own marker names the pin's version and image, and the installed
 * `@playwright/test` is that version too. It guards against accidents only.
 * Node built-ins only.
 */
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

/** The pin's shape: the official image, its version tag on noble, and its digest. */
const pinPattern = /^(mcr\.microsoft\.com\/playwright:v(\d+\.\d+\.\d+)-noble)@sha256:[0-9a-f]{64}$/;

/** The marker the official Playwright image carries. */
export const markerPath = '/ms-playwright/.docker-info';

/** What the guard reads: each is undefined when it could not be read. */
export interface ImageFacts {
	/** `ZURFUR_GALLERY_IMAGE`: the pinned image, name, tag and digest. */
	readonly pin: string | undefined;
	/** The text of the image's marker file. */
	readonly marker: string | undefined;
	/** The installed `@playwright/test` version. */
	readonly installed: string | undefined;
}

interface Pin {
	readonly nameAndTag: string;
	readonly version: string;
}

interface Marker {
	readonly driverVersion: string;
	readonly dockerImageName: string;
}

function parsePin(raw: string | undefined): Pin | undefined {
	const match = raw === undefined ? undefined : pinPattern.exec(raw);
	const nameAndTag = match?.[1];
	const version = match?.[2];
	return nameAndTag === undefined || version === undefined ? undefined : { nameAndTag, version };
}

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

function isMarker(value: unknown): value is Marker {
	return (
		isFields(value) &&
		typeof value.driverVersion === 'string' &&
		typeof value.dockerImageName === 'string'
	);
}

function parseMarker(text: string | undefined): Marker | undefined {
	if (text === undefined) return undefined;
	try {
		const value: unknown = JSON.parse(text);
		return isMarker(value) ? value : undefined;
	} catch {
		return undefined;
	}
}

/** Every reason the run is not inside the pinned image; empty means it is. */
export function imageGuardRefusals(facts: ImageFacts): readonly string[] {
	const refusals: string[] = [];
	const pin = parsePin(facts.pin);
	if (pin === undefined) {
		refusals.push(
			`ZURFUR_GALLERY_IMAGE is ${JSON.stringify(facts.pin)}, not mcr.microsoft.com/playwright:v<version>-noble@sha256:<digest>`
		);
	}
	const marker = parseMarker(facts.marker);
	if (marker === undefined) {
		const why = facts.marker === undefined ? 'missing' : 'unparsable';
		refusals.push(`the image marker ${markerPath} is ${why}: not the Playwright image`);
	}
	if (pin !== undefined && marker !== undefined) {
		if (marker.driverVersion !== pin.version) {
			refusals.push(
				`the image marker's driverVersion ${JSON.stringify(marker.driverVersion)} is not the pin's ${pin.version}`
			);
		}
		if (marker.dockerImageName !== pin.nameAndTag) {
			refusals.push(
				`the image marker's dockerImageName ${JSON.stringify(marker.dockerImageName)} is not the pin's ${pin.nameAndTag}`
			);
		}
	}
	if (pin !== undefined && facts.installed !== pin.version) {
		refusals.push(
			`the installed @playwright/test is ${JSON.stringify(facts.installed)}, not the pin's ${pin.version}`
		);
	}
	return refusals;
}

function readText(file: string): string | undefined {
	try {
		return readFileSync(file, 'utf8');
	} catch {
		return undefined;
	}
}

function installedPlaywrightVersion(): string | undefined {
	const manifestPath = createRequire(import.meta.url).resolve('@playwright/test/package.json');
	const text = readText(manifestPath);
	const manifest: unknown = text === undefined ? undefined : JSON.parse(text);
	return isFields(manifest) && typeof manifest.version === 'string' ? manifest.version : undefined;
}

/** Reads the facts from this process and its file system. */
export function readImageFacts(): ImageFacts {
	let installed: string | undefined;
	try {
		installed = installedPlaywrightVersion();
	} catch {
		installed = undefined;
	}
	return { pin: process.env.ZURFUR_GALLERY_IMAGE, marker: readText(markerPath), installed };
}

/** Throws one error naming every refusal, unless the run is inside the pinned image. */
export function imageGuard(): void {
	const refusals = imageGuardRefusals(readImageFacts());
	if (refusals.length === 0) return;
	const lines = refusals.map((refusal) => `  - ${refusal}`).join('\n');
	throw new Error(
		`The screenshots run only inside the pinned Playwright image (run them through scripts/gallery-shots.sh):\n${lines}`
	);
}

/**
 * The pin on the page shell's one inline script, which sets the stored skin
 * before first paint: base64 of SHA-256 over the script element's exact text,
 * in the form a Content-Security-Policy `script-src` takes. Any edit to the
 * script fails its spec until this changes with it.
 */
export const BOOT_SCRIPT_HASH = 'sha256-pltaweSudpzi6vg0AZthPyUqXMCsZj4dqo7eyEVjETQ=';

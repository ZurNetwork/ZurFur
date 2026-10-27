import { configDefaults, defineConfig, type TestProjectInlineConfiguration } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { storybookTest } from '@storybook/addon-vitest/vitest-plugin';
import { emulateMedia } from './src/checks/media-command.ts';
import { galleryPort } from './gallery-port.ts';

const port = galleryPort(process.env.ZURFUR_GALLERY_PORT);

/** Storybook's config folder, and the address the story tests name for it. */
const storybookConfigDir = '.storybook';
const storybookUrl = `http://127.0.0.1:${String(port)}`;

/** The story setup file: the preview's annotations, the accessibility addon's first. */
const storybookSetupFile = '.storybook/vitest.setup.ts';

/**
 * Every story as a test in headless Chromium, in Paper dark and Lively. The
 * test panel runs this project alone, renamed after the config folder.
 */
const storiesProject = {
	extends: './vite.config.ts',
	server: { strictPort: false },
	plugins: [
		storybookTest({
			configDir: storybookConfigDir,
			storybookUrl,
			initialGlobals: { skin: 'dark', motion: 'lively' }
		})
	],
	test: {
		name: 'stories',
		browser: {
			enabled: true,
			provider: playwright(),
			instances: [{ browser: 'chromium', headless: true }]
		},
		setupFiles: [storybookSetupFile]
	}
} satisfies TestProjectInlineConfiguration;

/**
 * Every story again in Paper light with motion stopped, the page asking for
 * light and reduced motion too. It runs after the other projects, since both
 * story projects share the plugin's one cache folder.
 */
const storiesLightStillProject = {
	extends: './vite.config.ts',
	server: { strictPort: false },
	plugins: [
		storybookTest({
			configDir: storybookConfigDir,
			storybookUrl,
			initialGlobals: { skin: 'light', motion: 'still' }
		})
	],
	test: {
		name: 'stories-light-still',
		sequence: { groupOrder: 2 },
		browser: {
			enabled: true,
			provider: playwright({
				contextOptions: { reducedMotion: 'reduce', colorScheme: 'light' }
			}),
			instances: [{ browser: 'chromium', headless: true }]
		},
		setupFiles: [storybookSetupFile]
	}
} satisfies TestProjectInlineConfiguration;

/** The specs that emulate media features; they run alone, after every other project but the stories in Paper light. */
const mediaSpecs = [
	'src/checks/media.svelte.spec.ts',
	'src/stories/frame/FrameSkin.svelte.spec.ts'
];

export default defineConfig({
	// Both servers bind IPv4 loopback only, on a port that must be free: a busy
	// port is an error, never a silent move to the next one.
	server: { host: '127.0.0.1', port, strictPort: true },
	preview: { host: '127.0.0.1', port, strictPort: true },
	// Pre-bundle the controls' libraries up front: found mid-run, they make Vite
	// reload the browser specs, and a spec loaded twice fails.
	optimizeDeps: { include: ['bits-ui', '@internationalized/date'] },
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter({ strict: true }),
			alias: { $gallery: 'src/gallery' },
			typescript: {
				// svelte-check also reads Storybook's config and the port rule; paths are
				// relative to the generated .svelte-kit/tsconfig.json.
				config: (config) => {
					config.include.push('../.storybook/*.ts', '../gallery-port.ts');
				}
			}
		})
	],
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				// The browser tests' own server inherits `server` above, and with it
				// `strictPort`; let it move to the next free port instead, so two
				// workspaces can run their suites at once.
				server: { strictPort: false },
				test: {
					name: 'client',
					browser: {
						enabled: true,
						provider: playwright(),
						instances: [{ browser: 'chromium', headless: true }]
					},
					include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
					exclude: [...configDefaults.exclude, ...mediaSpecs]
				}
			},
			{
				extends: './vite.config.ts',
				server: { strictPort: false },
				test: {
					name: 'client-media',
					// Groups run from lowest to highest, so no other project runs beside
					// this one while it changes the page's emulated media.
					sequence: { groupOrder: 1 },
					browser: {
						enabled: true,
						provider: playwright(),
						instances: [{ browser: 'chromium', headless: true }],
						commands: { emulateMedia }
					},
					include: mediaSpecs
				}
			},

			storiesProject,
			// The panel runs one project, so Paper light with motion stopped runs from
			// the command line only.
			...(process.env.VITEST_STORYBOOK === 'true' ? [] : [storiesLightStillProject]),
			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});

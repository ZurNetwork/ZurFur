/**
 * The screenshot run: Playwright Test over the built Storybook, served on
 * loopback, inside the pinned Playwright image only. Project `stories` walks
 * every entry and holds the reference pictures; project `self` proves the
 * checks catch what they should.
 */
import { defineConfig } from '@playwright/test';
import { imageGuard } from './image-guard.ts';
import { shotsOrigin, shotsPort } from './watch.ts';

// Before anything else: outside the pinned image, no test runs.
imageGuard();

export default defineConfig({
	outputDir: 'output/results',
	preserveOutput: 'failures-only',
	reporter: [['list']],
	retries: 0,
	failOnFlakyTests: true,
	forbidOnly: true,
	fullyParallel: true,
	workers: 4,
	updateSnapshots: 'none',
	captureGitInfo: { commit: false, diff: false },
	expect: {
		toHaveScreenshot: {
			threshold: 0.2,
			maxDiffPixels: 0,
			animations: 'disabled',
			caret: 'hide',
			scale: 'css'
		}
	},
	use: {
		browserName: 'chromium',
		headless: true,
		baseURL: shotsOrigin,
		viewport: { width: 1280, height: 900 },
		deviceScaleFactor: 1,
		locale: 'en-GB',
		timezoneId: 'UTC',
		contextOptions: { reducedMotion: 'reduce' },
		serviceWorkers: 'block',
		trace: 'off',
		video: 'off',
		screenshot: 'off'
	},
	webServer: {
		command: `node serve.mjs ../storybook-static ${String(shotsPort)}`,
		cwd: import.meta.dirname,
		url: `${shotsOrigin}/index.json`,
		reuseExistingServer: false,
		stdout: 'ignore',
		stderr: 'pipe',
		timeout: 30_000
	},
	projects: [
		{
			name: 'stories',
			testDir: '.',
			testMatch: 'stories.pw.ts',
			snapshotPathTemplate: '__screenshots__/{arg}{ext}'
		},
		{
			name: 'self',
			testDir: 'self',
			testMatch: '*.pw.ts',
			// Always missing, so the missing-reference self-test fails as it should.
			snapshotPathTemplate: 'output/self-pictures/{testFileBaseName}/{arg}{ext}'
		}
	]
});

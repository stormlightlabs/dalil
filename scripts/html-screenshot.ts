import fs from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { chromium, type Browser } from 'playwright';

const REPO_ROOT = join(import.meta.dir, '..');
const REPORT_PATH = join(REPO_ROOT, 'assets', 'dalil-report.html');
const SCREENSHOT_PATH = join(REPO_ROOT, 'assets', 'dalil-report.png');
const FRAME_PATH = join(import.meta.dir, '.html-screenshot-frame.html');
const VIEWPORT = { width: 1600, height: 1040 };

async function generateReport(): Promise<void> {
	const process = Bun.spawn(['cargo', 'run', '--quiet', '--', '--no-cache', '--html', '.'], {
		cwd: REPO_ROOT,
		stdout: 'pipe',
		stderr: 'pipe'
	});

	const [html, stderr, exitCode] = await Promise.all([
		new Response(process.stdout).text(),
		new Response(process.stderr).text(),
		process.exited
	]);

	if (exitCode !== 0) {
		throw new Error(`dalil exited with status ${exitCode}\n${stderr.trim()}`);
	}

	if (!html.startsWith('<!doctype html>')) {
		throw new Error('dalil did not return an HTML report');
	}

	await fs.mkdir(dirname(REPORT_PATH), { recursive: true });
	await fs.writeFile(REPORT_PATH, html);
}

function browserFrame(reportUrl: string): string {
	return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Dalil report preview</title>
  <style>
    * {
      box-sizing: border-box;
    }

    html,
    body {
      width: 100%;
      height: 100%;
      margin: 0;
    }

    body {
      display: grid;
      padding: 56px;
      overflow: hidden;
      background:
        radial-gradient(circle at 18% 12%, rgb(239 89 111 / 20%), transparent 28%),
        #141415;
      place-items: center;
    }

    .browser {
      width: 100%;
      height: 100%;
      overflow: hidden;
      background: #fbf9f9;
      border: 1px solid rgb(255 255 255 / 18%);
      border-radius: 18px;
      box-shadow:
        0 32px 80px rgb(2 18 20 / 46%),
        0 4px 16px rgb(2 18 20 / 24%);
    }

    .browser__bar {
      display: grid;
      grid-template-columns: 1fr minmax(20rem, 42rem) 1fr;
      align-items: center;
      height: 54px;
      padding: 0 18px;
      background: #f2eeef;
      border-bottom: 1px solid #dfdbdd;
    }

    .browser__controls {
      display: flex;
      gap: 9px;
    }

    .browser__control {
      width: 12px;
      height: 12px;
      border-radius: 50%;
    }

    .browser__control:nth-child(1) {
      background: #ef596f;
    }

    .browser__control:nth-child(2) {
      background: #e8b650;
    }

    .browser__control:nth-child(3) {
      background: #61b98b;
    }

    .browser__address {
      overflow: hidden;
      padding: 8px 18px;
      color: #686367;
      background: #fbf9f9;
      border: 1px solid #dfdbdd;
      border-radius: 9px;
      font: 500 13px/1.2 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      text-align: center;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    iframe {
      display: block;
      width: 100%;
      height: calc(100% - 54px);
      border: 0;
    }
  </style>
</head>
<body>
  <main class="browser">
    <header class="browser__bar">
      <div class="browser__controls" aria-hidden="true">
        <span class="browser__control"></span>
        <span class="browser__control"></span>
        <span class="browser__control"></span>
      </div>
      <div class="browser__address">dalil report · mariners-astrolabe</div>
    </header>
    <iframe src="${reportUrl}" title="Generated Dalil report"></iframe>
  </main>
</body>
</html>`;
}

async function captureScreenshot(): Promise<void> {
	await fs.writeFile(FRAME_PATH, browserFrame(pathToFileURL(REPORT_PATH).href));

	let browser: Browser | undefined;

	try {
		browser = await chromium.launch({ headless: true, args: ['--allow-file-access-from-files'] });

		const page = await browser.newPage({ colorScheme: 'light', deviceScaleFactor: 1, viewport: VIEWPORT });
		await page.goto(pathToFileURL(FRAME_PATH).href, { waitUntil: 'load' });

		const report = page.frameLocator('iframe');

		await report.locator('#report-title').waitFor();
		await report.locator('html').evaluate('() => document.fonts.ready');
		await page.screenshot({ path: SCREENSHOT_PATH, fullPage: false });
	} finally {
		try {
			await browser?.close();
		} finally {
			await fs.rm(FRAME_PATH, { force: true });
		}
	}
}

await generateReport();
await captureScreenshot();

console.log(`HTML report: ${REPORT_PATH}`);
console.log(`README screenshot: ${SCREENSHOT_PATH}`);

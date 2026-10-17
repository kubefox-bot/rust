import { resolve } from 'node:path';
export const REPOSITORY = resolve(import.meta.dir, '../..');
export const BROWSER_TESTS = { shell: 'tests/shell.browser.test.ts', cleanup: 'tests/browser/cleanup.test.ts' } as const;

import { BROWSER_TESTS, REPOSITORY } from './browser-config';
const run = async ({ argv, cwd }: Readonly<{argv: string[];cwd: string}>) => {
  const child = Bun.spawn(argv,{cwd,stdin:'inherit',stdout:'inherit',stderr:'inherit'});
  const code = await child.exited;
  if(code!==0)throw new Error(`Browser verification command failed (${code})`);
};
const isTarget = (value: string): value is keyof typeof BROWSER_TESTS => Object.hasOwn(BROWSER_TESTS,value);
const selections = Bun.argv.slice(2);
const names = selections.length ? selections : Object.keys(BROWSER_TESTS);
const paths = names.map(name=>{if(!isTarget(name))throw new Error(`Unknown browser suite: ${name}`);return BROWSER_TESTS[name];});
await run({argv:['bun','run','web:build'],cwd:REPOSITORY});
await run({argv:['cargo','build','-p','hmail-cleaner-center','--example','browser_fixture','--locked'],cwd:REPOSITORY});
await run({argv:['bun','test',...paths],cwd:`${REPOSITORY}/apps/web`});

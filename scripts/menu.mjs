#!/usr/bin/env node
// Interactive monorepo menu — `pnpm dev`.
// Modular: each top-level group lives in scripts/menu/<group>.mjs
// exporting { title, items }. Item shapes:
//   { label, run }            run: shell string or (answer) => string
//   { label, ask, run }       prompt first, then run(answer)
//   { label, menu }           nested submenu (array of items)
//   { label, note }           print a note, no command
//
// Input is consumed line-by-line via async iteration (works on TTYs and
// pipes alike); a small state machine tracks menu stack / prompts.
import { spawn } from 'node:child_process';
import * as readline from 'node:readline/promises';
import { stdin as input, stdout as output } from 'node:process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SHELL = process.platform === 'darwin' ? '/bin/zsh' : '/bin/bash';

const groups = await Promise.all(
  ['palm', 'flood', 'core', 'labeling', 'mobile', 'quality'].map((g) => import(`./menu/${g}.mjs`)),
);

const rl = readline.createInterface({ input, output });

function render(title, items, isTop) {
  console.log(`\n=== ${title} ===`);
  items.forEach((it, i) => console.log(`  ${i + 1}. ${it.label}`));
  console.log(isTop ? '  q. quit' : '  0. back');
  process.stdout.write('> ');
}

function runCommand(cmd) {
  return new Promise((resolve) => {
    console.log(`\n$ ${cmd}\n`);
    const child = spawn(cmd, { shell: SHELL, cwd: ROOT, stdio: 'inherit' });
    child.on('exit', (code) => {
      console.log(`\n— exit ${code} —`);
      resolve();
    });
  });
}

async function main() {
  console.log('vision-perception — pick a project to run (step by step).');
  const stack = [{ title: 'monorepo', items: groups.map((g) => ({ label: g.title, menu: g.items })) }];
  let pendingAsk = null; // { item } awaiting a free-text answer
  let pausing = false; // waiting for Enter after a command
  render(stack[0].title, stack[0].items, true);

  for await (const raw of rl) {
    const ans = raw.trim();
    if (pausing) {
      pausing = false;
      const cur = stack[stack.length - 1];
      render(cur.title, cur.items, stack.length === 1);
      continue;
    }
    if (pendingAsk) {
      const item = pendingAsk;
      pendingAsk = null;
      if (!ans) {
        const cur = stack[stack.length - 1];
        render(cur.title, cur.items, stack.length === 1);
        continue;
      }
      const cmd = typeof item.run === 'function' ? item.run(ans) : `${item.run} ${ans}`;
      await runCommand(cmd);
      console.log('↵ Enter to continue');
      pausing = true;
      continue;
    }

    const cur = stack[stack.length - 1];
    const isTop = stack.length === 1;
    const low = ans.toLowerCase();
    if (low === 'q' || (!isTop && ans === '0')) {
      if (isTop) break;
      stack.pop();
      const now = stack[stack.length - 1];
      render(now.title, now.items, stack.length === 1);
      continue;
    }
    const item = cur.items[Number(ans) - 1];
    if (!item) {
      console.log('Invalid choice.');
      render(cur.title, cur.items, isTop);
      continue;
    }
    if (item.menu) {
      stack.push({ title: item.label, items: item.menu });
      render(item.label, item.menu, false);
    } else if (item.note) {
      console.log(`\n${item.note}`);
      console.log('↵ Enter to continue');
      pausing = true;
    } else if (item.run) {
      if (item.ask) {
        pendingAsk = item;
        process.stdout.write(`${item.ask}: `);
      } else {
        const cmd = typeof item.run === 'function' ? item.run() : item.run;
        await runCommand(cmd);
        console.log('↵ Enter to continue');
        pausing = true;
      }
    }
  }
  rl.close();
}

await main();

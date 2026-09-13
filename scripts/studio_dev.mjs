// scripts/studio_dev.mjs
// One-command CurvStudio developer setup with React hot reload:
//
//   node scripts/studio_dev.mjs [--api-port 8080] [--vite-port 3000] [--open]
//
// Spawns (a) the Python API + static server and (b) the Vite dev server whose
// /api + /data + /output calls proxy to (a). Edit TSX/CSS and the browser
// updates in place via React Fast Refresh — no rebuild, no manual refresh.
// Ctrl-C stops both. Zero npm dependencies (node child_process only).
import { spawn } from 'node:child_process';

const args = process.argv.slice(2);
function flag(name, fallback) {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
}
const apiPort = flag('--api-port', '8080');
const vitePort = flag('--vite-port', '3000');
const shouldOpen = args.includes('--open');

const children = [];
function run(label, cmd, cmdArgs, extraEnv = {}) {
  const child = spawn(cmd, cmdArgs, {
    stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env, ...extraEnv },
  });
  children.push(child);
  child.stdout.on('data', d => process.stdout.write(`[${label}] ${d}`));
  child.stderr.on('data', d => process.stderr.write(`[${label}] ${d}`));
  child.on('exit', (code, signal) => {
    console.log(`[${label}] exited (code=${code} signal=${signal}) — stopping dev setup.`);
    shutdown();
  });
  return child;
}

let shuttingDown = false;
function shutdown() {
  if (shuttingDown) return;
  shuttingDown = true;
  for (const child of children) {
    try {
      child.kill('SIGTERM');
    } catch {
      /* already gone */
    }
  }
  setTimeout(() => process.exit(0), 500).unref();
}
process.on('SIGINT', () => {
  console.log('\nStopping CurvStudio dev setup.');
  shutdown();
});
process.on('SIGTERM', shutdown);

run('api', 'python3', ['scripts/studio_server.py', '--port', String(apiPort)]);
run('vite', 'npm', ['run', 'dev', '--prefix', 'apps/annotation_studio', '--', '--port', String(vitePort)], {
  VITE_STUDIO_API: `http://127.0.0.1:${apiPort}`,
});

console.log(`CurvStudio dev setup starting:
  UI (hot reload): http://localhost:${vitePort}/
  API + static:    http://localhost:${apiPort}/apps/annotation_studio/index.html`);

if (shouldOpen) {
  const url = `http://localhost:${vitePort}/`;
  const openCmd = process.platform === 'darwin' ? 'open' : process.platform === 'win32' ? 'cmd' : 'xdg-open';
  const openArgs = process.platform === 'win32' ? ['/c', 'start', url] : [url];
  spawn(openCmd, openArgs, { stdio: 'ignore', detached: true }).unref();
}

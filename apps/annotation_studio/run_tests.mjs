// apps/annotation_studio/run_tests.mjs
import { createServer } from 'vite';

async function main() {
  const server = await createServer({
    server: { middlewareMode: true },
    appType: 'custom',
    optimizeDeps: { noDiscovery: true },
  });

  try {
    await server.ssrLoadModule('./tests/drawing_tools.test.ts');
  } finally {
    await server.close();
  }
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});

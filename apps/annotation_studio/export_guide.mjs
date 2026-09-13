// apps/annotation_studio/export_guide.mjs
// Serializes the canonical palm-guide spec for native + web consumers:
//   node export_guide.mjs [--output ../../shared/palm-guide/palm_guide.json]
import { createServer } from 'vite';
import { writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

async function main() {
  const server = await createServer({
    server: { middlewareMode: true },
    appType: 'custom',
    optimizeDeps: { noDiscovery: true },
  });

  try {
    const { buildGuideSpec } = await server.ssrLoadModule('./src/components/Sidebar/Panels/guide/spec.ts');
    const outArg = process.argv.find(a => a.startsWith('--output='));
    const outPath = resolve(outArg ? outArg.slice('--output='.length) : '../../shared/palm-guide/palm_guide.json');
    mkdirSync(dirname(outPath), { recursive: true });
    writeFileSync(outPath, JSON.stringify(buildGuideSpec(), null, 2) + '\n');
    console.log(`Wrote palm guide spec -> ${outPath}`);
  } finally {
    await server.close();
  }
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});

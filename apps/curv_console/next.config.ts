import type { NextConfig } from "next";
import { HSTS, sendHsts } from "./src/lib/csp";
import { LEGACY_REDIRECTS, NON_ORG_SEGMENTS } from "./src/lib/paths";

/**
 * HIP-9 (WP-C.19): the public share page and its download route are never cached or indexed. The CSP stays the
 * per-request nonce policy from proxy.ts; the page also sets `robots: noindex` metadata.
 */
const SHARE_HEADERS = [
  { key: "Cache-Control", value: "no-store" },
  { key: "X-Robots-Tag", value: "noindex, nofollow" },
];

/** An org segment: anything but the app's own top-level routes (`/api/exports/...` is not an org page). */
const ORG = `:org((?!(?:${NON_ORG_SEGMENTS.join("|")})$)[^/]+)`;

/**
 * WP-C.23 (N-7): the flat org paths before the route groups keep working for one release, as 308s to their new home.
 * Query strings pass through (`/{org}/sessions?flag=red` -> `/{org}/clinic/sessions?flag=red`).
 */
function legacyRedirects() {
  return LEGACY_REDIRECTS.map(({ from, to, nested }) => ({
    source: `/${ORG}/${from}${nested ? "/:path*" : ""}`,
    destination: `/:org/${to}${nested ? "/:path*" : ""}`,
    permanent: true,
  }));
}

const config: NextConfig = {
  reactStrictMode: true,
  output: "standalone",
  poweredByHeader: false,
  // No next/image usage; keeps the optional LGPL libvips binary (via sharp) unused. See README "Licences".
  images: { unoptimized: true },
  async redirects() {
    return legacyRedirects();
  },
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [
          // CSP-2. The CSP itself is per request (nonce) and lives in src/proxy.ts, which also stamps HSTS on its
          // own responses (redirects included). HSTS is production only.
          ...(sendHsts() ? [{ key: "Strict-Transport-Security", value: HSTS }] : []),
          { key: "X-Frame-Options", value: "DENY" },
          { key: "X-Content-Type-Options", value: "nosniff" },
          { key: "Referrer-Policy", value: "no-referrer" },
        ],
      },
      { source: "/share/:path*", headers: SHARE_HEADERS },
      { source: "/api/share/:path*", headers: SHARE_HEADERS },
    ];
  },
};

export default config;

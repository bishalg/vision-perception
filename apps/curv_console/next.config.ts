import type { NextConfig } from "next";
import { HSTS, sendHsts } from "./src/lib/csp";

const config: NextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  // No next/image usage; keeps the optional LGPL libvips binary (via sharp) unused. See README "Licences".
  images: { unoptimized: true },
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
    ];
  },
};

export default config;

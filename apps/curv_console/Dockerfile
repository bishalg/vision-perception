# syntax=docker/dockerfile:1
# owner: WP-B.18 (DPL-5)
#
# Curv Console production image (Next.js standalone server). Build context is this folder:
#
#   docker build -t asd-console apps/curv_console
#
# The Rust core for the browser (public/asd-wasm/) is included when it is in the context:
# run scripts/build_asd_wasm.sh and `node scripts/prepare-asd-wasm.mjs` first (CI
# deploy.yml does). Without it the sandbox re-score shows "Rust core unavailable"; pass
# --build-arg ASD_WASM_REQUIRED=1 to make that a build error.
# Runtime: CURV_API_MODE=live against http://api:8000 (compose), Secure cookies, non-root.

FROM node:22-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund

FROM node:22-alpine AS build
WORKDIR /app
ARG ASD_WASM_REQUIRED=0
ENV NEXT_TELEMETRY_DISABLED=1 \
    ASD_WASM_REQUIRED=${ASD_WASM_REQUIRED}
COPY --from=deps /app/node_modules ./node_modules
COPY . .
# prebuild copies the wasm (if any), postbuild checks the static HTML for nonce-less scripts.
RUN npm run build \
 # The standalone copy of .next/server was taken before postbuild stripped the scripts from
 # the forced-static error page; run the same check on the copy that ships.
 && node scripts/check-static-html-csp.mjs .next/standalone/.next

FROM node:22-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production \
    NEXT_TELEMETRY_DISABLED=1 \
    HOSTNAME=0.0.0.0 \
    PORT=3000 \
    CURV_API_MODE=live \
    CURV_API_BASE_URL=http://api:8000 \
    CURV_COOKIE_SECURE=true
COPY --from=build --chown=node:node /app/.next/standalone ./
COPY --from=build --chown=node:node /app/.next/static ./.next/static
COPY --from=build --chown=node:node /app/public ./public
USER node
EXPOSE 3000
CMD ["node", "server.js"]

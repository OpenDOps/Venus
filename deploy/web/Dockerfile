# syntax=docker/dockerfile:1
# Venus host (static) + nginx reverse proxy. Not AGPL. Do not COPY OctoBase.
FROM node:22-bookworm AS build

WORKDIR /src
RUN corepack enable

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json apps/web/
RUN pnpm install --frozen-lockfile

COPY apps/web/index.html apps/web/vite.config.ts apps/web/tsconfig.json apps/web/tsconfig.node.json apps/web/
COPY apps/web/src apps/web/src

# Runtime WS/blob URLs come from window.location (see resolveSyncUrl).
ENV VITE_SYNC_URL=same-origin
ENV VITE_SIDECAR_URL=http://127.0.0.1:3002
RUN pnpm --filter @venus/web exec vite build

FROM nginx:1.27-alpine

COPY deploy/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/apps/web/dist /usr/share/nginx/html

EXPOSE 80

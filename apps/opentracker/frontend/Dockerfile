FROM node:23.3.0-alpine3.19 as builder
COPY . .
RUN yarn install
RUN yarn build

FROM caddy:2.10-alpine AS runtime
COPY --from=builder ./Caddyfile /etc/caddy/Caddyfile
COPY --from=builder ./build /usr/share/caddy

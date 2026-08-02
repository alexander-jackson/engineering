FROM node:23.3.0-alpine3.19 as builder
COPY . .
RUN yarn install
RUN yarn build

FROM nginx:1.25.4-alpine3.18 AS runtime
COPY --from=builder ./nginx.conf /etc/nginx/nginx.conf
COPY --from=builder ./build /usr/share/nginx/html

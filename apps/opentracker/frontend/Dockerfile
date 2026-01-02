FROM node:21.5.0-alpine3.19 as builder
COPY . .
RUN yarn install
RUN yarn build

FROM nginx:1.25.3-alpine3.18 AS runtime
COPY --from=builder ./nginx.conf /etc/nginx/nginx.conf
COPY --from=builder ./build /usr/share/nginx/html

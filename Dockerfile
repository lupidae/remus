# The image is for pipelines, so it is built without the guided flow: nothing
# in a container has a terminal to prompt at, and dropping it removes 14 crates.
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN cargo build --release -p remus --no-default-features --locked

# remus talks to Postgres over TLS with webpki's bundled roots and reads nothing
# from the filesystem it does not own, so it needs no base image at all.
FROM scratch
COPY --from=build /src/target/release/remus /remus
ENTRYPOINT ["/remus"]

FROM docker.io/library/rust:latest as build
WORKDIR /build
COPY src/ /build/src
COPY Cargo.toml /build/Cargo.toml
RUN cargo build --release

FROM docker.io/library/debian:buster-slim as main
WORKDIR /app
COPY --from=build /build/target/release/test-service /app
ENTRYPOINT ["./test-service"]
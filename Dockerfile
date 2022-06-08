FROM rust:alpine as builder

ENV RUSTFLAGS="-C target-feature=-crt-static"

WORKDIR /usr/lib/pepe

RUN apk add --no-cache musl-dev openssl openssl-dev cmake make g++

# build dependecies (cache)
COPY Cargo.toml ./
RUN echo "fn main() {}" > dummy.rs && sed -i 's#src/main.rs#dummy.rs#' Cargo.toml
RUN cargo build --release

# build app
COPY ./src ./src
COPY ./config.yaml ./
RUN sed -i 's#dummy.rs#src/main.rs#' Cargo.toml
RUN cargo build --release


FROM alpine
WORKDIR /usr/lib/pepe
RUN apk add --no-cache libgcc

COPY --from=builder /usr/lib/pepe/target/release/service .

CMD ["./service"]

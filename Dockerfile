FROM rust:alpine as builder

ENV RUSTFLAGS="-C target-feature=-crt-static"

WORKDIR /usr/lib/pepe

RUN apk add --no-cache musl-dev openssl openssl-dev cmake make g++

# build dependecies (cache)
COPY Cargo.toml ./
RUN echo "fn main() {}" > mod && sed -i 's#src/main.rs#mod#' Cargo.toml
RUN cargo build --release

# build app
COPY ./src ./src
COPY ./config.yaml ./
RUN sed -i 's#mod#src/main.rs#' Cargo.toml
RUN cargo build --release


FROM alpine
WORKDIR /usr/lib/pepe
RUN apk add --no-cache libgcc

COPY --from=builder /usr/lib/pepe/target/release/service .

CMD ["./service"]

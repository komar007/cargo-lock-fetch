FROM rust:1.88.0-alpine3.22 AS builder

# Tools layer
RUN apk update \
 && apk add --no-cache musl-dev

# Use cargo-lock-fetch from its docker distribution
COPY --from=komar007/cargo-lock-fetch:0.2.0 \
    /cargo-lock-fetch /usr/local/cargo/bin

WORKDIR /app

# Dependencies layer: fetch all dependencies, but only rebuild layer
# when Cargo.lock changes.
#
# This is for demonstration only - using cargo-lock-fetch starts to
# matter only when multiple Cargo.toml files are used because the
# project consists of many crates. It eliminates the need to specify
# each and every Crate.toml file to be copied into the build context.
COPY Cargo.lock .
RUN cargo lock-fetch

# Sources layer: the build runs offline here. This layer rebuilds when
# any file changes, but dependencies are cached in the previous layer.
COPY . .
RUN cargo build --frozen --release

FROM scratch

COPY --from=builder /app/target/release/app /app
CMD [ "/app" ]

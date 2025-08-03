FROM rust:1.88.0-alpine3.22 AS builder

RUN apk update \
 && apk add --no-cache musl-dev \
 && cargo install cargo-lock-fetch

WORKDIR /app

COPY Cargo.lock .
RUN cargo lock-fetch

COPY . .
RUN cargo build --frozen --release

FROM scratch
COPY --from=builder /app/target/release/cargo-lock-fetch /cargo-lock-fetch
CMD [ "/cargo-lock-fetch" ]

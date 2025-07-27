# `cargo-lock-fetch` - `cargo fetch` and `cargo vendor` with just Cargo.lock

![Crates.io License](https://img.shields.io/crates/l/cargo-lock-fetch) [![Crates.io
Version](https://img.shields.io/crates/v/cargo-lock-fetch)](https://crates.io/crates/cargo-lock-fetch/)
![GitHub branch check runs](https://img.shields.io/github/check-runs/komar007/cargo-lock-fetch/main)
![Crates.io MSRV](https://img.shields.io/crates/msrv/cargo-lock-fetch)

This [cargo](https://doc.rust-lang.org/cargo/) plugin fetches and optionally vendors crates based
only on `Cargo.lock`.

It is particularly useful when building rarely changing docker layers containing just project
dependencies without copying/mounting all `Cargo.toml` files of a multi-crate workspace.

## Installation

``` sh
cargo install cargo-lock-fetch
```

## Usage

To fetch dependencies to cargo’s registry cache:

``` sh
cargo lock-fetch --lockfile-path path/to/Cargo.lock
```

To additionally vendor the dependencies (like `cargo vendor`):

``` sh
cargo lock-fetch --lockfile-path path/to/Cargo.lock --vendor vendor_dir/
```

There is no need to run `cargo lock-fetch` from any specific directory.

## Example: primary use case

The following example is the reason this plugin was written.

Assuming the `Dockerfile` is in the root directory of a cargo project, a minimal setup that caches
project dependencies in a docker layer and rebuilds it only on Cargo.lock changes would look like
this:

```dockerfile
FROM rust:1.88.0-alpine3.22 AS builder

# Tools layer
RUN apk update \
 && apk add --no-cache musl-dev \
 && cargo install cargo-lock-fetch

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
```

The idea can be tested with `docker compose build` in `examples/fetch-deps-to-layer`.

> [!TIP]
> This simple example will benefit from using build cache (`RUN --mount=type=cache`) for
> `$CARGO_HOME` so that each Cargo.lock update only downloads the added dependencies instead of
> re-downloading all of them, but it is not covered here. Similarly, build cache can be used to
> speed up incremental builds by letting cargo reuse `$CARGO_TARGET_DIR`.

> [!WARNING]
> Don't shoot yourself in the foot while using cache mounts in docker builds, remember to use
> sensible values of `id` in each `RUN --mount=type=cache`. You have been warned.

## How it works

In order to use `cargo` to fetch the crates, `cargo-lock-fetch` creates a cargo package and adds the
dependencies found in the input Cargo.lock file to its Cargo.toml, and then calls `cargo fetch` and
optionally `cargo vendor`.

Because a single Cargo.toml file cannot contain multiple versions of the same crate as dependencies,
and this situation is perfectly correct for cargo packages if the versions are pulled in indirectly
by different dependencies, `cargo-lock-fetch` distributes the list of dependencies between
sub-crates using an approach based on greedy vertex coloring, which is optimal for cluster graphs
(there is an edge between 2 dependencies iff they are different versions of the same crate).

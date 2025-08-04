# `cargo-lock-fetch` - `cargo fetch` and `cargo vendor` with just Cargo.lock

![Crates.io License](https://img.shields.io/crates/l/cargo-lock-fetch)
[![Crates.io Version](https://img.shields.io/crates/v/cargo-lock-fetch)](https://crates.io/crates/cargo-lock-fetch/)
[![Docker Image Version](https://img.shields.io/docker/v/komar007/cargo-lock-fetch?logo=docker&label=hub)](https://hub.docker.com/r/komar007/cargo-lock-fetch)
![GitHub branch check runs](https://img.shields.io/github/check-runs/komar007/cargo-lock-fetch/main)
![Crates.io MSRV](https://img.shields.io/crates/msrv/cargo-lock-fetch)

This [cargo](https://doc.rust-lang.org/cargo/) plugin fetches and optionally vendors crates based
only on `Cargo.lock`.

It is particularly useful when building rarely changing docker layers containing just project
dependencies without copying/mounting all `Cargo.toml` files of a multi-crate workspace.

## Installation

`cargo-lock-fetch` is mainly intended to be used with containers. Docker users can copy it from the
binary docker distribution:

``` dockerfile
COPY --from=komar007/cargo-lock-fetch \
    /cargo-lock-fetch /usr/local/cargo/bin
```

It's also possible to build from source:

``` sh
cargo install cargo-lock-fetch
```

or install a binary release from github:

``` sh
cargo binstall cargo-lock-fetch # requires cargo-binstall
```

> [!IMPORTANT]
> For reproducible builds, avoid omitting version requirements when specifying dependencies. See
> below for semver guarantees. For `cargo` `install`/`binstall` use `cargo-lock-fetch@0.x.y`, for
> docker images, use specific tag: `komar007/cargo-lock-fetch:0.x.y`.

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

## [SemVer](https://semver.org/) compatibility

This tool follows the cargo/semver guidelines with respect to its CLI interface. At the current
`0.x.y` stage, changes of `x` (MINOR) indicate breaking changes. `cargo-lock-fetch` is close to
declaring a public interface which will be indicated by reaching version `1.0.0`. From this moment,
breaking changes to the CLI interface will be indicated by MAJOR version increments.

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

The example can be tested with `docker compose build` in `examples/fetch-deps-to-layer`.

## How it works

In order to use `cargo` to fetch the crates, `cargo-lock-fetch` creates a cargo package and adds the
dependencies found in the input Cargo.lock file to its Cargo.toml, and then calls `cargo fetch` and
optionally `cargo vendor`.

Because a single Cargo.toml file cannot contain multiple versions of the same crate as dependencies,
and this situation is perfectly correct for cargo packages if the versions are pulled in indirectly
by different dependencies, `cargo-lock-fetch` distributes the list of dependencies between
sub-crates using an approach based on greedy vertex coloring, which is optimal for cluster graphs
(there is an edge between 2 dependencies iff they are different versions of the same crate).

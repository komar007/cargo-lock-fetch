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

## How it works

In order to use `cargo` to fetch the crates, `cargo-lock-fetch` creates a cargo package and adds the
dependencies found in the input Cargo.lock file to its Cargo.toml, and then calls `cargo fetch` and
optionally `cargo vendor`.

Because a single Cargo.toml file cannot contain multiple versions of the same crate as dependencies,
and this situation is perfectly correct for cargo packages if the versions are pulled in indirectly
by different dependencies, `cargo-lock-fetch` distributes the list of dependencies between
sub-crates using an approach based on greedy vertex coloring, which is optimal for cluster graphs
(there is an edge between 2 dependencies iff they are different versions of the same crate).

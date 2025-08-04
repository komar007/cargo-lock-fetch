# Docker example: fetch depenencies to docker layer, but use binary distribution of cargo-lock-fetch

This example shows how to use prebuilt cargo-lock-fetch in a builder stage of a docker-based rust
project.

## Building

Run:

```sh
docker compose run --build example
```

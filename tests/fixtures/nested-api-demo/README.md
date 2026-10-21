# Nested API demo

This is a local preview fixture for the package registry, not a published library.
It uses a nested entrypoint, an internal module, a test suite, and an example.

```text
sema.toml
src/
  api.sema
  internal/
    scale.sema
tests/
  api.sema
examples/
  basic.sema
```

The API reference can extract the direct declarations of `demo/clamp` and
`demo/scale` from `src/api.sema`. It lists the imported alias `demo/triple` as
unresolved. It does not follow imports or expose the internal module, tests,
and examples as additional source files.

Run the test suite from this directory:

```sh
sema tests/api.sema
```

The tests cover both clamp boundaries, an in-range value, the helper wrapper,
and the imported alias. They do not need network access or extra packages.

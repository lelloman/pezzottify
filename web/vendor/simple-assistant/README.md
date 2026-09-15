# Simple Assistant packages

These npm archives contain the built Simple Assistant 0.2.0 web packages from
[`simple-android-assistant`](https://github.com/lelloman/simple-android-assistant),
source revision `9a771be5f74d385aa636efd968ff310279aaa3c7`.
They include JavaScript, declarations, the core WASM asset, and Apache-2.0
LICENSE/NOTICE files. The lockfile records each archive's integrity hash.

To update them:

1. Check out the desired source revision and follow that repository's toolchain
   setup, then run `npm ci`, `npm run build`, and `npm run pack:all` there.
2. Copy its three `target/packages/*.tgz` files into this directory, removing
   superseded versions. Update the source revision above and the archive paths
   in `web/package.json` if their versions changed.
3. Run `npm install --package-lock-only` in `web`, then `npm ci`,
   `npm run test:unit`, and `npm run build`.
4. From the Pezzottify repository root, validate the isolated Docker stage with
   `docker build --target web-builder -f pezzottify-server/Dockerfile .`.

Commit the archives together with the package manifest and lockfile. Consumers
do not need a sibling checkout or a Rust/WASM toolchain.

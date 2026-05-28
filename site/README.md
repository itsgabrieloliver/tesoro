# tesoro · landing site

One-page static site for tesoro, served by Nubo via the httpd buildpack.

## Layout

```
index.html         landing page
css/style.css      styling
downloads/         empty in the repo; Nubo Volume mounts here at deploy time
project.toml       buildpack config (httpd, root = ".", port 8080)
```

## Deploy on Nubo

1. **Create a Project** in the Nubo dashboard (e.g. `tesoro`).
2. **New Frame** pointing at this repo's branch. Tracked branch can be `main`.
   The buildpack is auto-detected from `project.toml` (httpd + workspace root).
3. **Create a Volume** in the same Project — name it `tesoro-downloads`, size 1 GiB is plenty.
4. **Attach the Volume** to the Frame at the mount path **`/workspace/downloads`**.
   That path overlays the empty `downloads/` directory in the repo, so binaries you
   put on the volume become reachable at `/downloads/<file>` on the served URL.
5. Push to the tracked branch — Nubo builds and rolls out.

## Upload binaries to the volume

The volume is just a folder the site Frame sees. To put files on it, you need a Frame
that *writes* to it. The repo ships both halves of that wiring:

- [`../site-uploader/`](../site-uploader) — a tiny **sshd Frame** for the same Project
  with the same volume attached at `/var/lib/tesoro/downloads`. Set `SSH_AUTHORIZED_KEYS`
  on it and you can `scp` tarballs onto the volume from anywhere with the matching key.
- [`../.github/workflows/release.yml`](../.github/workflows/release.yml) — a GitHub
  Actions workflow that cross-compiles the four targets on tag push and scp's them onto
  the uploader Frame automatically when these repo secrets are set:

  | Secret | Value |
  | --- | --- |
  | `UPLOAD_HOST` | hostname of the uploader Frame |
  | `UPLOAD_PORT` | `22` unless remapped |
  | `UPLOAD_SSH_KEY` | private key matching `SSH_AUTHORIZED_KEYS` |

The artifacts that need to land on the volume (file names match `index.html`):

```
tesoro-aarch64-apple-darwin.tar.gz
tesoro-x86_64-apple-darwin.tar.gz
tesoro-x86_64-unknown-linux-gnu.tar.gz
tesoro-aarch64-unknown-linux-gnu.tar.gz
```

The CI workflow drops the matching `.sha256` next to each, so client downloads can be
checksum-verified.

## Local preview

The buildpack also runs locally if you have `pack` (Cloud Native Buildpacks CLI):

```
cd site
pack build tesoro-site --builder paketobuildpacks/builder-jammy-base
docker run --rm -p 8080:8080 \
  -v "$PWD/downloads:/workspace/downloads:ro" \
  tesoro-site
open http://localhost:8080
```

Without `pack`, just open `index.html` directly in a browser to preview the page; the
download links won't resolve until the volume is mounted on a real Frame.

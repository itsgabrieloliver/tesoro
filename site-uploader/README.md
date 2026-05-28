# tesoro-uploader

Tiny sshd Frame whose only job is to receive release tarballs (via `scp`) and write
them to the Nubo Volume that the [site](../site) reads its `/downloads/` from.

## Deploy on Nubo

1. **New Frame** in the same Project as `tesoro-site`, pointing at this directory.
   Nubo detects the `Dockerfile` and builds the image.
2. **Attach the same `tesoro-downloads` Volume** at mount path **`/var/lib/tesoro/downloads`**.
3. Set the Frame's exposed port to **`22`** (TCP). If your Nubo plan doesn't route raw
   TCP/22 publicly, switch to the HTTP-uploader alternative noted at the bottom.
4. Add a runtime env var:
   - `SSH_AUTHORIZED_KEYS` — paste a public key (one line, normal `authorized_keys` format).
     The deploy user is named `deploy`.

## Upload by hand from a laptop

```bash
scp tesoro-aarch64-apple-darwin.tar.gz \
    deploy@<uploader-host>:/var/lib/tesoro/downloads/
```

## Upload from CI (GitHub Actions)

The workflow at `.github/workflows/release.yml` does this on every tag push when
these repo secrets are set:

| Secret             | Value                                                  |
| ------------------ | ------------------------------------------------------ |
| `UPLOAD_HOST`      | Hostname/IP of the uploader Frame                      |
| `UPLOAD_PORT`      | TCP port (`22` unless you've remapped)                 |
| `UPLOAD_SSH_KEY`   | Private key matching `SSH_AUTHORIZED_KEYS` on the Frame |

Tag `v0.1.0` and the four target tarballs land on the volume — and the next page
load on the site picks them up.

## HTTP alternative

If raw TCP/22 isn't usable on your Nubo plan, swap the sshd image for a tiny
HTTP service that accepts authenticated `POST /upload/<filename>` with a bearer
token and writes the body to `/var/lib/tesoro/downloads/<filename>`. The CI step
becomes a `curl --data-binary @file` instead of `scp`. I can stub that out
whenever you want.

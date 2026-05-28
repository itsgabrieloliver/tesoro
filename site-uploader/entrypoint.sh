#!/bin/bash
set -euo pipefail

# Install the deploy user's authorized keys from the env each boot, so
# rotating the key just needs an env update + Frame restart.
if [ -n "${SSH_AUTHORIZED_KEYS:-}" ]; then
  printf '%s\n' "$SSH_AUTHORIZED_KEYS" > /home/deploy/.ssh/authorized_keys
  chmod 600 /home/deploy/.ssh/authorized_keys
  chown deploy:deploy /home/deploy/.ssh/authorized_keys
else
  echo "warning: SSH_AUTHORIZED_KEYS is empty — no logins will succeed" >&2
fi

exec /usr/sbin/sshd -D -e

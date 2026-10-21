#!/usr/bin/env bash
# Scratch copy of PlexusTerm v0.2.15 scripts/openssh-interop.sh for proto-ssh. Args: cargo test filter args.
set -euo pipefail
SCR="$(dirname "$0")"
WORK=$(mktemp -d "$SCR/interop.XXXX"); NAME="plexusterm-native-interop-$$"
cleanup(){ docker rm -f "$NAME" >/dev/null 2>&1 || true; rm -rf "$WORK"; }
trap cleanup EXIT
ssh-keygen -q -t ed25519 -N '' -C plexusterm-interop -f "$WORK/id_ed25519"
PUBKEY="$(cat "$WORK/id_ed25519.pub")"
docker run -d --name "$NAME" -p 127.0.0.1::22 -e PUBKEY="$PUBKEY" alpine:3.20 sh -c '
  set -e
  apk add --no-cache openssh-server openssh-sftp-server >/dev/null
  ssh-keygen -A >/dev/null
  adduser -D -s /bin/sh tester
  passwd -u tester >/dev/null 2>&1 || true
  mkdir -p /home/tester/.ssh
  echo "$PUBKEY" > /home/tester/.ssh/authorized_keys
  chown -R tester:tester /home/tester/.ssh
  chmod 700 /home/tester/.ssh && chmod 600 /home/tester/.ssh/authorized_keys
  exec /usr/sbin/sshd -D -e -o PasswordAuthentication=no -o AllowTcpForwarding=yes
' >/dev/null
PORT="$(docker port "$NAME" 22/tcp | head -n1 | sed 's/.*://')"
for _ in $(seq 90); do docker logs "$NAME" 2>&1 | grep -q "Server listening" && break; sleep 1; done
docker logs "$NAME" 2>&1 | grep -q "Server listening" || { docker logs "$NAME" | tail; exit 1; }
docker exec "$NAME" sh -c 'sshd -V 2>&1 | head -n1' || true
cd "$INTEROP_DIR"
set +e
PLEXUSTERM_INTEROP_PORT="$PORT" PLEXUSTERM_INTEROP_USER=tester PLEXUSTERM_INTEROP_KEY="$WORK/id_ed25519" \
  cargo test -p "$INTEROP_CRATE" -- --ignored --test-threads=1 "$@"
rc=$?
docker logs "$NAME" 2>&1 | tail -n 15
exit $rc

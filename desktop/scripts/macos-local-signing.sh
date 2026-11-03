#!/bin/sh
# Creates a self-signed code-signing certificate in your login keychain, once, so
# local macOS builds keep one stable code identity. An ad-hoc signature changes with
# every build, and macOS then treats each build as a different app: notification
# permission (and other per-app grants) are not kept. Run this yourself; it asks
# macOS to trust the certificate for code signing, which needs your password.
#
#   sh scripts/macos-local-signing.sh            # creates "NPi Desktop Local"
#   npm run build -- --config src-tauri/tauri.localsign.conf.json
set -eu
NAME="${1:-NPi Desktop Local}"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"
if security find-identity -v -p codesigning | grep -q "\"$NAME\""; then
  echo "\"$NAME\" already exists in your keychain."; exit 0
fi
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
cat > "$tmp/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
CNF
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -config "$tmp/cert.cnf" \
  -keyout "$tmp/key.pem" -out "$tmp/cert.pem" 2>/dev/null
openssl pkcs12 -export -inkey "$tmp/key.pem" -in "$tmp/cert.pem" -name "$NAME" \
  -out "$tmp/id.p12" -passout pass:npi-local
security import "$tmp/id.p12" -k "$KEYCHAIN" -P npi-local -T /usr/bin/codesign
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$tmp/cert.pem"
echo "Created \"$NAME\". Build with: npm run build -- --config src-tauri/tauri.localsign.conf.json"

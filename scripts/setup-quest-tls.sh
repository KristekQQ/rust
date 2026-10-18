#!/usr/bin/env bash
# Create a private development CA and a LAN certificate, without changing system trust.
set -euo pipefail
umask 077
lan_ip="${1:-$(ipconfig getifaddr en0 2>/dev/null || true)}"
python3 -c 'import ipaddress,sys; ipaddress.ip_address(sys.argv[1])' "$lan_ip" || {
    echo 'Usage: scripts/setup-quest-tls.sh <Mac-LAN-IP>' >&2; exit 1;
}
tls_dir="$HOME/.local/share/rust-webxr/tls"
mkdir -p "$tls_dir"
if [[ ! -f "$tls_dir/rootCA.key" && ! -f "$tls_dir/rootCA.crt" ]]; then
    openssl req -x509 -newkey rsa:3072 -sha256 -nodes -days 3650 \
        -keyout "$tls_dir/rootCA.key" -out "$tls_dir/rootCA.crt" \
        -subj '/CN=Rust WebXR Local Development CA' \
        -addext 'basicConstraints=critical,CA:TRUE,pathlen:0' \
        -addext 'keyUsage=critical,keyCertSign,cRLSign' >/dev/null 2>&1
fi
[[ -f "$tls_dir/rootCA.key" && -f "$tls_dir/rootCA.crt" ]] || {
    echo "Incomplete CA in $tls_dir; preserve it and repair manually." >&2; exit 1;
}
work_dir="$(mktemp -d "$tls_dir/leaf.XXXXXX")"
trap 'rm -rf "$work_dir"' EXIT
openssl req -new -newkey rsa:2048 -sha256 -nodes \
    -keyout "$work_dir/server.key" -out "$work_dir/server.csr" \
    -subj '/CN=Rust WebXR local server' >/dev/null 2>&1
cat > "$work_dir/extensions.cnf" <<EXT
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=IP:$lan_ip,IP:127.0.0.1,DNS:localhost
EXT
openssl x509 -req -in "$work_dir/server.csr" -CA "$tls_dir/rootCA.crt" \
    -CAkey "$tls_dir/rootCA.key" -set_serial "0x$(openssl rand -hex 16)" \
    -out "$work_dir/server.crt" -days 90 -sha256 -extfile "$work_dir/extensions.cnf" >/dev/null 2>&1
openssl verify -CAfile "$tls_dir/rootCA.crt" -verify_ip "$lan_ip" "$work_dir/server.crt"
mv "$work_dir/server.key" "$tls_dir/server.key"
mv "$work_dir/server.crt" "$tls_dir/server.crt"
chmod 600 "$tls_dir/"*.key
chmod 644 "$tls_dir/"*.crt
printf 'TLS files: %s\nQuest CA certificate: %s/rootCA.crt\n' "$tls_dir" "$tls_dir"
printf 'Local URL: https://%s:8443/examples/webxr/\n' "$lan_ip"
echo 'CA trust has NOT been installed on Mac or Quest. Keep private *.key files private.'

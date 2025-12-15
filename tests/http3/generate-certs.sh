#!/bin/bash
#
# Generate self-signed certificates for HTTP/3 testing
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CERT_DIR="$SCRIPT_DIR/certs"

# Create certs directory
mkdir -p "$CERT_DIR"

echo "==> Generating self-signed certificates for HTTP/3 testing..."

# Generate private key
openssl genrsa -out "$CERT_DIR/server.key" 2048 2>/dev/null

# Generate certificate signing request (CSR)
openssl req -new -key "$CERT_DIR/server.key" \
    -out "$CERT_DIR/server.csr" \
    -subj "/C=US/ST=Test/L=Test/O=Test/OU=Test/CN=localhost" \
    2>/dev/null

# Create certificate extensions file for SAN (Subject Alternative Name)
cat > "$CERT_DIR/cert_ext.cnf" <<EOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = *.localhost
IP.1 = 127.0.0.1
IP.2 = ::1
EOF

# Generate self-signed certificate (valid for 365 days)
openssl x509 -req \
    -in "$CERT_DIR/server.csr" \
    -signkey "$CERT_DIR/server.key" \
    -out "$CERT_DIR/server.crt" \
    -days 365 \
    -extfile "$CERT_DIR/cert_ext.cnf" \
    2>/dev/null

# Clean up CSR and extensions file
rm -f "$CERT_DIR/server.csr" "$CERT_DIR/cert_ext.cnf"

# Set appropriate permissions
chmod 600 "$CERT_DIR/server.key"
chmod 644 "$CERT_DIR/server.crt"

echo "✓ Certificate generated: $CERT_DIR/server.crt"
echo "✓ Private key generated: $CERT_DIR/server.key"
echo ""
echo "Certificate details:"
openssl x509 -in "$CERT_DIR/server.crt" -noout -text | grep -A 1 "Subject:"
openssl x509 -in "$CERT_DIR/server.crt" -noout -text | grep -A 3 "Subject Alternative Name"
echo ""
echo "✓ Certificates ready for HTTP/3 testing"

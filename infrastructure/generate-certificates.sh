#!/usr/bin/env bash

set -e

# Parse command line arguments
MODE="from-scratch"
if [[ $# -gt 0 ]]; then
	MODE="$1"
fi

function generate_client_cert() {
	client_name=$1
	common_name=$2

	echo "Generating client certificate for $client_name ($common_name)..."

	# Generate a key, CSR and certificate for the client
	openssl genrsa -out "$client_name.key" 2048 2>/dev/null
	openssl req -new -key "$client_name.key" -subj "/C=GB/ST=England/L=London/O=client/CN=$common_name" -addext "subjectAltName = DNS:localhost" -out "$client_name.csr" 2>/dev/null
	openssl x509 -req -in "$client_name.csr" -CA ca.crt -CAkey ca.key -CAcreateserial -extfile <(printf "subjectAltName=DNS:localhost") -days 365 -out "$client_name.crt" 2>/dev/null

	# Generate a PEM file for `curl` and a `p12` for browser usage
	cat "$client_name.crt" "$client_name.key" > "$client_name.pem"
	/usr/bin/openssl pkcs12 -export -in "$client_name.pem" -out "$client_name.p12" -name "$client_name" -passout pass:
}

function generate_from_scratch() {
	echo "Generating certificates from scratch..."

	# Remove existing certificates
	if [[ -d certs ]]; then
		rm -rf certs
	fi

	# Create the directory again
	mkdir -p certs
	cd certs

	# Generate a key for the CA as well as a self-signed certificate
	echo "Generating root CA..."
	openssl genrsa -out ca.key 2048 2>/dev/null
	openssl req -new -x509 -key ca.key -out ca.crt -subj "/C=GB/ST=England/L=London/O=root/CN=localhost" 2>/dev/null

	# Generate a key, CSR and certificate for the server
	echo "Generating server certificate..."
	openssl genrsa -out localhost.key 2048 2>/dev/null
	openssl req -new -key localhost.key -subj "/C=GB/ST=England/L=London/O=server/CN=localhost" -addext "subjectAltName = DNS:localhost" -out localhost.csr 2>/dev/null
	openssl x509 -req -in localhost.csr -CA ca.crt -CAkey ca.key -CAcreateserial -extfile <(printf "subjectAltName=DNS:localhost") -out localhost.crt 2>/dev/null

	# Concatenate the certificates for the server to use
	cat localhost.crt ca.crt > localhost.bundle.crt

	# Generate client certificates
	generate_client_cert mobile "Pixel 6"
	generate_client_cert work "M1 Max"
	generate_client_cert personal "M2 Pro"

	cd ..

	echo "Certificate generation complete!"
}

function cleanup_intermediate_files() {
	echo "Cleaning up intermediate files..."

	if [[ ! -d certs ]]; then
		echo "Error: certs directory does not exist"
		exit 1
	fi

	cd certs

	# Remove intermediate files that aren't needed for distribution
	rm -f *.csr  # Certificate signing requests
	rm -f ca.srl # Serial number file

	# Organize files into distribution folders
	mkdir -p dist/server dist/clients

	# Server files
	echo "Organizing server files..."
	cp localhost.bundle.crt dist/server/
	cp localhost.key dist/server/
	cp ca.crt dist/server/

	# Client files
	echo "Organizing client files..."
	for client in mobile work personal; do
		mkdir -p "dist/clients/$client"
		cp "$client.p12" "dist/clients/$client/"
		cp ca.crt "dist/clients/$client/ca.crt"
	done

	cd ..

	echo "Cleanup complete! Distribution files are in certs/dist/"
}

function package_mobile() {
	echo "Packaging mobile certificate for Android..."

	if [[ ! -d certs ]]; then
		echo "Error: certs directory does not exist"
		exit 1
	fi

	cd certs

	# Create mobile distribution directory
	mkdir -p dist/clients/mobile/android

	# Android needs:
	# 1. The CA certificate in DER format (to install as trusted CA)
	# 2. The client certificate in PKCS12 format with the CA chain included

	echo "Converting CA to DER format for Android..."
	openssl x509 -outform der -in ca.crt -out dist/clients/mobile/android/ca.der 2>/dev/null

	echo "Creating PKCS12 bundle with full certificate chain..."
	# Create a full chain PEM (client cert + CA cert)
	cat mobile.crt ca.crt > mobile.chain.pem
	cat mobile.chain.pem mobile.key > mobile.full.pem

	# Create PKCS12 with the full chain (with password for Android compatibility)
	# Using AES-256-CBC and SHA256 for Android compatibility
	/usr/bin/openssl pkcs12 -export \
		-in mobile.crt \
		-inkey mobile.key \
		-certfile ca.crt \
		-out dist/clients/mobile/android/mobile-with-ca.p12 \
		-name "Pixel 6" \
		-keypbe AES-256-CBC \
		-certpbe AES-256-CBC \
		-macalg sha256 \
		-passout pass:password

	# Also copy the standard p12 for reference
	cp mobile.p12 dist/clients/mobile/android/mobile.p12

	# Create a README for installation
	cat > dist/clients/mobile/android/README.md <<EOF
# Android mTLS Certificate Installation

## Files
- \`ca.der\`: Root CA certificate (install as trusted CA)
- \`mobile-with-ca.p12\`: Client certificate with CA chain (recommended)
- \`mobile.p12\`: Client certificate only (alternative)

## Installation Steps

### Option 1: Install full chain (Recommended)
1. Transfer \`mobile-with-ca.p12\` to your Android device
2. Open the file or go to Settings > Security > Install from storage
3. Select the .p12 file and install
4. When prompted for password, enter: \`password\`

### Option 2: Install separately
1. Transfer both \`ca.der\` and \`mobile.p12\` to your device
2. First install \`ca.der\` as a CA certificate:
   - Settings > Security > Install from storage > CA certificate
3. Then install \`mobile.p12\` as a user certificate:
   - Settings > Security > Install from storage > VPN & app user certificate
4. No password required for \`mobile.p12\` (empty password)

## Password
- \`mobile-with-ca.p12\`: **password**
- \`mobile.p12\`: (empty/no password)
- \`ca.der\`: (no password needed)

## Notes
- The certificate is valid for 365 days from generation
- Common name: Pixel 6
- Used for mTLS authentication to localhost
EOF

	# Cleanup temporary files
	rm -f mobile.chain.pem mobile.full.pem

	cd ..

	echo "Mobile packaging complete! Files are in certs/dist/clients/mobile/android/"
}

# Main execution
case "$MODE" in
	from-scratch)
		generate_from_scratch
		cleanup_intermediate_files
		package_mobile
		;;
	cleanup)
		cleanup_intermediate_files
		;;
	package-mobile)
		package_mobile
		;;
	*)
		echo "Usage: $0 [from-scratch|cleanup|package-mobile]"
		echo ""
		echo "Modes:"
		echo "  from-scratch    - Generate all certificates from scratch (default)"
		echo "  cleanup         - Organize and cleanup existing certificate directory"
		echo "  package-mobile  - Package mobile certificates for Android"
		echo ""
		echo "The 'from-scratch' mode automatically runs cleanup and package-mobile."
		exit 1
		;;
esac

echo ""
echo "Done! Certificate structure:"
echo "  certs/dist/server/           - Server certificates"
echo "  certs/dist/clients/*/        - Client certificates"
echo "  certs/dist/clients/mobile/android/ - Android-specific packages"

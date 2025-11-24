# WebSocket Proxying with Authenticated Encryption

## Overview

The reverse proxy now supports WebSocket proxying with transparent authenticated encryption. This feature allows users to secure WebSocket traffic between clients and the proxy, while the proxy communicates with backends in plaintext (or re-encrypts as needed).

## Architecture

```
Client <--[Encrypted WebSocket]--> Proxy <--[Plain WebSocket]--> Backend
```

The proxy acts as a cryptographic gateway:
1. **Decrypt** messages from clients
2. **Verify** signatures and HMAC
3. **Forward** plaintext to backend
4. **Encrypt** responses from backend
5. **Sign** messages before sending to client

## Supported Cryptographic Algorithms

### Encryption

#### 1. **AES-256-GCM** (Recommended)
- **Type**: Symmetric AEAD (Authenticated Encryption with Associated Data)
- **Key Size**: 256 bits (32 bytes)
- **Nonce**: 96 bits (12 bytes)
- **Use Case**: High performance, hardware-accelerated on most CPUs
- **Security**: NIST-approved, battle-tested

#### 2. **ChaCha20-Poly1305**
- **Type**: Symmetric AEAD
- **Key Size**: 256 bits (32 bytes)
- **Nonce**: 96 bits (12 bytes)
- **Use Case**: Software-only implementations, mobile devices
- **Security**: Modern, constant-time implementation

#### 3. **RSA-OAEP**
- **Type**: Asymmetric encryption
- **Key Sizes**: 2048, 3072, or 4096 bits
- **Use Case**: Key exchange, hybrid encryption
- **Security**: Well-established, slower than symmetric

#### 4. **X25519 + ChaCha20-Poly1305** (Modern Hybrid)
- **Type**: ECDH key exchange + symmetric AEAD
- **Key Size**: 256 bits (32 bytes for X25519)
- **Use Case**: Modern applications, perfect forward secrecy
- **Security**: State-of-the-art, fast, secure

### Message Authentication

#### 1. **HMAC-SHA256** (Defense in Depth)
- Additional MAC layer on top of AEAD
- Protects against implementation flaws
- Recommended for critical applications

#### 2. **HMAC-SHA512**
- Stronger variant of HMAC
- Slightly slower but more secure

### Digital Signatures (Non-Repudiation)

#### 1. **Ed25519** (Recommended)
- **Type**: EdDSA (Edwards-curve Digital Signature Algorithm)
- **Key Size**: 256 bits (32 bytes)
- **Signature Size**: 64 bytes
- **Use Case**: Modern applications requiring non-repudiation
- **Security**: Fast, secure, small signatures

#### 2. **RSA-PSS**
- **Type**: RSA Probabilistic Signature Scheme
- **Key Sizes**: 2048, 3072, or 4096 bits
- **Use Case**: Legacy systems, compliance requirements
- **Security**: Well-established, larger signatures

## Configuration

### Example 1: AES-256-GCM with HMAC and Ed25519 Signatures

```yaml
server:
  bind:
    - "0.0.0.0:8080"

websocket:
  enabled: true
  max_message_size: 16777216  # 16 MB
  max_frame_size: 16384        # 16 KB
  ping_interval: 30
  pong_timeout: 10

  encryption:
    algorithm: aes-256-gcm
    enable_hmac: true
    hmac_algorithm: sha256
    enable_signature: true
    signature_algorithm: ed25519
    max_message_age: 300  # 5 minutes (replay protection)

    keys:
      # Generate with: openssl rand -base64 32
      encryption_key: "base64-encoded-32-byte-key"

      # Generate with: openssl rand -base64 32
      hmac_key: "base64-encoded-hmac-secret"

      # Generate Ed25519 keys
      ed25519_private_key: "base64-encoded-private-key"
      ed25519_public_key: "base64-encoded-public-key"

routes:
  - name: "websocket_app"
    match:
      paths:
        - "/ws/*"
      upgrade: "websocket"
    upstream: "websocket_backend"
```

### Example 2: X25519 + ChaCha20-Poly1305 (Modern Hybrid)

```yaml
websocket:
  enabled: true

  encryption:
    algorithm: x25519_chacha20
    enable_hmac: false  # AEAD already provides authentication
    enable_signature: true
    signature_algorithm: ed25519
    max_message_age: 300

    keys:
      # X25519 keys for ECDH key exchange
      x25519_private_key: "base64-encoded-x25519-private"
      x25519_public_key: "base64-encoded-x25519-public"

      # Ed25519 for signatures
      ed25519_private_key: "base64-encoded-ed25519-private"
      ed25519_public_key: "base64-encoded-ed25519-public"
```

### Example 3: RSA for Encryption and Signatures

```yaml
websocket:
  enabled: true

  encryption:
    algorithm: rsa_oaep
    enable_hmac: true
    hmac_algorithm: sha512
    enable_signature: true
    signature_algorithm: rsa_pss
    max_message_age: 300

    keys:
      # RSA keys (PEM format)
      rsa_private_key: |
        -----BEGIN PRIVATE KEY-----
        MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC...
        -----END PRIVATE KEY-----

      rsa_public_key: |
        -----BEGIN PUBLIC KEY-----
        MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAvZ...
        -----END PUBLIC KEY-----

      hmac_key: "base64-encoded-hmac-secret"
```

## Message Format

Encrypted messages are transmitted as JSON with the following structure:

```json
{
  "algorithm": "aes-256-gcm",
  "nonce": "base64-encoded-nonce",
  "ciphertext": "base64-encoded-encrypted-data",
  "auth_tag": "base64-encoded-authentication-tag",
  "hmac": "base64-encoded-hmac",
  "signature": {
    "algorithm": "ed25519",
    "signature": "base64-encoded-signature",
    "key_id": "optional-key-rotation-id"
  },
  "timestamp": 1698765432
}
```

## Security Features

### 1. **Authenticated Encryption**
- AEAD algorithms (AES-GCM, ChaCha20-Poly1305) provide:
  - **Confidentiality**: Data is encrypted
  - **Integrity**: Tampering is detected
  - **Authenticity**: Origin is verified

### 2. **Defense in Depth (Optional HMAC)**
- Additional HMAC layer protects against:
  - Implementation vulnerabilities
  - Side-channel attacks
  - Cryptanalytic weaknesses

### 3. **Digital Signatures (Optional)**
- Provides:
  - **Non-repudiation**: Sender cannot deny sending
  - **Strong authentication**: Beyond AEAD authentication
  - **Key rotation**: Support for multiple signing keys

### 4. **Replay Attack Protection**
- Timestamp validation prevents:
  - Message replay
  - Time-based attacks
- Configurable `max_message_age` (default: 5 minutes)

### 5. **Perfect Forward Secrecy (PFS)**
- X25519 key exchange provides:
  - Session-specific encryption keys
  - Protection even if long-term keys are compromised

## Key Generation

### Generate AES-256 Key
```bash
openssl rand -base64 32
```

### Generate HMAC Secret
```bash
openssl rand -base64 32
```

### Generate Ed25519 Keys
```bash
# Private key
openssl genpkey -algorithm ED25519 -out ed25519_private.pem
openssl pkey -in ed25519_private.pem -pubout -out ed25519_public.pem

# Convert to base64 for configuration
base64 < ed25519_private.pem
base64 < ed25519_public.pem
```

### Generate X25519 Keys
```bash
# Private key
openssl genpkey -algorithm X25519 -out x25519_private.pem
openssl pkey -in x25519_private.pem -pubout -out x25519_public.pem

# Convert to base64
base64 < x25519_private.pem
base64 < x25519_public.pem
```

### Generate RSA Keys
```bash
# 4096-bit RSA keys
openssl genrsa -out rsa_private.pem 4096
openssl rsa -in rsa_private.pem -pubout -out rsa_public.pem
```

## Client Implementation Example

### JavaScript/TypeScript Client

```typescript
import nacl from 'tweetnacl';
import { Buffer } from 'buffer';

class SecureWebSocketClient {
  private ws: WebSocket;
  private encryptionKey: Uint8Array;
  private signingKey: nacl.SignKeyPair;

  constructor(url: string, encryptionKey: string, signingKey: string) {
    this.ws = new WebSocket(url);
    this.encryptionKey = Buffer.from(encryptionKey, 'base64');
    this.signingKey = nacl.sign.keyPair.fromSecretKey(
      Buffer.from(signingKey, 'base64')
    );

    this.ws.onmessage = (event) => {
      const encrypted = JSON.parse(event.data);
      const decrypted = this.decrypt(encrypted);
      this.handleMessage(decrypted);
    };
  }

  send(message: any) {
    const plaintext = JSON.stringify(message);
    const encrypted = this.encrypt(Buffer.from(plaintext, 'utf8'));
    this.ws.send(JSON.stringify(encrypted));
  }

  private encrypt(plaintext: Uint8Array): AuthenticatedMessage {
    const nonce = nacl.randomBytes(12);
    const timestamp = Math.floor(Date.now() / 1000);

    // Encrypt with ChaCha20-Poly1305
    const ciphertext = nacl.secretbox(plaintext, nonce, this.encryptionKey);

    // Sign
    const signData = Buffer.concat([
      nonce,
      ciphertext,
      Buffer.from(timestamp.toString())
    ]);
    const signature = nacl.sign.detached(signData, this.signingKey.secretKey);

    return {
      algorithm: 'chacha20-poly1305',
      nonce: Buffer.from(nonce).toString('base64'),
      ciphertext: Buffer.from(ciphertext).toString('base64'),
      auth_tag: null,
      hmac: null,
      signature: {
        algorithm: 'ed25519',
        signature: Buffer.from(signature).toString('base64'),
        key_id: null
      },
      timestamp
    };
  }

  private decrypt(message: AuthenticatedMessage): any {
    const nonce = Buffer.from(message.nonce, 'base64');
    const ciphertext = Buffer.from(message.ciphertext, 'base64');

    // Verify signature if present
    if (message.signature) {
      // Verify signature logic here
    }

    // Decrypt
    const plaintext = nacl.secretbox.open(
      ciphertext,
      nonce,
      this.encryptionKey
    );

    if (!plaintext) {
      throw new Error('Decryption failed');
    }

    return JSON.parse(Buffer.from(plaintext).toString('utf8'));
  }

  private handleMessage(message: any) {
    console.log('Received:', message);
  }
}

// Usage
const client = new SecureWebSocketClient(
  'ws://localhost:8080/ws/app',
  'base64-encoded-encryption-key',
  'base64-encoded-signing-key'
);

client.send({ action: 'subscribe', channel: 'updates' });
```

## Performance Considerations

### Algorithm Performance (Relative)
- **ChaCha20-Poly1305**: Fastest on software-only systems
- **AES-256-GCM**: Fastest with AES-NI hardware support
- **X25519 + ChaCha20**: Fast key exchange, then symmetric speed
- **RSA-OAEP**: Slowest, use only for key exchange

### Recommendations
- **High-throughput**: AES-256-GCM (on modern CPUs with AES-NI)
- **Mobile/IoT**: ChaCha20-Poly1305
- **Maximum security**: X25519 + ChaCha20 + Ed25519 signatures
- **Legacy compatibility**: RSA-OAEP + RSA-PSS

## Use Cases

### 1. Financial Trading Platforms
- **Config**: AES-256-GCM + HMAC + Ed25519
- **Why**: Maximum security with non-repudiation

### 2. IoT Device Communication
- **Config**: ChaCha20-Poly1305 (no HMAC, no signatures)
- **Why**: Resource-constrained devices, software-only crypto

### 3. Healthcare Applications (HIPAA)
- **Config**: AES-256-GCM + HMAC-SHA512 + RSA-PSS
- **Why**: Compliance, auditability, non-repudiation

### 4. Real-time Gaming
- **Config**: ChaCha20-Poly1305 (minimal overhead)
- **Why**: Low latency, high throughput

### 5. Enterprise Chat Applications
- **Config**: X25519 + ChaCha20 + Ed25519
- **Why**: Modern security, perfect forward secrecy

## Troubleshooting

### Error: "Message expired: possible replay attack"
- **Cause**: Message timestamp is older than `max_message_age`
- **Fix**: Check client/server clock synchronization (NTP)
- **Fix**: Increase `max_message_age` if needed

### Error: "HMAC verification failed"
- **Cause**: Wrong HMAC key or message tampering
- **Fix**: Verify HMAC key matches on client and proxy
- **Fix**: Check for network issues corrupting messages

### Error: "Signature verification failed"
- **Cause**: Wrong public key or signature algorithm mismatch
- **Fix**: Verify signing keys are correctly configured
- **Fix**: Ensure algorithm matches (Ed25519 vs RSA-PSS)

### Error: "Decryption failed"
- **Cause**: Wrong encryption key or corrupted message
- **Fix**: Verify encryption key matches on client and proxy
- **Fix**: Check nonce and ciphertext are transmitted correctly

## Security Best Practices

1. **Key Management**
   - Rotate keys regularly (every 90 days recommended)
   - Store keys securely (use key management systems)
   - Never commit keys to version control

2. **Algorithm Selection**
   - Prefer AEAD algorithms (AES-GCM, ChaCha20-Poly1305)
   - Use Ed25519 over RSA-PSS for signatures
   - Enable HMAC for critical applications

3. **Configuration**
   - Set appropriate `max_message_age` (300 seconds default)
   - Use TLS for the WebSocket connection in addition to message encryption
   - Enable signatures for applications requiring non-repudiation

4. **Monitoring**
   - Log decryption failures
   - Alert on replay attack attempts
   - Monitor for unusual traffic patterns

## Deployment Scenarios

### Scenario 1: Single Server
```yaml
# All encryption happens at the proxy
websocket:
  encryption:
    algorithm: aes-256-gcm
    keys:
      encryption_key: "..."
```

### Scenario 2: Distributed (Multi-Instance)
```yaml
# All proxy instances share the same keys
# Keys must be distributed securely via config management
websocket:
  encryption:
    algorithm: aes-256-gcm
    keys:
      encryption_key: "{{ vault.websocket.encryption_key }}"
      hmac_key: "{{ vault.websocket.hmac_key }}"
```

### Scenario 3: Per-Route Encryption
```yaml
routes:
  - name: "public_ws"
    match:
      paths: ["/ws/public"]
    upstream: "backend1"
    websocket:
      encryption: null  # No encryption

  - name: "secure_ws"
    match:
      paths: ["/ws/secure"]
    upstream: "backend2"
    websocket:
      encryption:
        algorithm: x25519_chacha20
        keys: { ... }
```

## Future Enhancements

- [ ] Support for key rotation without downtime
- [ ] Integration with Hardware Security Modules (HSMs)
- [ ] Support for TLS 1.3 0-RTT for WebSocket upgrades
- [ ] Quantum-resistant algorithms (post-quantum cryptography)
- [ ] Certificate-based authentication (mTLS for WebSockets)

## References

- [RFC 6455](https://tools.ietf.org/html/rfc6455) - The WebSocket Protocol
- [RFC 5288](https://tools.ietf.org/html/rfc5288) - AES-GCM Cipher Suites
- [RFC 8439](https://tools.ietf.org/html/rfc8439) - ChaCha20-Poly1305
- [RFC 7748](https://tools.ietf.org/html/rfc7748) - Elliptic Curves (X25519, Ed25519)
- [RFC 8017](https://tools.ietf.org/html/rfc8017) - RSA Cryptography Specifications

---

**Note**: This feature provides application-layer encryption for WebSocket messages. For maximum security, always use TLS (wss://) for the WebSocket connection in addition to message-level encryption.

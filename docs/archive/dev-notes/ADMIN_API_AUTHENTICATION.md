# Admin API Authentication

The Admin API now supports JWT-based authentication with a SQLite user database.

## Features

- **SQLite User Database**: Persistent storage for user accounts
- **Dual Password Hashing**: Configurable support for **Bcrypt** (default) and **Argon2id** algorithms
- **Automatic Algorithm Detection**: Verifies passwords regardless of hash format
- **JWT Token Authentication**: Stateless authentication with HS256 algorithm
- **Role-Based Access Control**: Users have roles (admin, viewer, etc.)
- **User Management API**: Create, list, and delete users

## API Endpoints

### Authentication

#### POST /api/auth/login
Login with username and password to obtain a JWT token.

**Request:**
```json
{
  "username": "admin",
  "password": "secure_password"
}
```

**Response:**
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "expires_in": 86400,
  "user": {
    "username": "admin",
    "role": "admin"
  }
}
```

### User Management

#### GET /api/users
List all users (requires authentication).

**Response:**
```json
{
  "users": [
    {
      "id": 1,
      "username": "admin",
      "role": "admin",
      "created_at": 1234567890,
      "enabled": true
    }
  ],
  "count": 1
}
```

#### POST /api/users
Create a new user (requires authentication).

**Request:**
```json
{
  "username": "newuser",
  "password": "secure_password",
  "role": "viewer"
}
```

**Response:**
```json
{
  "id": 2,
  "username": "newuser",
  "role": "viewer",
  "created_at": 1234567890,
  "enabled": true
}
```

#### DELETE /api/users/{username}
Delete a user (requires authentication).

**Response:**
```json
{
  "message": "User deleted successfully",
  "username": "newuser"
}
```

## Configuration

To enable authentication, initialize the `AuthDb` and attach it to the `AdminServer`:

### Using Default Bcrypt Algorithm

```rust
use rust_proxy::admin::auth::AuthDb;
use std::sync::Arc;

// Initialize auth database with default bcrypt hashing
let auth_db = AuthDb::new(
    "sqlite:admin_users.db",      // Database path
    "your-secret-key-here",        // JWT secret
    "24h"                          // Token expiration (24h, 7d, 3600s, etc.)
).await?;

// Attach to admin server
let admin_server = AdminServer::new(config, proxy_config)
    .with_auth_db(Arc::new(auth_db));
```

### Using Argon2id Algorithm (Recommended for New Deployments)

```rust
use rust_proxy::admin::auth::{AuthDb, PasswordHashAlgorithm};
use std::sync::Arc;

// Initialize auth database with Argon2id hashing
let auth_db = AuthDb::new_with_hash_algorithm(
    "sqlite:admin_users.db",           // Database path
    "your-secret-key-here",             // JWT secret
    "24h",                              // Token expiration
    PasswordHashAlgorithm::Argon2id     // Use Argon2id
).await?;

// Attach to admin server
let admin_server = AdminServer::new(config, proxy_config)
    .with_auth_db(Arc::new(auth_db));
```

## JWT Token Expiration

The JWT expiration time can be specified in various formats:
- `"3600s"` - 3600 seconds (1 hour)
- `"60m"` - 60 minutes (1 hour)
- `"1h"` - 1 hour
- `"24h"` - 24 hours (1 day)
- `"7d"` - 7 days

## Using JWT Tokens

Once you have a token, include it in the `Authorization` header for all API requests:

```bash
# Login
curl -X POST http://localhost:8081/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"secure_password"}'

# Use token for authenticated requests
curl http://localhost:8081/api/users \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

## Security Notes

1. **JWT Secret**: Use a strong, randomly generated secret key (minimum 32 bytes)
2. **HTTPS**: Always use HTTPS in production to protect JWT tokens in transit
3. **Token Expiration**: Set appropriate expiration times based on your security requirements
4. **Password Requirements**: Enforce strong password policies in your application
5. **Database Security**: Secure the SQLite database file with appropriate file permissions (e.g., `chmod 600`)
6. **Algorithm Choice**:
   - Use **Argon2id** for new deployments (best resistance to modern attacks)
   - Use **Bcrypt** for compatibility with existing systems
   - Both algorithms provide strong security when properly configured
7. **Hash Migration**: Existing bcrypt hashes will continue to work when switching to Argon2id

## Database Schema

The SQLite database contains a `users` table:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'viewer',
    created_at INTEGER NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1
)
```

## Password Hashing Algorithms

The authentication system supports two industry-standard password hashing algorithms:

### Bcrypt (Default)
- **Cost Factor**: 12 rounds (default)
- **Use Case**: Widely supported, proven track record
- **Security**: Strong protection against brute-force attacks
- **Performance**: Slower than plain hashing (by design)
- **Hash Format**: `$2a$12$...` or `$2b$12$...`

### Argon2id (Recommended for New Deployments)
- **Algorithm**: Argon2id variant (hybrid of Argon2i and Argon2d)
- **Use Case**: Modern deployments requiring highest security
- **Security**: Winner of Password Hashing Competition (2015)
- **Features**:
  - Memory-hard algorithm (resistant to GPU/ASIC attacks)
  - Protection against side-channel attacks
  - Configurable memory and time costs
- **Hash Format**: `$argon2id$v=19$m=...`

### Automatic Algorithm Detection

The verification system automatically detects which algorithm was used to hash a password by inspecting the hash prefix:
- Hashes starting with `$2` are verified using Bcrypt
- Hashes starting with `$argon2` are verified using Argon2

This allows for **seamless migration** from Bcrypt to Argon2id:
1. Configure new `AuthDb` with Argon2id algorithm
2. Existing users with Bcrypt hashes can still log in
3. New users and password changes will use Argon2id
4. No downtime or data migration required

## Implementation Details

- **Password Hashing**:
  - Bcrypt: DEFAULT_COST (12 rounds)
  - Argon2id: Default parameters (memory cost, parallelism)
- **Salt Generation**: Cryptographically secure random salt for each password
- **JWT Algorithm**: HS256 (HMAC with SHA-256)
- **Token Claims**: Includes `sub` (username), `iat` (issued at), `exp` (expiration), and `role`
- **Login Endpoint**: Does NOT require authentication (public endpoint)
- **All Other Endpoints**: Require valid JWT token when `auth_enabled` is true

## Error Responses

Authentication errors return appropriate HTTP status codes:

- `401 Unauthorized`: Invalid credentials or missing/invalid token
- `403 Forbidden`: User disabled or insufficient permissions
- `404 Not Found`: User not found
- `409 Conflict`: Username already exists
- `503 Service Unavailable`: Authentication database not configured

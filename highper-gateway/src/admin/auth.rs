//! Admin API authentication with SQLite user database
//!
//! Provides JWT token generation, user management, and password hashing.
//! Supports both Bcrypt and Argon2id hashing algorithms.

use bytes::Bytes;
use http_body_util::Full;
use hyper::{Response, StatusCode};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Row};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Password hashing algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PasswordHashAlgorithm {
    /// Bcrypt (default, widely supported)
    Bcrypt,
    /// Argon2id (modern, more secure)
    Argon2id,
}

impl Default for PasswordHashAlgorithm {
    fn default() -> Self {
        Self::Bcrypt
    }
}

impl std::fmt::Display for PasswordHashAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bcrypt => write!(f, "bcrypt"),
            Self::Argon2id => write!(f, "argon2id"),
        }
    }
}

impl std::str::FromStr for PasswordHashAlgorithm {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bcrypt" => Ok(Self::Bcrypt),
            "argon2id" | "argon2" => Ok(Self::Argon2id),
            _ => Err(format!("Unknown password hash algorithm: {}", s)),
        }
    }
}

/// JWT claims for admin API authentication
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject (user ID or username)
    pub sub: String,
    /// Issued at (Unix timestamp)
    pub iat: usize,
    /// Expiration time (Unix timestamp)
    pub exp: usize,
    /// User role
    pub role: String,
}

/// User in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub created_at: i64,
    pub enabled: bool,
}

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_in: usize,
    pub user: UserInfo,
}

/// User info (without sensitive data)
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub username: String,
    pub role: String,
}

/// Create user request
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String,
}

/// Account lockout configuration
#[derive(Debug, Clone)]
pub struct LockoutConfig {
    /// Maximum failed login attempts before lockout
    pub max_attempts: u32,
    /// Lockout duration in seconds
    pub lockout_duration_seconds: u64,
}

impl Default for LockoutConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            lockout_duration_seconds: 900, // 15 minutes
        }
    }
}

/// Authentication database
pub struct AuthDb {
    pool: Arc<SqlitePool>,
    jwt_secret: String,
    jwt_expiration_seconds: usize,
    hash_algorithm: PasswordHashAlgorithm,
    lockout_config: LockoutConfig,
}

impl AuthDb {
    /// Create new auth database with default settings (bcrypt)
    pub async fn new(db_path: &str, jwt_secret: String, jwt_expiration: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_hash_algorithm(db_path, jwt_secret, jwt_expiration, PasswordHashAlgorithm::default()).await
    }

    /// Create new auth database with configurable hash algorithm
    pub async fn new_with_hash_algorithm(
        db_path: &str,
        jwt_secret: String,
        jwt_expiration: &str,
        hash_algorithm: PasswordHashAlgorithm,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_lockout(db_path, jwt_secret, jwt_expiration, hash_algorithm, LockoutConfig::default()).await
    }

    /// Create new auth database with configurable hash algorithm and lockout settings
    pub async fn new_with_lockout(
        db_path: &str,
        jwt_secret: String,
        jwt_expiration: &str,
        hash_algorithm: PasswordHashAlgorithm,
        lockout_config: LockoutConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing authentication database: {} (hash algorithm: {}, lockout: {} attempts / {}s)",
              db_path, hash_algorithm, lockout_config.max_attempts, lockout_config.lockout_duration_seconds);

        // Create SQLite connection pool
        let pool = SqlitePool::connect(db_path).await?;

        // Create users table if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'viewer',
                created_at INTEGER NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                failed_attempts INTEGER NOT NULL DEFAULT 0,
                locked_until INTEGER
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Add lockout columns to existing tables (migration)
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN failed_attempts INTEGER NOT NULL DEFAULT 0")
            .execute(&pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN locked_until INTEGER")
            .execute(&pool)
            .await;

        info!("Users table initialized successfully with lockout support");

        // Parse JWT expiration (e.g., "24h", "7d", "3600s")
        let jwt_expiration_seconds = parse_duration(jwt_expiration)?;

        Ok(Self {
            pool: Arc::new(pool),
            jwt_secret,
            jwt_expiration_seconds,
            hash_algorithm,
            lockout_config,
        })
    }

    /// Authenticate user and generate JWT token
    pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse, AuthError> {
        debug!("Login attempt for user: {}", req.username);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Fetch user from database with lockout info
        let row = sqlx::query(
            "SELECT id, username, password_hash, role, created_at, enabled, failed_attempts, locked_until FROM users WHERE username = ?"
        )
        .bind(&req.username)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| AuthError::NotFound("User not found".to_string()))?;

        let user = User {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            role: row.get("role"),
            created_at: row.get("created_at"),
            enabled: row.get::<i64, _>("enabled") == 1,
        };

        let failed_attempts: i64 = row.get("failed_attempts");
        let locked_until: Option<i64> = row.get("locked_until");

        // Check if account is locked
        if let Some(until) = locked_until {
            if until > now {
                let remaining = (until - now) as u64;
                warn!("Login attempt for locked user: {} (locked for {} more seconds)", req.username, remaining);
                return Err(AuthError::Locked {
                    until,
                    remaining_seconds: remaining,
                });
            }
        }

        // Check if user is enabled
        if !user.enabled {
            warn!("Login attempt for disabled user: {}", req.username);
            return Err(AuthError::Unauthorized("User is disabled".to_string()));
        }

        // Verify password (auto-detect algorithm from hash format)
        if !verify_password(&req.password, &user.password_hash, self.hash_algorithm)? {
            // Increment failed attempts
            let new_attempts = failed_attempts + 1;
            let lock_until = if new_attempts >= self.lockout_config.max_attempts as i64 {
                Some(now + self.lockout_config.lockout_duration_seconds as i64)
            } else {
                None
            };

            sqlx::query("UPDATE users SET failed_attempts = ?, locked_until = ? WHERE username = ?")
                .bind(new_attempts)
                .bind(lock_until)
                .bind(&req.username)
                .execute(&*self.pool)
                .await
                .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?;

            if let Some(until) = lock_until {
                warn!("User {} locked after {} failed attempts", req.username, new_attempts);
                return Err(AuthError::Locked {
                    until,
                    remaining_seconds: self.lockout_config.lockout_duration_seconds,
                });
            }

            warn!("Failed login attempt for user: {} ({}/{} attempts)",
                  req.username, new_attempts, self.lockout_config.max_attempts);
            return Err(AuthError::Unauthorized("Invalid credentials".to_string()));
        }

        // Reset failed attempts on successful login
        sqlx::query("UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE username = ?")
            .bind(&req.username)
            .execute(&*self.pool)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?;

        // Generate JWT token
        let claims = JwtClaims {
            sub: user.username.clone(),
            iat: now as usize,
            exp: now as usize + self.jwt_expiration_seconds,
            role: user.role.clone(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| AuthError::Internal(format!("Failed to generate token: {}", e)))?;

        info!("User logged in successfully: {}", user.username);

        Ok(LoginResponse {
            token,
            expires_in: self.jwt_expiration_seconds,
            user: UserInfo {
                username: user.username,
                role: user.role,
            },
        })
    }

    /// Unlock a locked user account
    pub async fn unlock_user(&self, username: &str) -> Result<(), AuthError> {
        info!("Unlocking user: {}", username);

        let result = sqlx::query("UPDATE users SET failed_attempts = 0, locked_until = NULL WHERE username = ?")
            .bind(username)
            .execute(&*self.pool)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AuthError::NotFound("User not found".to_string()));
        }

        info!("User unlocked successfully: {}", username);
        Ok(())
    }

    /// Create a new user
    pub async fn create_user(&self, req: CreateUserRequest) -> Result<User, AuthError> {
        info!("Creating new user: {} (hash algorithm: {})", req.username, self.hash_algorithm);

        // Hash the password using configured algorithm
        let password_hash = hash_password(&req.password, self.hash_algorithm)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Insert user into database
        let result = sqlx::query(
            r#"
            INSERT INTO users (username, password_hash, role, created_at, enabled)
            VALUES (?, ?, ?, ?, 1)
            "#,
        )
        .bind(&req.username)
        .bind(&password_hash)
        .bind(&req.role)
        .bind(now)
        .execute(&*self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("UNIQUE constraint failed") {
                AuthError::Conflict("Username already exists".to_string())
            } else {
                AuthError::Internal(format!("Database error: {}", e))
            }
        })?;

        info!("User created successfully: {}", req.username);

        Ok(User {
            id: result.last_insert_rowid(),
            username: req.username,
            password_hash,
            role: req.role,
            created_at: now,
            enabled: true,
        })
    }

    /// Get user by username
    async fn get_user_by_username(&self, username: &str) -> Result<User, AuthError> {
        let row = sqlx::query("SELECT id, username, password_hash, role, created_at, enabled FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&*self.pool)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?
            .ok_or_else(|| AuthError::NotFound("User not found".to_string()))?;

        Ok(User {
            id: row.get("id"),
            username: row.get("username"),
            password_hash: row.get("password_hash"),
            role: row.get("role"),
            created_at: row.get("created_at"),
            enabled: row.get::<i64, _>("enabled") == 1,
        })
    }

    /// List all users
    pub async fn list_users(&self) -> Result<Vec<User>, AuthError> {
        let rows = sqlx::query("SELECT id, username, password_hash, role, created_at, enabled FROM users ORDER BY created_at DESC")
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?;

        Ok(rows
            .into_iter()
            .map(|row| User {
                id: row.get("id"),
                username: row.get("username"),
                password_hash: row.get("password_hash"),
                role: row.get("role"),
                created_at: row.get("created_at"),
                enabled: row.get::<i64, _>("enabled") == 1,
            })
            .collect())
    }

    /// Delete a user
    pub async fn delete_user(&self, username: &str) -> Result<(), AuthError> {
        info!("Deleting user: {}", username);

        let result = sqlx::query("DELETE FROM users WHERE username = ?")
            .bind(username)
            .execute(&*self.pool)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AuthError::NotFound("User not found".to_string()));
        }

        info!("User deleted successfully: {}", username);
        Ok(())
    }

    /// Enable/disable a user
    pub async fn set_user_enabled(&self, username: &str, enabled: bool) -> Result<(), AuthError> {
        info!("Setting user {} enabled status to: {}", username, enabled);

        let result = sqlx::query("UPDATE users SET enabled = ? WHERE username = ?")
            .bind(if enabled { 1 } else { 0 })
            .bind(username)
            .execute(&*self.pool)
            .await
            .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AuthError::NotFound("User not found".to_string()));
        }

        Ok(())
    }
}

/// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    Unauthorized(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
    /// Account is locked due to too many failed attempts
    Locked { until: i64, remaining_seconds: u64 },
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AuthError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AuthError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            AuthError::Internal(msg) => write!(f, "Internal error: {}", msg),
            AuthError::Locked { remaining_seconds, .. } => {
                write!(f, "Account locked. Try again in {} seconds", remaining_seconds)
            }
        }
    }
}

impl std::error::Error for AuthError {}

impl From<AuthError> for Response<Full<Bytes>> {
    fn from(err: AuthError) -> Self {
        let (status, body) = match err {
            AuthError::Unauthorized(msg) => (
                StatusCode::UNAUTHORIZED,
                serde_json::json!({ "error": msg }),
            ),
            AuthError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                serde_json::json!({ "error": msg }),
            ),
            AuthError::Conflict(msg) => (
                StatusCode::CONFLICT,
                serde_json::json!({ "error": msg }),
            ),
            AuthError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({ "error": msg }),
            ),
            AuthError::Locked { until, remaining_seconds } => (
                StatusCode::TOO_MANY_REQUESTS,
                serde_json::json!({
                    "error": format!("Account locked. Try again in {} seconds", remaining_seconds),
                    "locked_until": until,
                    "retry_after": remaining_seconds
                }),
            ),
        };

        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(body.to_string())))
            .unwrap()
    }
}

/// Hash a password using the specified algorithm
fn hash_password(password: &str, algorithm: PasswordHashAlgorithm) -> Result<String, AuthError> {
    match algorithm {
        PasswordHashAlgorithm::Bcrypt => {
            bcrypt::hash(password, bcrypt::DEFAULT_COST)
                .map_err(|e| AuthError::Internal(format!("Bcrypt hashing failed: {}", e)))
        }
        PasswordHashAlgorithm::Argon2id => {
            use argon2::{
                password_hash::{PasswordHasher, SaltString},
                Argon2,
            };

            // Generate a random salt
            let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);

            // Hash the password with Argon2id
            let argon2 = Argon2::default();
            let password_hash = argon2
                .hash_password(password.as_bytes(), &salt)
                .map_err(|e| AuthError::Internal(format!("Argon2id hashing failed: {}", e)))?;

            Ok(password_hash.to_string())
        }
    }
}

/// Verify a password against a hash (auto-detects algorithm from hash format)
fn verify_password(password: &str, hash: &str, _preferred_algorithm: PasswordHashAlgorithm) -> Result<bool, AuthError> {
    // Auto-detect algorithm from hash prefix
    if hash.starts_with("$2") {
        // Bcrypt hash format: $2a$, $2b$, $2y$
        bcrypt::verify(password, hash)
            .map_err(|e| AuthError::Internal(format!("Bcrypt verification failed: {}", e)))
    } else if hash.starts_with("$argon2") {
        // Argon2 hash format: $argon2id$, $argon2i$, $argon2d$
        use argon2::{
            password_hash::{PasswordHash, PasswordVerifier},
            Argon2,
        };

        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AuthError::Internal(format!("Invalid Argon2 hash format: {}", e)))?;

        let argon2 = Argon2::default();
        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(_) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(AuthError::Internal(format!("Argon2 verification failed: {}", e))),
        }
    } else {
        Err(AuthError::Internal(format!("Unknown password hash format")))
    }
}

/// Parse duration string (e.g., "24h", "7d", "3600s")
fn parse_duration(duration: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let duration = duration.trim();

    if duration.ends_with('h') {
        let hours: usize = duration[..duration.len() - 1].parse()?;
        Ok(hours * 3600)
    } else if duration.ends_with('d') {
        let days: usize = duration[..duration.len() - 1].parse()?;
        Ok(days * 86400)
    } else if duration.ends_with('s') {
        let seconds: usize = duration[..duration.len() - 1].parse()?;
        Ok(seconds)
    } else if duration.ends_with('m') {
        let minutes: usize = duration[..duration.len() - 1].parse()?;
        Ok(minutes * 60)
    } else {
        // Assume seconds if no suffix
        Ok(duration.parse()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("3600s").unwrap(), 3600);
        assert_eq!(parse_duration("60m").unwrap(), 3600);
        assert_eq!(parse_duration("1h").unwrap(), 3600);
        assert_eq!(parse_duration("1d").unwrap(), 86400);
        assert_eq!(parse_duration("24h").unwrap(), 86400);
    }

    #[test]
    fn test_password_hashing_bcrypt() {
        let password = "test_password_123";
        let hash = hash_password(password, PasswordHashAlgorithm::Bcrypt).unwrap();

        // Verify hash starts with bcrypt prefix
        assert!(hash.starts_with("$2"));

        // Verify password
        assert!(verify_password(password, &hash, PasswordHashAlgorithm::Bcrypt).unwrap());
        assert!(!verify_password("wrong_password", &hash, PasswordHashAlgorithm::Bcrypt).unwrap());
    }

    #[test]
    fn test_password_hashing_argon2id() {
        let password = "test_password_123";
        let hash = hash_password(password, PasswordHashAlgorithm::Argon2id).unwrap();

        // Verify hash starts with argon2 prefix
        assert!(hash.starts_with("$argon2"));

        // Verify password
        assert!(verify_password(password, &hash, PasswordHashAlgorithm::Argon2id).unwrap());
        assert!(!verify_password("wrong_password", &hash, PasswordHashAlgorithm::Argon2id).unwrap());
    }

    #[test]
    fn test_password_algorithm_auto_detection() {
        let password = "test_password_123";

        // Test bcrypt
        let bcrypt_hash = hash_password(password, PasswordHashAlgorithm::Bcrypt).unwrap();
        assert!(verify_password(password, &bcrypt_hash, PasswordHashAlgorithm::Argon2id).unwrap());

        // Test argon2id
        let argon2_hash = hash_password(password, PasswordHashAlgorithm::Argon2id).unwrap();
        assert!(verify_password(password, &argon2_hash, PasswordHashAlgorithm::Bcrypt).unwrap());
    }

    #[test]
    fn test_hash_algorithm_from_str() {
        assert_eq!("bcrypt".parse::<PasswordHashAlgorithm>().unwrap(), PasswordHashAlgorithm::Bcrypt);
        assert_eq!("argon2id".parse::<PasswordHashAlgorithm>().unwrap(), PasswordHashAlgorithm::Argon2id);
        assert_eq!("argon2".parse::<PasswordHashAlgorithm>().unwrap(), PasswordHashAlgorithm::Argon2id);
        assert!("unknown".parse::<PasswordHashAlgorithm>().is_err());
    }
}

//! Socket configuration for kernel TLS
//!
//! This module provides low-level socket configuration for enabling kTLS

use super::{CipherSuite, TlsVersion};
use std::io;
use std::os::unix::io::AsRawFd;
use tracing::debug;

#[cfg(target_os = "linux")]
use libc::{c_int, c_void, setsockopt, socklen_t, SOL_TLS, TLS_RX, TLS_TX};

#[cfg(target_os = "linux")]
const TCP_ULP: c_int = 31;

/// Crypto information for kTLS (matches kernel struct tls_crypto_info)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct TlsCryptoInfo {
    pub version: u16,
    pub cipher_type: u16,
}

/// AES-GCM-128 crypto info (matches kernel struct tls12_crypto_info_aes_gcm_128)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Tls12CryptoInfoAesGcm128 {
    pub info: TlsCryptoInfo,
    pub iv: [u8; 8],
    pub key: [u8; 16],
    pub salt: [u8; 4],
    pub rec_seq: [u8; 8],
}

/// AES-GCM-256 crypto info
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Tls12CryptoInfoAesGcm256 {
    pub info: TlsCryptoInfo,
    pub iv: [u8; 8],
    pub key: [u8; 32],
    pub salt: [u8; 4],
    pub rec_seq: [u8; 8],
}

/// ChaCha20-Poly1305 crypto info
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Tls12CryptoInfoChaCha20Poly1305 {
    pub info: TlsCryptoInfo,
    pub iv: [u8; 12],
    pub key: [u8; 32],
    pub salt: [u8; 4],
    pub rec_seq: [u8; 8],
}

/// Session keys extracted from TLS connection
#[derive(Debug, Clone)]
pub struct SessionKeys {
    pub version: TlsVersion,
    pub cipher_suite: CipherSuite,
    pub key: Vec<u8>,
    pub iv: Vec<u8>,
    pub salt: Vec<u8>,
    pub seq_num: u64,
}

/// Enable kTLS on a socket
#[cfg(target_os = "linux")]
pub fn enable_ktls_on_socket<S: AsRawFd>(
    socket: &S,
    keys: &SessionKeys,
    direction: Direction,
) -> io::Result<()> {
    let fd = socket.as_raw_fd();

    // Step 1: Set TCP ULP (Upper Layer Protocol) to "tls"
    enable_tcp_ulp(fd)?;

    // Step 2: Configure crypto parameters
    match (keys.cipher_suite, keys.version) {
        (CipherSuite::Aes128Gcm, _) => {
            configure_aes_gcm_128(fd, keys, direction)?;
        }
        (CipherSuite::Aes256Gcm, _) => {
            configure_aes_gcm_256(fd, keys, direction)?;
        }
        (CipherSuite::Chacha20Poly1305, _) => {
            configure_chacha20_poly1305(fd, keys, direction)?;
        }
    }

    debug!(
        "kTLS enabled for {:?} direction with cipher: {:?}",
        direction, keys.cipher_suite
    );

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn enable_ktls_on_socket<S: AsRawFd>(
    _socket: &S,
    _keys: &SessionKeys,
    _direction: Direction,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "kTLS is only supported on Linux",
    ))
}

/// Direction for kTLS (transmit or receive)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Transmit (send)
    Tx,
    /// Receive (recv)
    Rx,
}

#[cfg(target_os = "linux")]
fn enable_tcp_ulp(fd: c_int) -> io::Result<()> {
    let ulp_name = b"tls\0";
    let result = unsafe {
        setsockopt(
            fd,
            libc::SOL_TCP,
            TCP_ULP,
            ulp_name.as_ptr() as *const c_void,
            ulp_name.len() as socklen_t,
        )
    };

    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn configure_aes_gcm_128(fd: c_int, keys: &SessionKeys, direction: Direction) -> io::Result<()> {
    if keys.key.len() != 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AES-128-GCM requires 16-byte key",
        ));
    }

    let mut crypto_info = Tls12CryptoInfoAesGcm128 {
        info: TlsCryptoInfo {
            version: keys.version.protocol_version(),
            cipher_type: keys.cipher_suite.cipher_type(),
        },
        iv: [0u8; 8],
        key: [0u8; 16],
        salt: [0u8; 4],
        rec_seq: [0u8; 8],
    };

    // Copy keys
    crypto_info.key[..keys.key.len()].copy_from_slice(&keys.key);
    crypto_info.iv[..keys.iv.len().min(8)].copy_from_slice(&keys.iv[..keys.iv.len().min(8)]);
    crypto_info.salt[..keys.salt.len().min(4)]
        .copy_from_slice(&keys.salt[..keys.salt.len().min(4)]);

    // Convert sequence number to big-endian bytes
    let seq_bytes = keys.seq_num.to_be_bytes();
    crypto_info.rec_seq.copy_from_slice(&seq_bytes);

    let opt_name = match direction {
        Direction::Tx => TLS_TX,
        Direction::Rx => TLS_RX,
    };

    let result = unsafe {
        setsockopt(
            fd,
            SOL_TLS,
            opt_name,
            &crypto_info as *const _ as *const c_void,
            std::mem::size_of::<Tls12CryptoInfoAesGcm128>() as socklen_t,
        )
    };

    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn configure_aes_gcm_256(fd: c_int, keys: &SessionKeys, direction: Direction) -> io::Result<()> {
    if keys.key.len() != 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AES-256-GCM requires 32-byte key",
        ));
    }

    let mut crypto_info = Tls12CryptoInfoAesGcm256 {
        info: TlsCryptoInfo {
            version: keys.version.protocol_version(),
            cipher_type: keys.cipher_suite.cipher_type(),
        },
        iv: [0u8; 8],
        key: [0u8; 32],
        salt: [0u8; 4],
        rec_seq: [0u8; 8],
    };

    crypto_info.key.copy_from_slice(&keys.key);
    crypto_info.iv[..keys.iv.len().min(8)].copy_from_slice(&keys.iv[..keys.iv.len().min(8)]);
    crypto_info.salt[..keys.salt.len().min(4)]
        .copy_from_slice(&keys.salt[..keys.salt.len().min(4)]);

    let seq_bytes = keys.seq_num.to_be_bytes();
    crypto_info.rec_seq.copy_from_slice(&seq_bytes);

    let opt_name = match direction {
        Direction::Tx => TLS_TX,
        Direction::Rx => TLS_RX,
    };

    let result = unsafe {
        setsockopt(
            fd,
            SOL_TLS,
            opt_name,
            &crypto_info as *const _ as *const c_void,
            std::mem::size_of::<Tls12CryptoInfoAesGcm256>() as socklen_t,
        )
    };

    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn configure_chacha20_poly1305(
    fd: c_int,
    keys: &SessionKeys,
    direction: Direction,
) -> io::Result<()> {
    if keys.key.len() != 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ChaCha20-Poly1305 requires 32-byte key",
        ));
    }

    let mut crypto_info = Tls12CryptoInfoChaCha20Poly1305 {
        info: TlsCryptoInfo {
            version: keys.version.protocol_version(),
            cipher_type: keys.cipher_suite.cipher_type(),
        },
        iv: [0u8; 12],
        key: [0u8; 32],
        salt: [0u8; 4],
        rec_seq: [0u8; 8],
    };

    crypto_info.key.copy_from_slice(&keys.key);
    crypto_info.iv[..keys.iv.len().min(12)].copy_from_slice(&keys.iv[..keys.iv.len().min(12)]);
    crypto_info.salt[..keys.salt.len().min(4)]
        .copy_from_slice(&keys.salt[..keys.salt.len().min(4)]);

    let seq_bytes = keys.seq_num.to_be_bytes();
    crypto_info.rec_seq.copy_from_slice(&seq_bytes);

    let opt_name = match direction {
        Direction::Tx => TLS_TX,
        Direction::Rx => TLS_RX,
    };

    let result = unsafe {
        setsockopt(
            fd,
            SOL_TLS,
            opt_name,
            &crypto_info as *const _ as *const c_void,
            std::mem::size_of::<Tls12CryptoInfoChaCha20Poly1305>() as socklen_t,
        )
    };

    if result < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_keys_creation() {
        let keys = SessionKeys {
            version: TlsVersion::Tls13,
            cipher_suite: CipherSuite::Aes256Gcm,
            key: vec![0u8; 32],
            iv: vec![0u8; 8],
            salt: vec![0u8; 4],
            seq_num: 0,
        };

        assert_eq!(keys.key.len(), 32);
        assert_eq!(keys.iv.len(), 8);
        assert_eq!(keys.salt.len(), 4);
    }

    #[test]
    fn test_direction() {
        assert_eq!(Direction::Tx, Direction::Tx);
        assert_ne!(Direction::Tx, Direction::Rx);
    }
}

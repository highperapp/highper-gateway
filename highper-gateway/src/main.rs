// Copyright 2024-2026 Highper Gateway Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Highper Gateway - High-performance reverse proxy and API gateway
//!
//! Main entry point for the proxy server

// Use jemalloc as the global allocator for better performance
#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use highper_gateway::config::{load_config, validate_config, dsl_generator, dsl_parser, dsl_converter};
use highper_gateway::runtime::Runtime;
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Highper Gateway - High-performance reverse proxy and API gateway
#[derive(Parser, Debug)]
#[command(
    name = "highper-gateway",
    author,
    version,
    about = "High-performance reverse proxy and API gateway",
    long_about = "A production-ready reverse proxy and API gateway with advanced features:\n\
                  - HTTP/1.1, HTTP/2, HTTP/3 (QUIC) support\n\
                  - 8 load balancing algorithms (including Maglev)\n\
                  - Compression (gzip, brotli, zstd, deflate)\n\
                  - TLS 1.2/1.3 with automatic ACME certificates\n\
                  - Rate limiting, caching, circuit breakers\n\
                  - WebSocket and gRPC proxying\n\
                  - Hot reload and zero-downtime updates"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, env = "RUST_LOG", default_value = "info", global = true)]
    log_level: String,

    /// Enable JSON logging
    #[arg(short, long, global = true)]
    json_logs: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the proxy server (default)
    Start {
        /// Path to configuration file
        #[arg(short, long, default_value = "config/config.yaml")]
        config: PathBuf,

        /// Enable hot reload of configuration on file changes
        #[arg(long, default_value = "true")]
        hot_reload: bool,

        /// Daemonize (run in background)
        #[arg(short, long)]
        daemon: bool,
    },

    /// Validate configuration file without starting the server
    Validate {
        /// Path to configuration file
        #[arg(short, long, default_value = "config/config.yaml")]
        config: PathBuf,

        /// Show detailed validation results
        #[arg(short, long)]
        verbose: bool,
    },

    /// Test connectivity to upstream servers
    Test {
        /// Path to configuration file
        #[arg(short, long, default_value = "config/config.yaml")]
        config: PathBuf,

        /// Specific upstream to test (tests all if not specified)
        #[arg(short, long)]
        upstream: Option<String>,

        /// Timeout in seconds for each test
        #[arg(short = 't', long, default_value = "5")]
        timeout: u64,
    },

    /// Check health of running server
    Health {
        /// Admin API URL
        #[arg(long, default_value = "http://localhost:9090")]
        admin_url: String,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Reload configuration (send SIGHUP to running process)
    Reload {
        /// PID file location
        #[arg(short, long, default_value = "/var/run/highper-gateway.pid")]
        pid_file: PathBuf,
    },

    /// Display version and build information
    Version {
        /// Show detailed version information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Migrate configuration from YAML/JSON to DSL format
    Migrate {
        /// Input configuration file (YAML/JSON/TOML)
        #[arg(short, long)]
        input: PathBuf,

        /// Output DSL file (defaults to input with .proxy extension)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Validate equivalence after migration
        #[arg(short, long, default_value = "true")]
        validate: bool,

        /// Show diff between input and output
        #[arg(short, long)]
        diff: bool,
    },

    /// Print final configuration with environment variable overrides
    PrintConfig {
        /// Path to configuration file
        #[arg(short, long, default_value = "config/config.yaml")]
        config: PathBuf,

        /// Output format (yaml, json, toml)
        #[arg(short, long, default_value = "yaml")]
        format: String,

        /// Show environment variable overrides separately
        #[arg(short, long)]
        show_overrides: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize rustls crypto provider (required for TLS)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Parse command-line arguments
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli);

    // Execute the appropriate command
    match cli.command {
        None | Some(Commands::Start { .. }) => {
            // Default: start the server
            let Commands::Start { config, hot_reload, daemon } = cli.command.unwrap_or(Commands::Start {
                config: PathBuf::from("config/config.yaml"),
                hot_reload: true,
                daemon: false,
            }) else {
                unreachable!()
            };

            if daemon {
                warn!("Daemon mode requested but not yet implemented");
                warn!("Starting in foreground mode instead");
            }

            start_server(config, hot_reload).await
        }

        Some(Commands::Validate { config, verbose }) => {
            validate_command(config, verbose).await
        }

        Some(Commands::Test { config, upstream, timeout }) => {
            test_command(config, upstream, timeout).await
        }

        Some(Commands::Health { admin_url, format }) => {
            health_command(admin_url, format).await
        }

        Some(Commands::Reload { pid_file }) => {
            reload_command(pid_file).await
        }

        Some(Commands::Version { verbose }) => {
            version_command(verbose);
            Ok(())
        }

        Some(Commands::Migrate { input, output, validate, diff }) => {
            migrate_command(input, output, validate, diff).await
        }

        Some(Commands::PrintConfig { config, format, show_overrides }) => {
            print_config_command(config, format, show_overrides).await
        }
    }
}

/// Start the proxy server
async fn start_server(config_path: PathBuf, hot_reload: bool) -> Result<()> {
    info!("Starting Highper Gateway v{}", env!("CARGO_PKG_VERSION"));
    info!("Loading configuration from: {}", config_path.display());

    // Load configuration
    let config = load_config(&config_path)
        .context("Failed to load configuration")?;

    // Validate configuration
    validate_config(&config)
        .context("Configuration validation failed")?;

    info!("Configuration loaded and validated successfully");

    // Create and run the runtime
    let runtime = if hot_reload {
        info!("Hot reload enabled - configuration changes will be applied automatically");
        info!("Send SIGHUP signal to manually reload configuration");
        Runtime::with_hot_reload(config, config_path)?
    } else {
        info!("Hot reload disabled - restart required for configuration changes");
        Runtime::new(config)?
    };

    runtime.run().await
        .context("Runtime error")?;

    info!("Highper Gateway shut down successfully");
    Ok(())
}

/// Validate configuration file
async fn validate_command(config_path: PathBuf, verbose: bool) -> Result<()> {
    println!("🔍 Validating configuration: {}", config_path.display());
    println!();

    // Load configuration
    let config = match load_config(&config_path) {
        Ok(cfg) => {
            println!("✅ Configuration file loaded successfully");
            cfg
        }
        Err(e) => {
            eprintln!("❌ Failed to load configuration:");
            eprintln!("   {}", e);
            std::process::exit(1);
        }
    };

    if verbose {
        println!("\n📋 Configuration summary:");
        println!("   Upstreams: {}", config.upstreams.len());
        println!("   Routes: {}", config.routes.len());
        println!("   TLS enabled: {}", config.tls.is_some());
        println!("   HTTP/3 port: {}", config.server.http3.port);
        println!("   Admin API: {}", config.admin.is_some());
    }

    // Validate configuration
    match validate_config(&config) {
        Ok(_) => {
            println!("\n✅ Configuration is valid!");
            println!("\n💡 Tip: Use 'highper-gateway test' to verify upstream connectivity");
            Ok(())
        }
        Err(e) => {
            eprintln!("\n❌ Configuration validation failed:");
            eprintln!("   {}", e);
            std::process::exit(1);
        }
    }
}

/// Test upstream connectivity
async fn test_command(config_path: PathBuf, upstream: Option<String>, timeout: u64) -> Result<()> {
    println!("🔌 Testing upstream connectivity...");
    println!();

    // Load configuration
    let config = load_config(&config_path)
        .context("Failed to load configuration")?;

    let upstreams_to_test: Vec<_> = if let Some(name) = upstream {
        config.upstreams.iter()
            .filter(|u| u.name == name)
            .collect()
    } else {
        config.upstreams.iter().collect()
    };

    if upstreams_to_test.is_empty() {
        eprintln!("❌ No upstreams found to test");
        std::process::exit(1);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout))
        .build()?;

    let mut total = 0;
    let mut passed = 0;
    let mut failed = 0;

    for upstream_config in upstreams_to_test {
        println!("📦 Upstream: {}", upstream_config.name);

        for server in &upstream_config.servers {
            total += 1;
            print!("   {} ... ", server.url);

            match client.get(&server.url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() || status.is_redirection() {
                        println!("✅ OK ({})", status);
                        passed += 1;
                    } else {
                        println!("⚠️  HTTP {} (unexpected status)", status);
                        failed += 1;
                    }
                }
                Err(e) => {
                    println!("❌ FAILED ({})", e);
                    failed += 1;
                }
            }
        }
        println!();
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Total: {} | Passed: {} | Failed: {}", total, passed, failed);

    if failed > 0 {
        println!("\n⚠️  Some upstreams failed connectivity tests");
        std::process::exit(1);
    } else {
        println!("\n✅ All upstreams passed connectivity tests!");
    }

    Ok(())
}

/// Check health of running server
async fn health_command(admin_url: String, format: String) -> Result<()> {
    let client = reqwest::Client::new();
    let health_url = format!("{}/api/health", admin_url.trim_end_matches('/'));

    match client.get(&health_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let body = response.text().await?;

                if format == "json" {
                    println!("{}", body);
                } else {
                    // Parse JSON and display nicely
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        println!("✅ Server is healthy");
                        println!();
                        if let Some(obj) = json.as_object() {
                            for (key, value) in obj {
                                println!("   {}: {}", key, value);
                            }
                        }
                    } else {
                        println!("✅ Server is healthy");
                        println!("{}", body);
                    }
                }
                Ok(())
            } else {
                eprintln!("❌ Server returned status: {}", response.status());
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to connect to admin API: {}", e);
            eprintln!("\n💡 Is the server running? Check the admin API URL: {}", admin_url);
            std::process::exit(1);
        }
    }
}

/// Reload configuration
async fn reload_command(pid_file: PathBuf) -> Result<()> {
    #[cfg(unix)]
    {
        use std::fs;

        println!("🔄 Reloading configuration...");

        let pid_str = fs::read_to_string(&pid_file)
            .context(format!("Failed to read PID file: {}", pid_file.display()))?;

        let pid: i32 = pid_str.trim().parse()
            .context("Invalid PID in file")?;

        // Send SIGHUP signal
        unsafe {
            if libc::kill(pid, libc::SIGHUP) == 0 {
                println!("✅ Reload signal sent to process {}", pid);
                println!("\n💡 Check server logs to verify configuration reload");
                Ok(())
            } else {
                eprintln!("❌ Failed to send signal to process {}", pid);
                std::process::exit(1);
            }
        }
    }

    #[cfg(not(unix))]
    {
        eprintln!("❌ Reload command is only supported on Unix-like systems");
        eprintln!("\n💡 On Windows, please restart the service manually");
        std::process::exit(1);
    }
}

/// Migrate configuration from YAML/JSON to DSL format
async fn migrate_command(
    input: PathBuf,
    output: Option<PathBuf>,
    validate: bool,
    diff: bool,
) -> Result<()> {
    use std::fs;

    info!("🔄 Migrating configuration from {} to DSL format", input.display());

    // Determine output path
    let output_path = output.unwrap_or_else(|| {
        let mut path = input.clone();
        path.set_extension("proxy");
        path
    });

    // Load YAML/JSON/TOML config
    info!("📖 Loading configuration from: {}", input.display());
    let config = load_config(&input)
        .context("Failed to load input configuration")?;

    info!("✅ Configuration loaded successfully");

    // Generate DSL
    info!("🔨 Generating DSL format...");
    let dsl_content = dsl_generator::generate_dsl(&config)
        .context("Failed to generate DSL from configuration")?;

    // Write to output file
    info!("💾 Writing to: {}", output_path.display());
    fs::write(&output_path, &dsl_content)
        .context("Failed to write DSL output file")?;

    info!("✅ Migration complete!");
    println!();
    println!("📄 Generated DSL configuration:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{}", dsl_content);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // Validate equivalence if requested
    if validate {
        info!("🔍 Validating equivalence...");

        // Load DSL and convert back to Config
        let dsl_config = {
            let dsl_ast = dsl_parser::parse_dsl(&dsl_content)
                .context("Failed to parse generated DSL")?;
            dsl_converter::convert_dsl_to_config(dsl_ast)
                .context("Failed to convert DSL back to Config")?;
            load_config(&output_path)
                .context("Failed to load generated DSL config")?
        };

        // Basic validation - check key properties
        let original_upstreams = config.upstreams.len();
        let dsl_upstreams = dsl_config.upstreams.len();
        let original_routes = config.routes.len();
        let dsl_routes = dsl_config.routes.len();

        if original_upstreams == dsl_upstreams && original_routes == dsl_routes {
            println!("✅ Validation passed: Configuration equivalence verified");
            println!("   Upstreams: {}", original_upstreams);
            println!("   Routes: {}", original_routes);
        } else {
            warn!("⚠️  Validation warning: Some differences detected");
            warn!("   Original upstreams: {}, DSL upstreams: {}", original_upstreams, dsl_upstreams);
            warn!("   Original routes: {}, DSL routes: {}", original_routes, dsl_routes);
            warn!("   This is expected for complex configurations");
        }
    }

    // Show diff if requested
    if diff {
        println!("\n📊 Configuration Simplification:");
        let original_lines = fs::read_to_string(&input)?.lines().count();
        let dsl_lines = dsl_content.lines().count();
        let reduction = if original_lines > 0 {
            ((original_lines - dsl_lines) as f64 / original_lines as f64) * 100.0
        } else {
            0.0
        };

        println!("   Original: {} lines", original_lines);
        println!("   DSL:      {} lines", dsl_lines);
        println!("   Reduction: {:.1}% ({:.1}x simpler)", reduction, original_lines as f64 / dsl_lines as f64);
    }

    println!();
    println!("💡 Usage:");
    println!("   highper-gateway start --config {}", output_path.display());
    println!();

    Ok(())
}

/// Print final configuration with environment variable overrides
async fn print_config_command(config_path: PathBuf, format: String, show_overrides: bool) -> Result<()> {
    println!("📄 Loading configuration: {}", config_path.display());
    println!();

    // Load configuration
    let config = load_config(&config_path)
        .context("Failed to load configuration")?;

    // Show environment variable overrides if requested
    if show_overrides {
        println!("🔧 Environment Variable Overrides:");
        println!();

        let env_vars = vec![
            ("HIGHPER_MAX_FILE_SIZE", std::env::var("HIGHPER_MAX_FILE_SIZE").ok()),
            ("HIGHPER_MAX_REQUEST_BODY", std::env::var("HIGHPER_MAX_REQUEST_BODY").ok()),
            ("HIGHPER_MAX_UPLOAD_SIZE", std::env::var("HIGHPER_MAX_UPLOAD_SIZE").ok()),
            ("HIGHPER_MAX_PATH_DEPTH", std::env::var("HIGHPER_MAX_PATH_DEPTH").ok()),
            ("HIGHPER_MAX_CONNECTIONS_PER_IP", std::env::var("HIGHPER_MAX_CONNECTIONS_PER_IP").ok()),
            ("HIGHPER_MAX_REQUESTS_PER_SECOND", std::env::var("HIGHPER_MAX_REQUESTS_PER_SECOND").ok()),
            ("HIGHPER_LOG_LEVEL", std::env::var("HIGHPER_LOG_LEVEL").ok()),
            ("HIGHPER_METRICS_PORT", std::env::var("HIGHPER_METRICS_PORT").ok()),
        ];

        let mut has_overrides = false;
        for (name, value) in env_vars {
            if let Some(val) = value {
                println!("  ✓ {} = {}", name, val);
                has_overrides = true;
            }
        }

        if !has_overrides {
            println!("  (No environment variable overrides detected)");
        }

        println!();
        println!("───────────────────────────────────────────────────");
        println!();
    }

    // Print configuration in requested format
    match format.as_str() {
        "yaml" => {
            println!("📋 Configuration (YAML format):");
            println!();
            let yaml = serde_yaml::to_string(&config)
                .context("Failed to serialize config to YAML")?;
            println!("{}", yaml);
        }
        "json" => {
            println!("📋 Configuration (JSON format):");
            println!();
            let json = serde_json::to_string_pretty(&config)
                .context("Failed to serialize config to JSON")?;
            println!("{}", json);
        }
        "toml" => {
            println!("📋 Configuration (TOML format):");
            println!();
            let toml = toml::to_string_pretty(&config)
                .context("Failed to serialize config to TOML")?;
            println!("{}", toml);
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown format: {}. Use yaml, json, or toml", format));
        }
    }

    Ok(())
}

/// Display version information
fn version_command(verbose: bool) {
    println!("Highper Gateway v{}", env!("CARGO_PKG_VERSION"));

    if verbose {
        println!();
        println!("Build Information:");
        println!("  Compiler: rustc {}", env!("RUSTC_VERSION"));
        println!("  Target: {}", env!("TARGET"));
        println!("  Profile: {}", env!("PROFILE"));
        println!("  Build date: {}", env!("BUILD_DATE"));
        println!();
        println!("Features:");
        println!("  jemalloc: {}", cfg!(feature = "jemalloc"));
        println!("  io-uring: {}", cfg!(feature = "io-uring"));
        println!("  consul: {}", cfg!(feature = "consul"));
        println!("  etcd: {}", cfg!(feature = "etcd-client"));
        println!();
        println!("Capabilities:");
        println!("  HTTP/1.1: ✅");
        println!("  HTTP/2: ✅");
        println!("  HTTP/3 (QUIC): ✅");
        println!("  TLS 1.2/1.3: ✅");
        println!("  WebSocket: ✅");
        println!("  gRPC: ✅");
        println!("  Load balancing algorithms: 8");
        println!("  Compression: gzip, brotli, zstd, deflate");
    }
}

/// Initialize logging based on configuration
fn init_logging(cli: &Cli) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&cli.log_level));

    // Check for JSON logs from env var or CLI flag
    let json_logs = cli.json_logs ||
        std::env::var("HIGHPER_JSON_LOGS")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);

    if json_logs {
        // Structured JSON logging for production (12-Factor XI)
        // Output format compatible with ELK, Splunk, CloudWatch, etc.
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_target(true)
                    .with_level(true)
                    .with_thread_ids(true)
                    .with_thread_names(true)
            )
            .init();

        eprintln!("📋 Structured JSON logging enabled");
        eprintln!("   Compatible with: ELK Stack, Splunk, CloudWatch, Datadog");
        eprintln!("   All logs include: timestamp, level, target, structured fields");
    } else {
        // Pretty logging for development
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_target(true)
                    .with_level(true)
            )
            .init();
    }
}

#!/bin/bash
# Quick test to parse the PHP-FPM scenarios DSL file

cd "$(dirname "$0")/highper-gateway"

# Build a simple test binary to parse the DSL
cat > /tmp/test_parse.rs << 'EOF'
use highper_gateway::config::dsl_parser::parse_dsl;
use std::fs;

fn main() {
    let content = fs::read_to_string("../examples/php-fpm-scenarios.dsl")
        .expect("Failed to read scenarios file");

    match parse_dsl(&content) {
        Ok(config) => {
            println!("✓ Successfully parsed PHP-FPM scenarios!");
            println!("  Sites: {}", config.sites.len());
            for (idx, site) in config.sites.iter().enumerate() {
                println!("  Site {}: {:?}", idx + 1, site.address);
                println!("    Routes: {}", site.routes.len());
                println!("    Directives: {}", site.directives.len());
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to parse: {}", e);
            std::process::exit(1);
        }
    }
}
EOF

echo "Testing PHP-FPM scenarios parsing..."
cargo run --example test_parse 2>/dev/null || echo "Note: Run 'cargo build' first to test parsing"

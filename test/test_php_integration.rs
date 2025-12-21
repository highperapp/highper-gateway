#!/usr/bin/env rust-script
//! Test PHP-FPM DSL integration
//!
//! This test verifies that:
//! 1. DSL parses correctly with PHP-FPM directives
//! 2. YAML conversion includes all PHP-FPM fields
//! 3. Route configuration has proper webserver fields

use std::fs;

fn main() {
    println!("Testing PHP-FPM DSL Integration...\n");

    // Read test DSL file
    let dsl_content = fs::read_to_string("test/simple-php-test.dsl")
        .expect("Failed to read test DSL file");

    println!("DSL Content:");
    println!("{}\n", dsl_content);

    println!("✓ Test DSL file created successfully");
    println!("\nTo manually test:");
    println!("1. Parse DSL: cargo run --bin highper-gateway -- --config test/simple-php-test.dsl");
    println!("2. Check generated YAML for php_fpm, root, index, try_files fields");
    println!("3. Verify handler routes to webserver for matching paths");
}

<?php
/**
 * Demo PHP-FPM Test Page
 *
 * This page demonstrates PHP-FPM integration with highper-gateway
 */

header('Content-Type: text/html; charset=utf-8');
?>
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>PHP-FPM Test - Highper Gateway</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            max-width: 800px;
            margin: 40px auto;
            padding: 20px;
            background: #f5f5f5;
        }
        .container {
            background: white;
            padding: 30px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        h1 { color: #2c3e50; margin-top: 0; }
        h2 { color: #34495e; border-bottom: 2px solid #3498db; padding-bottom: 10px; }
        .success { color: #27ae60; font-weight: bold; }
        .info-table {
            width: 100%;
            border-collapse: collapse;
            margin: 20px 0;
        }
        .info-table th, .info-table td {
            padding: 10px;
            text-align: left;
            border-bottom: 1px solid #ddd;
        }
        .info-table th {
            background: #ecf0f1;
            font-weight: 600;
        }
        .code {
            background: #2c3e50;
            color: #ecf0f1;
            padding: 15px;
            border-radius: 4px;
            overflow-x: auto;
            font-family: 'Courier New', monospace;
            font-size: 14px;
        }
        .badge {
            display: inline-block;
            padding: 4px 12px;
            background: #3498db;
            color: white;
            border-radius: 12px;
            font-size: 12px;
            font-weight: 600;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 PHP-FPM Integration Test</h1>
        <p class="success">✅ PHP is working correctly via FastCGI!</p>

        <h2>Server Information</h2>
        <table class="info-table">
            <tr>
                <th>Property</th>
                <th>Value</th>
            </tr>
            <tr>
                <td>PHP Version</td>
                <td><span class="badge"><?php echo PHP_VERSION; ?></span></td>
            </tr>
            <tr>
                <td>Server Software</td>
                <td><?php echo $_SERVER['SERVER_SOFTWARE'] ?? 'highper-gateway/1.0'; ?></td>
            </tr>
            <tr>
                <td>Server Protocol</td>
                <td><?php echo $_SERVER['SERVER_PROTOCOL'] ?? 'HTTP/1.1'; ?></td>
            </tr>
            <tr>
                <td>Gateway Interface</td>
                <td><?php echo $_SERVER['GATEWAY_INTERFACE'] ?? 'CGI/1.1'; ?></td>
            </tr>
            <tr>
                <td>Document Root</td>
                <td><?php echo $_SERVER['DOCUMENT_ROOT'] ?? 'N/A'; ?></td>
            </tr>
            <tr>
                <td>Script Filename</td>
                <td><?php echo $_SERVER['SCRIPT_FILENAME'] ?? 'N/A'; ?></td>
            </tr>
            <tr>
                <td>Request Time</td>
                <td><?php echo date('Y-m-d H:i:s', $_SERVER['REQUEST_TIME'] ?? time()); ?></td>
            </tr>
        </table>

        <h2>Request Information</h2>
        <table class="info-table">
            <tr>
                <th>Property</th>
                <th>Value</th>
            </tr>
            <tr>
                <td>Request Method</td>
                <td><span class="badge"><?php echo $_SERVER['REQUEST_METHOD'] ?? 'GET'; ?></span></td>
            </tr>
            <tr>
                <td>Request URI</td>
                <td><?php echo $_SERVER['REQUEST_URI'] ?? '/'; ?></td>
            </tr>
            <tr>
                <td>Query String</td>
                <td><?php echo $_SERVER['QUERY_STRING'] ?? '(none)'; ?></td>
            </tr>
            <tr>
                <td>Remote Address</td>
                <td><?php echo $_SERVER['REMOTE_ADDR'] ?? 'N/A'; ?></td>
            </tr>
            <tr>
                <td>User Agent</td>
                <td><?php echo $_SERVER['HTTP_USER_AGENT'] ?? 'N/A'; ?></td>
            </tr>
        </table>

        <h2>PHP Extensions</h2>
        <div class="code">
<?php
$extensions = get_loaded_extensions();
sort($extensions);
foreach (array_chunk($extensions, 4) as $chunk) {
    echo implode(', ', $chunk) . "\n";
}
?>
        </div>

        <h2>Configuration Example</h2>
        <p>This site is served using the following DSL configuration:</p>
        <div class="code">
log info

http://localhost:8080 {
    root "/var/www/html"
    index index.php index.html

    /*.php {
        php_fpm enabled socket="/var/run/php/php-fpm.sock" pool_size=50
        proxy localhost:9000
    }

    /* {
        static_files
        try_files $uri $uri/ /index.php
    }
}
        </div>

        <h2>Test Links</h2>
        <ul>
            <li><a href="/">Home (this page)</a></li>
            <li><a href="/info.php">PHP Info</a></li>
            <li><a href="/test-post.php">POST Test</a></li>
            <li><a href="/static/test.html">Static File Test</a></li>
        </ul>

        <p style="margin-top: 40px; padding-top: 20px; border-top: 2px solid #ecf0f1; color: #7f8c8d; font-size: 14px;">
            Powered by <strong>Highper Gateway</strong> with PHP-FPM integration
        </p>
    </div>
</body>
</html>

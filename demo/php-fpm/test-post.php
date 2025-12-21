<?php
/**
 * POST Request Test
 *
 * Tests POST body handling via FastCGI
 */

header('Content-Type: application/json');

$response = [
    'method' => $_SERVER['REQUEST_METHOD'],
    'timestamp' => time(),
    'post_data' => $_POST,
    'raw_input' => file_get_contents('php://input'),
    'content_type' => $_SERVER['CONTENT_TYPE'] ?? null,
    'content_length' => $_SERVER['CONTENT_LENGTH'] ?? null,
];

if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    $response['status'] = 'success';
    $response['message'] = 'POST data received successfully';
} else {
    $response['status'] = 'info';
    $response['message'] = 'Use POST to test body handling';
}

echo json_encode($response, JSON_PRETTY_PRINT);

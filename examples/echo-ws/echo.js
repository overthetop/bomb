const WebSocket = require('ws');
const wss = new WebSocket.Server({port: 8081});

wss.on('connection', (ws) => {
    console.log('Client connected to echo server');

    ws.on('message', (data) => {
        ws.send(data.toString());
    });

    ws.on('close', () => {
        console.log('Client disconnected from echo server');
    });

    ws.on('error', (error) => {
        console.error('WebSocket error:', error);
    });
});

wss.on('error', (error) => {
    console.error('Server error:', error);
});

console.log('WebSocket echo server running on ws://localhost:8081');
const WebSocket = require('ws');
const wss = new WebSocket.Server({port: 8080});

// Track all connected clients
const clients = new Set();

wss.on('connection', (ws) => {
    console.log('Client connected');
    clients.add(ws);

    ws.on('message', (data) => {
        // Broadcast message to all connected clients
        const message = data.toString();
        clients.forEach(client => {
            if (client.readyState === WebSocket.OPEN) {
                client.send(message);
            }
        });
    });

    ws.on('close', () => {
        console.log('Client disconnected');
        clients.delete(ws);
    });
});

console.log('WebSocket broadcast server running on ws://localhost:8080');

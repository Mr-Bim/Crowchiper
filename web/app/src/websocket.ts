/**
 * WebSocket client with automatic reconnection and JWT authentication.
 *
 * The auth token is sent via HTTP-only cookie (handled automatically by browser).
 *
 * Usage:
 *   import { wsClient, WsUser } from './websocket.ts';
 *
 *   // Connect (auth cookie sent automatically)
 *   wsClient.connect();
 *
 *   // Get user info after connected
 *   const user = wsClient.getUser();
 *   if (user) {
 *     console.log('UUID:', user.uuid);
 *     console.log('Username:', user.username);
 *   }
 *
 *   // Listen for events
 *   wsClient.onConnected((user) => console.log('Connected:', user));
 *   wsClient.onDisconnected(() => console.log('Disconnected'));
 *
 *   // Disconnect when done
 *   wsClient.disconnect();
 */

declare const API_PATH: string;

export interface WsUser {
	uuid: string;
	username: string;
}

interface ConnectedMessage {
	type: "connected";
	user: WsUser;
}

interface PingMessage {
	type: "ping";
}

interface ErrorMessage {
	type: "error";
	message: string;
}

type ServerMessage = ConnectedMessage | PingMessage | ErrorMessage;

type ConnectedCallback = (user: WsUser) => void;
type DisconnectedCallback = () => void;
type ErrorCallback = (error: string) => void;

class WebSocketClient {
	private ws: WebSocket | null = null;
	private user: WsUser | null = null;
	private reconnectAttempts = 0;
	private maxReconnectAttempts = 10;
	private baseReconnectDelay = 1000; // 1 second
	private maxReconnectDelay = 30000; // 30 seconds
	private reconnectTimer: number | null = null;
	private shouldReconnect = true;

	private connectedCallbacks: ConnectedCallback[] = [];
	private disconnectedCallbacks: DisconnectedCallback[] = [];
	private errorCallbacks: ErrorCallback[] = [];

	/**
	 * Build the WebSocket URL.
	 */
	private buildWsUrl(): string {
		const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
		const host = window.location.host;
		return `${protocol}//${host}${API_PATH}/ws`;
	}

	/**
	 * Connect to the WebSocket server.
	 * Automatically reconnects on disconnect.
	 */
	connect(): void {
		if (this.ws?.readyState === WebSocket.OPEN) {
			return; // Already connected
		}

		this.shouldReconnect = true;
		this.doConnect();
	}

	private doConnect(): void {
		const url = this.buildWsUrl();

		try {
			this.ws = new WebSocket(url);

			this.ws.onopen = () => {
				console.log("WebSocket: Connected");
				this.reconnectAttempts = 0;
			};

			this.ws.onmessage = (event) => {
				this.handleMessage(event.data);
			};

			this.ws.onclose = (event) => {
				console.log(`WebSocket: Closed (code: ${event.code})`);
				this.user = null;
				this.notifyDisconnected();

				if (this.shouldReconnect && event.code !== 1000) {
					this.scheduleReconnect();
				}
			};

			this.ws.onerror = () => {
				console.error("WebSocket: Connection error");
				this.notifyError("Connection error");
			};
		} catch (error) {
			console.error("WebSocket: Failed to create connection", error);
			this.notifyError("Failed to create connection");
			this.scheduleReconnect();
		}
	}

	private handleMessage(data: string): void {
		try {
			const message: ServerMessage = JSON.parse(data);

			switch (message.type) {
				case "connected":
					this.user = message.user;
					this.notifyConnected(message.user);
					break;

				case "ping":
					// Server ping to keep connection alive
					// No response needed - server just verifies connection is open
					break;

				case "error":
					console.error("WebSocket: Server error:", message.message);
					this.notifyError(message.message);
					break;
			}
		} catch (error) {
			console.error("WebSocket: Failed to parse message", error);
		}
	}

	private scheduleReconnect(): void {
		if (this.reconnectTimer !== null) {
			return; // Already scheduled
		}

		if (this.reconnectAttempts >= this.maxReconnectAttempts) {
			console.error("WebSocket: Max reconnect attempts reached");
			this.notifyError("Max reconnect attempts reached");
			return;
		}

		// Exponential backoff with jitter
		const delay = Math.min(
			this.baseReconnectDelay * 2 ** this.reconnectAttempts +
				Math.random() * 1000,
			this.maxReconnectDelay,
		);

		console.log(
			`WebSocket: Reconnecting in ${Math.round(delay / 1000)}s (attempt ${this.reconnectAttempts + 1}/${this.maxReconnectAttempts})`,
		);

		this.reconnectTimer = window.setTimeout(() => {
			this.reconnectTimer = null;
			this.reconnectAttempts++;
			this.doConnect();
		}, delay);
	}

	/**
	 * Disconnect from the WebSocket server.
	 * Does not trigger automatic reconnection.
	 */
	disconnect(): void {
		this.shouldReconnect = false;

		if (this.reconnectTimer !== null) {
			clearTimeout(this.reconnectTimer);
			this.reconnectTimer = null;
		}

		if (this.ws) {
			this.ws.close(1000, "Client disconnect");
			this.ws = null;
		}

		this.user = null;
	}

	/**
	 * Check if currently connected.
	 */
	isConnected(): boolean {
		return this.ws?.readyState === WebSocket.OPEN;
	}

	/**
	 * Get the current user info.
	 * Returns null if not connected.
	 */
	getUser(): WsUser | null {
		return this.user;
	}

	/**
	 * Get the user UUID.
	 * Returns null if not connected.
	 */
	getUserUuid(): string | null {
		return this.user?.uuid ?? null;
	}

	/**
	 * Register a callback for when connection is established.
	 */
	onConnected(callback: ConnectedCallback): void {
		this.connectedCallbacks.push(callback);
	}

	/**
	 * Register a callback for when connection is lost.
	 */
	onDisconnected(callback: DisconnectedCallback): void {
		this.disconnectedCallbacks.push(callback);
	}

	/**
	 * Register a callback for errors.
	 */
	onError(callback: ErrorCallback): void {
		this.errorCallbacks.push(callback);
	}

	private notifyConnected(user: WsUser): void {
		for (const callback of this.connectedCallbacks) {
			try {
				callback(user);
			} catch (error) {
				console.error("WebSocket: Connected callback error", error);
			}
		}
	}

	private notifyDisconnected(): void {
		for (const callback of this.disconnectedCallbacks) {
			try {
				callback();
			} catch (error) {
				console.error("WebSocket: Disconnected callback error", error);
			}
		}
	}

	private notifyError(message: string): void {
		for (const callback of this.errorCallbacks) {
			try {
				callback(message);
			} catch (error) {
				console.error("WebSocket: Error callback error", error);
			}
		}
	}
}

// Export singleton instance
export const wsClient = new WebSocketClient();

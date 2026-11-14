-- Staff login: each organization can name the server its staff connect to.
-- NULL means "this server" (the deployment that serves the request).
ALTER TABLE organizations ADD COLUMN server_url TEXT;

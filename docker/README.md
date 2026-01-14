# Stremio Service - Docker Deployment

This directory contains Docker configuration files for running Stremio Service in a container with enhanced security features.

## Features

- **Multi-stage Docker build** for optimized image size
- **Nginx reverse proxy** with rate limiting and security headers
- **Automatic IP banning** for scanner/bot detection
- **HTTP Basic Authentication** support
- **HTTPS/SSL support** with custom certificates
- **Health checks** for container orchestration
- **Persistent data volumes** for configuration and cache

## Quick Start

### Using Docker Compose (Recommended)

```bash
cd docker
docker compose up -d
```

The server will be available at `http://localhost:8080`

### Using Docker Run

```bash
docker build -t stremio-service -f docker/Dockerfile.light .

docker run -d \
  --name stremio-service \
  -e NO_CORS=1 \
  -e AUTO_SERVER_URL=1 \
  -e ENABLE_IP_BAN=1 \
  -p 8080:8080 \
  -v stremio-data:/data \
  --restart unless-stopped \
  stremio-service
```

## Dockerfile Variants

We provide two Dockerfile options:

| Dockerfile | Build Time | Size | Best For |
|------------|------------|------|----------|
| `Dockerfile.light` | ~5 min | ~200MB | **Dokploy**, CI/CD, quick deployments |
| `Dockerfile` | ~30+ min | ~180MB | Custom FFmpeg builds, specific codec needs |

### Dockerfile.light (Default - Recommended)

Uses pre-built static FFmpeg binaries from [johnvansickle.com](https://johnvansickle.com/ffmpeg/). Much faster to build.

```bash
docker build -f docker/Dockerfile.light -t stremio-service .
```

### Dockerfile (Full Build)

Compiles FFmpeg from source with specific codecs. Use if you need:
- Custom FFmpeg configuration
- Specific codec versions
- Maximum optimization for your CPU

```bash
docker build -f docker/Dockerfile -t stremio-service .
```

## Dokploy Deployment

1. Connect your repository to Dokploy
2. Dokploy will auto-detect the `docker-compose.yml` in the project root
3. Configure environment variables in Dokploy's UI:
   - `USERNAME` / `PASSWORD` - Enable authentication
   - `ENABLE_IP_BAN=1` - Block scanner bots
   - `SERVER_VERSION` - Override server.js version if needed

The default configuration uses `Dockerfile.light` for fast builds.

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NO_CORS` | `0` | Disable CORS headers (set to `1` for local deployments) |
| `AUTO_SERVER_URL` | `1` | Automatically configure server URL from browser |
| `SERVER_URL` | - | Manual server URL (e.g., `http://192.168.1.100:8080/`) |
| `IPADDRESS` | - | Your server's IP address for HTTPS setup |
| `DOMAIN` | - | Your custom domain name |
| `USERNAME` | - | HTTP Basic Auth username |
| `PASSWORD` | - | HTTP Basic Auth password |
| `ENABLE_IP_BAN` | `1` | Enable automatic IP banning for scanners |
| `BAN_THRESHOLD` | `10` | Number of violations before IP is banned |
| `BAN_WINDOW` | `60` | Time window (seconds) for counting violations |
| `BAN_DURATION` | `3600` | Ban duration in seconds (default: 1 hour) |
| `LOG_LEVEL` | `info` | Logging level (debug, info, warn, error) |

### Authentication

To enable HTTP Basic Authentication:

```yaml
environment:
  - USERNAME=your_username
  - PASSWORD=your_secure_password
```

### HTTPS with Custom Certificates

Mount your certificates and configure:

```yaml
volumes:
  - ./certs:/certs:ro
environment:
  - CERT_FILE=/certs/certificate.pem
  - KEY_FILE=/certs/private.key
  - DOMAIN=stremio.yourdomain.com
```

## IP Banning

The container includes an automatic IP banning system that:

1. **Monitors nginx logs** for scanner/bot requests (WordPress probes, etc.)
2. **Tracks violations** per IP within a configurable time window
3. **Automatically bans IPs** that exceed the threshold
4. **Updates nginx configuration** to block banned IPs
5. **Automatically expires bans** after the configured duration

### Blocked Request Types

The following requests are logged as scanner activity and counted towards bans:

- WordPress paths (`/wp-admin`, `/wp-content`, `/wp-includes`, etc.)
- Config files (`.env`, `.git`, `.htaccess`, etc.)
- PHP files
- Admin panels (`/phpmyadmin`, `/admin`, `/manager`, etc.)
- Common exploit paths

### Manual IP Management

To manually ban an IP, add it to the mounted volume:

```bash
echo "deny 1.2.3.4;" >> stremio-bans/blocked_ips.conf
docker exec stremio-service nginx -s reload
```

## Volumes

| Volume | Purpose |
|--------|---------|
| `/data` | Stremio server data and configuration |
| `/var/lib/stremio-bans` | Ban database (persists bans across restarts) |

## Ports

| Port | Protocol | Description |
|------|----------|-------------|
| 8080 | TCP | HTTP/HTTPS (via nginx) |
| 11470 | TCP | Stremio server (internal, optional exposure) |

## Health Checks

The container includes a health check that verifies:

1. Nginx is running
2. Stremio server is responding on port 11470

```bash
# Check container health
docker inspect --format='{{.State.Health.Status}}' stremio-service
```

## Logs

View container logs:

```bash
# All logs
docker logs stremio-service

# Follow logs
docker logs -f stremio-service

# Scanner activity
docker exec stremio-service cat /var/log/nginx/scanner.log
```

## Troubleshooting

### Container won't start

1. Check logs: `docker logs stremio-service`
2. Verify port 8080 is not in use
3. Ensure volumes have correct permissions

### IP banning not working

1. Ensure `ENABLE_IP_BAN=1` is set
2. Check if the container has `NET_ADMIN` capability
3. View ban database: `docker exec stremio-service cat /var/lib/stremio-bans/bans.db`

### Performance issues

1. Increase `worker_connections` in nginx.conf
2. Adjust rate limiting in default.conf
3. Monitor resource usage: `docker stats stremio-service`

## Building

```bash
# Build the image
docker build -t stremio-service -f docker/Dockerfile ..

# Build with specific platform
docker build --platform linux/amd64 -t stremio-service -f docker/Dockerfile ..
```

## License

GPL-2.0 - See [LICENSE.md](../LICENSE.md)

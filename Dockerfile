# CRISPR Production Dockerfile
# Multi-stage build for minimal image size

# ============================================================================
# Stage 1: Build the UI
# ============================================================================
FROM node:20-alpine AS ui-builder

WORKDIR /app/ui

# Install dependencies
COPY ui/package*.json ./
RUN npm ci

# Build the UI
COPY ui/ ./
RUN npm run build

# ============================================================================
# Stage 2: Build the Rust binary
# ============================================================================
FROM rust:1.75-bookworm AS rust-builder

WORKDIR /app

# Install protobuf compiler for tonic-build
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Cache dependencies by building a dummy project first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy source code and proto files
COPY src/ src/
COPY proto/ proto/
COPY build.rs ./

# Copy built UI from previous stage
COPY --from=ui-builder /app/ui/build ui/build/

# Build the actual binary
RUN touch src/main.rs && cargo build --release

# ============================================================================
# Stage 3: Runtime image
# ============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -s /bin/false crispr

WORKDIR /app

# Copy binary
COPY --from=rust-builder /app/target/release/crispr /app/crispr

# Copy config templates
COPY config/ /app/config/

# Set ownership
RUN chown -R crispr:crispr /app

USER crispr

# Environment
ENV RUST_LOG=info
ENV CRISPR_CONFIG=/app/config/config.yaml

# Expose ports
EXPOSE 8080 4317

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run
ENTRYPOINT ["/app/crispr"]

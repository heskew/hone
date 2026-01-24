.PHONY: all build dev test clean install-ui

# Default target
all: build

# Build everything
build:
	cargo build --release
	cd ui && npm run build

# Development mode - run backend and frontend concurrently
dev:
	@echo "Starting development servers..."
	@echo "Backend: http://localhost:3000"
	@echo "Frontend: http://localhost:5173"
	@echo ""
	@echo "Run these in separate terminals:"
	@echo "  make dev-backend"
	@echo "  make dev-ui"

dev-backend:
	cargo run -- serve --port 3000 --no-auth

dev-ui:
	cd ui && npm run dev

# Install UI dependencies
install-ui:
	cd ui && npm install

# Run linters
lint:
	cargo fmt --check
	cargo clippy -- -D warnings
	cd ui && npm run lint
	cd ui && npm run fmt:check

# Format code
fmt:
	cargo fmt
	cd ui && npm run fmt

# Run tests
test:
	cargo test

# Run tests with coverage (requires cargo-llvm-cov)
coverage:
	cargo llvm-cov

# Run tests with coverage summary only
coverage-summary:
	cargo llvm-cov --summary-only

# Clean build artifacts
clean:
	cargo clean
	rm -rf ui/dist ui/node_modules

# Initialize a fresh database
init:
	cargo run -- init

# Import sample data (for testing)
sample-import:
	cargo run -- import --file samples/chase_sample.csv --bank chase

# Run detection
detect:
	cargo run -- detect --kind all

# Show dashboard
dashboard:
	cargo run -- dashboard

# Show help
help:
	@echo "Hone Development Commands"
	@echo ""
	@echo "  make build       - Build backend and frontend"
	@echo "  make dev         - Show instructions for dev mode"
	@echo "  make dev-backend - Run backend in dev mode"
	@echo "  make dev-ui      - Run frontend in dev mode"
	@echo "  make lint        - Run linters (clippy, oxlint, dprint)"
	@echo "  make fmt         - Format code"
	@echo "  make test        - Run tests"
	@echo "  make coverage    - Run tests with coverage report"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make init        - Initialize database"
	@echo "  make detect      - Run waste detection"
	@echo "  make dashboard   - Show CLI dashboard"

# Kellnr Dev Container

A comprehensive development container setup for the Kellnr project, providing a consistent development environment with all necessary tools and dependencies.

## Overview

This devcontainer provides a complete development environment based on Ubuntu 24.04 with pre-installed tools for Rust, Node.js, Docker, and various development utilities. It's designed to ensure consistent development experiences across different machines and team members.

## Features

### Pre-installed Tools

- **Rust Development**
  - Rust stable toolchain
  - Cargo with additional tools (cargo2junit, cargo-nextest)
  - Rustup package manager

- **Node.js Development**
  - Node.js 23.x
  - npm 11.0.0
  - Webpack globally installed

- **Docker Support**
  - Docker CLI
  - Docker Buildx plugin
  - Docker Compose plugin

- **Development Tools**
  - Git and Git LFS
  - Vim, Nano editors
  - Build tools (gcc, cmake, ninja-build, etc.)
  - Network utilities (ping, curl, etc.)
  - Various libraries (OpenSSL, Boost, etc.)

### VSCode/Cursor Integration

The Kellnr DevContainer includes pre-configured VSCode/Cursor extensions:
- Rust Bundle
- Rust Analyzer
- CMake Tools
- C/C++ Tools
- Dependi (dependency management)

## Quick Start

### Prerequisites

- Docker installed on your host machine
- VSCode/Cursor with the DevContainers extension (optional, for VSCode/Cursor integration)

### Building the Container

Execute the following from the root of the kellnr project. *Note*, these steps will build the DevContainer from within the devcontainer dir.

1. **Build the container image:**
   ```bash
   cd devcontainer
   ./build.sh
   cd ..
   ```

## File Structure

```
devcontainer/
├── README.md               # This file
├── build.sh                # Build script for the container
├── devcontainer.common     # Common configuration variables
├── setupUser.sh            # User setup script
├── shell.sh                # Shell launcher script
├── sourceEnv.sh            # Environment variable setup
├── initUserVol.sh          # User volume initialization
├── src/                    # Container source files
│   ├── Containerfile       # Main container image definition
│   ├── setupUser.sh        # User setup script (internal container ver)
│   ├── initUserVol.sh      # Volume init script (internal container ver)
│   └── test.sh             # Test script
├── LICENSE                 # License file
└── VERSION                 # Version information
```

## Usage

### DevContainer Usage

Execute the following from the root of the kellnr project. *Note*: at the project root, these are shortcuts into the devcontainer dir.

1. **Initialize user data (*first time only*):**
   ```bash
   ./initUserVol.sh y
   ```

2. **Set up your user environment (*first time only*):**
   ```bash
   ./setupUser.sh
   ```

3. **Launch a DevContainer shell (to use it outside VSCode/Cursor):**
   ```bash
   ./shell.sh
   ```

### VSCode/Cursor DevContainer Usage

The setup script automatically generates a `.devcontainer/devcontainer.json` file for VSCode/Cursor integration:

1. Follow the manual setup steps above
2. Open the project in VSCode/Cursor
3. Use "Reopen in Container" from the Command Palette

## Configuration

### Environment Variables (defined in devcontainer.common)

The container uses several environment variables defined in `devcontainer.common`:

- `version`: Container version tag (default: "latest")
- `cont_tag`: Full container tag name
- `build_user_data`: Docker volume for user data
- `build_user_home`: Host directory for user home
- `devcontainer_name`: Display name for VSCode/Cursor

### Volume Mounts

The container automatically mounts:
- Your SSH keys (`~/.ssh`)
- Git configuration (`~/.gitconfig`)
- Current workspace directory
- User data volume for persistent settings
- User home directory for persistent files

## Development Workflow

### Rust Development

The container includes the complete Rust toolchain:
```bash
# Check Rust installation
cargo --version
rustc --version

# Build your project
cargo build

# Run tests
cargo test
```

### Node.js Development

Node.js 23.x and npm 11.0.0 are pre-installed:
```bash
# Check Node.js installation
node --version
npm --version

# Install dependencies
npm install

# Build with webpack
webpack
```

### Docker Development

Docker CLI tools are available for containerized development:
```bash
# Build Docker images
docker build -t myapp .

# Run containers
docker run myapp

# Use Docker Compose
docker compose up
```

## Troubleshooting

### Container Build Issues

If the container build fails:
1. Ensure Docker is running
2. Check available disk space
3. Verify internet connectivity for package downloads

### User Setup Issues

If user setup fails:
1. Ensure the container image was built successfully
2. Check Docker volume permissions
3. Try recreating user volumes: `./initUserVol.sh y`

### Volume Issues

If persistent data isn't working:
1. Check Docker volume exists: `docker volume ls`
2. Recreate volumes if needed: `./initUserVol.sh y`
3. Verify volume mounts in `sourceEnv.sh`

## Maintenance

### Updating the Container

To update the container with new packages or tools:
1. Modify `src/Containerfile`
2. Rebuild: `./build.sh`
3. Recreate user volumes: `./initUserVol.sh y`
4. Re-run setup: `./setupUser.sh`

### Cleaning Up

To clean up Docker resources:
```bash
# Remove container image
docker rmi kellnrdevcontainer:latest

# Remove volumes (WARNING: This will delete persistent data)
docker volume rm build-kellnr-user-data-latest
```

## Contributing

When modifying the devcontainer:
1. Test changes thoroughly
2. Update this README if needed
3. Ensure all scripts remain executable
4. Test both manual and VSCode/Cursor usage

## License

See the LICENSE file in the root kellnr project for licensing information.

## Support

For issues or questions about the devcontainer setup, please refer to the main Kellnr project documentation or create an issue in the project repository.

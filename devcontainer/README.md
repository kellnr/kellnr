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
- VSCode/Cursor with the [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) (optional, for VSCode/Cursor integration)

### Building the Container

**(1)** Build the container image:
   ```bash
   devcontainer/dockerBuild.sh
   ```

## Usage

### DevContainer Usage

**(1)** Set up devcontainer user
```bash
devcontainer/setupUser.sh
```

**(2)** Run the `devcontainer` in a terminal.
```bash
./shell.sh
```

### VSCode/Cursor DevContainer Usage

The setup script automatically generates a `.devcontainer/devcontainer.json` file for VSCode/Cursor integration:

**(1)** Follow the steps above

**(2)** Open the project in VSCode/Cursor

**(3)** Use "Reopen in Container" from the Command Palette

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
- User home volume for persistent files

## Additional notes
1. Any of the devcontainer scripts can be run either in the devcontainer directory or in the root of the project, e.g.
```bash
cd devcontainer
./setupUser.sh
# or
devcontainer/setupUser.sh
```
2. `setupUser.sh` will automatically create user volumes whenever needed.
1. If the user **data** volume already exists, `setupUser.sh` takes no action on it.
1. If the user **home** volume already exists, `setupUser.sh` will attempt to freshen its contents, but does not remove it beforehand.
1. If either user volume exists, and `setupUser.sh -c` is run (i.e. with the `-c` switch), it will remove both volumes and recreate them.

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
3. Try recreating user volumes: `devcontainer/setupUser.sh -c`

### Volume Issues

If persistent data isn't working:
1. Check Docker volume exists: `docker volume ls`
2. Recreate volumes if needed: `devcontainer/setupUser.sh -c`
3. Verify volume mounts in `sourceEnv.sh`

## File Structure

```
devcontainer/
├── README.md               # This file
├── devcontainer.common     # Common configuration variables
├── dockerBuild.sh          # Build script for the container image
├── dockerPull.sh           # Pull script for the container image
├── setupUser.sh            # User setup script
├── shell.sh                # Shell launcher script
├── sourceEnv.sh            # Environment variable setup
└── src/                    # Container source files
    ├── bootstrapUser.sh    # User setup script
    ├── bootstrapUserVol.sh # Volume init script
    ├── Containerfile       # Main container image definition    
    └── test.sh             # Test script
```

## Maintenance

### Updating the Container

To update the container with new packages or tools:
1. Modify `src/Containerfile`
2. Rebuild: `devcontainer/dockerBuild.sh`
3. Recreate user volumes: `devcontainer/setupUser.sh -c`

### Cleaning Up

To clean up Docker resources:
```bash
# Remove container image
docker rmi kellnrdevcontainer:latest

# Remove volumes (WARNING: This will delete persistent data)
. devcontainer/devcontainer.common
docker volume rm $build_user_data
docker volume rm $build_user_home
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

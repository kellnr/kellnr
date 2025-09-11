#!/usr/bin/env bash
set -euo pipefail

# Define constants
PLATFORMS="linux/arm/v7,linux/arm64/v8,linux/amd64"

function parse_args {
    echo "STEP: Parse arguments"
    if [ $# -lt 2 ]; then
        echo "Error: Insufficient arguments"
        echo "Usage: $0 IMAGE_NAME VERSION [BUILD_TYPE]"
        echo "  BUILD_TYPE: 'all' (default), 'standard', 'minimal'"
        exit 1
    else
        IMAGE=$1
        VERSION=$2
        BUILD_TYPE=${3:-"all"}
        echo "Use image: $IMAGE"
        echo "Use version: $VERSION"
        echo "Build type: $BUILD_TYPE"
    fi
}

function validate_version {
    if ! [[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
        echo "Error: Invalid version format. Expected format: X.Y.Z or X.Y.Z-suffix"
        exit 1
    fi
}

function extract_version_parts {
    # Split the version into an array
    IFS='.' read -ra VERSION_PARTS <<< "$VERSION"
    
    MAJOR=${VERSION_PARTS[0]}
    MINOR=${VERSION_PARTS[1]}
    
    # Handle patch version which might contain pre-release suffix
    if [[ ${VERSION_PARTS[2]} == *-* ]]; then
        PATCH=$(echo ${VERSION_PARTS[2]} | cut -d'-' -f1)
    else
        PATCH=${VERSION_PARTS[2]}
    fi
}

function get_tags {
    echo "STEP: Get tags"
    extract_version_parts
    
    # Check if VERSION is a pre-release
    if [[ $VERSION == *-* ]]; then
        echo "Version is a pre-release"
        TAGS="-t $IMAGE:$VERSION"
    else
        echo "Version is a release"
        TAGS="-t $IMAGE:$MAJOR.$MINOR.$PATCH -t $IMAGE:$MAJOR.$MINOR -t $IMAGE:$MAJOR"
    fi
    
    echo "Tags: $TAGS"
}

function get_tags_minimal {
    echo "STEP: Get minimal tags"
    extract_version_parts
    
    # Check if VERSION is a pre-release
    if [[ $VERSION == *-* ]]; then
        echo "Version is a pre-release"
        TAGS_MIN="-t $IMAGE-minimal:$VERSION"
    else
        echo "Version is a release"
        TAGS_MIN="-t $IMAGE-minimal:$MAJOR.$MINOR.$PATCH -t $IMAGE-minimal:$MAJOR.$MINOR -t $IMAGE-minimal:$MAJOR"
    fi
    
    echo "Tags: $TAGS_MIN"
}

function build_and_push_image {
    local dockerfile=$1
    local tags=$2
    local build_args=$3
    
    echo "STEP: Building and pushing with Dockerfile: $dockerfile"
    cd .. || exit 1
    # shellcheck disable=SC2086
    docker buildx build . $build_args \
        --push \
        --platform $PLATFORMS \
        -f "$dockerfile" \
        $tags
    
    local result=$?
    cd - || exit 1
    
    if [ $result -ne 0 ]; then
        echo "Error: Docker build failed for $dockerfile"
        exit 1
    fi
    
    echo "Successfully built and pushed $dockerfile"
}

function build_and_push {
    build_and_push_image "./docker/Dockerfile" "$TAGS" "--build-arg VERSION=$VERSION"
}

function build_and_push_minimal {
    build_and_push_image "./docker/Dockerfile.minimal" "$TAGS_MIN" "--build-arg VERSION=$VERSION"
}

# Main execution
parse_args "$@"
validate_version
get_tags
get_tags_minimal

# Build images based on BUILD_TYPE
case "$BUILD_TYPE" in
    "all")
        build_and_push
        build_and_push_minimal
        ;;
    "standard")
        build_and_push
        ;;
    "minimal")
        build_and_push_minimal
        ;;
    *)
        echo "Error: Unknown build type '$BUILD_TYPE'"
        exit 1
        ;;
esac

echo "All builds completed successfully!"

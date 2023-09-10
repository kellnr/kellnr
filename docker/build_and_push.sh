#!/usr/bin/env bash

function parse_args {
    echo "STEP: Parse arguments"
    if [ $# -eq 0 ]; then
        echo "No arguments supplied"
        exit 1
    else
        IMAGE=$1
        VERSION=$2
        echo "Use image: $IMAGE"
        echo "Use version: $VERSION"
    fi
}


function get_tags {
    echo "STEP: Get tags"
    # Split the version into an array
    IFS='.' read -ra VERSION_PARTS <<< "$VERSION"

    MAJOR=${VERSION_PARTS[0]}
    MINOR=${VERSION_PARTS[1]}
    PATCH=${VERSION_PARTS[2]}

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

function build_and_push {
  echo "STEP: Build and push"
  # shellcheck disable=SC2086
  docker buildx build . --build-arg VERSION="$VERSION" \
    --push \
    --platform linux/arm/v7,linux/arm64/v8,linux/amd64 \
    $TAGS
}

parse_args "$@"
get_tags
build_and_push
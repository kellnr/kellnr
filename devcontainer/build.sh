#!/bin/bash

source devcontainer.common
echo "cont_tag: $cont_tag"

docker build --load --progress=plain -t $cont_tag -f src/Containerfile src



#!/bin/bash

source ./sourceEnv.sh

echo
echo "docker run $runArgs_concatinated $container $@"
echo
eval docker run $runArgs_concatinated $container "$@"

#!/bin/bash

source ./devcontainer.common

contName="devcontainer-"$$
contUser=$(id -un | sed 's/[^\]*\\//')
contUserGroup=$(id -u):$(id -g)
contHome=/home/$contUser
envHome=HOME=$contHome
volWorkspace=$(pwd)

runArgs=("--rm" "-it" "--security-opt" "seccomp=unconfined" 
         "--name" "$contName" 
         "--ulimit" "rtprio=99:99" 
         "--net=host"
         "-u" "$contUserGroup"
         "-v" "$build_user_data:/etc"
         "-v" "$build_user_home:$contHome"
         "-v" "$HOME/.ssh:/.ssh"
         "-v" "$HOME/.gitconfig:/.gitconfig"
         "-v" "$volWorkspace:$volWorkspace"
         "-e" "$envHome"
         "-w" "$volWorkspace")

export runArgs_concatinated=${runArgs[@]}

printf -v joined '"%s",' "${runArgs[@]}"
export runArgs_delimited="${joined%,}"

#echo
#echo $runArgs_concatinated
#echo
#echo $runArgs_delimited
#echo

export container=$cont_tag

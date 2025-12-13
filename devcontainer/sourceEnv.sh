. ./devcontainer.common

contName="devcontainer-"$$
contUser=$(id -un | sed 's/[^\]*\\//')
contHome=/home/$contUser
volWorkspace=$(cd .. && pwd)
volWorkspaceMnt=$(cd .. && pwd) # dir where the .git repo is located

runArgs=("--rm" "-it" "--security-opt" "seccomp=unconfined" 
         "--name" "$contName" 
         "--ulimit" "rtprio=99:99" 
         "--net=host"
         "-u" "$(id -u):$(id -g)"
         "-v" "$build_user_data:/etc"
         "-v" "$build_user_home:$contHome"
         "-v" "$HOME/.ssh:$contHome/.ssh"
         "-v" "$HOME/.gitconfig:$contHome/.gitconfig:ro"
         "-v" "/var/run/docker.sock:/var/run/docker.sock"
         "-v" "$volWorkspaceMnt:$volWorkspaceMnt"
         "-e" "HOME=$contHome"
         "-w" "$volWorkspace")

# Add docker group access (docker_gid is set in devcontainer.common)
if [ ! -z "$docker_gid" ]; then
    runArgs+=("--group-add" "$docker_gid")
else
    echo "Warning: 'docker' group not found. Docker socket access may fail."
fi

if [ "$x11_enabled" = true ]; then
    runArgs+=("-e" "DISPLAY=$DISPLAY")
    runArgs+=("-v" "/tmp/.X11-unix:/tmp/.X11-unix")
fi

# Add environment variables from an optional devcontainer.env file
if [ -f "devcontainer.env" ]; then
    while IFS= read -r line || [ -n "$line" ]; do
        if [[ -n "$line" ]] && [[ ! "$line" =~ ^# ]]; then
            # Evaluate the line to resolve variable references (e.g., $HOME)
            eval "expanded_line=\"$line\""
            runArgs+=("-e" "$expanded_line")
        fi
    done < "devcontainer.env"
fi

export runArgs_concatinated=${runArgs[@]}

printf -v joined '"%s",' "${runArgs[@]}"
export runArgs_delimited="${joined%,}"

#echo
#echo $runArgs_concatinated
#echo
#echo $runArgs_delimited
#echo

export container=$cont_tag

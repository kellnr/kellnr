#!/bin/bash
source devcontainer.common

inspect=$(docker volume inspect $build_user_data 2>/dev/null)

if [[ $inspect == *"[]"* ]]; then
   ./initUserVol.sh y
fi

envUser=ENV_USER=$(id -un | sed 's/[^\]*\\//')
envGroup=ENV_GROUP=$(id -gn | sed 's/[^\]*\\//' | sed 's/ //g')
envUID=ENV_UID=$(id -u)
envGID=ENV_GID=$(id -g)
envDockerGID=ENV_DOCKER_GID=$(getent group docker | cut -d: -f3)

runParams='--rm -it
           -u root
           -e '$envUser' 
           -e '$envGroup'
           -e '$envUID'
           -e '$envGID'
		   -e '$envDockerGID'
           -v '$build_user_data':/etc
           -v '$build_user_home':/tmp/home'

container=$cont_tag

echo
echo "Setting up user..."
#echo "docker run $runParams $container /setupUser.sh $@"
eval docker run $runParams $container /setupUser.sh "$@"

echo "Generating decontainer.json..."
. ./sourceEnv.sh
mkdir -p .devcontainer
echo "
{
	\"name\": \"$devcontainer_name\",
	\"image\": \"$container\",
    \"workspaceFolder\": \"$volWorkspace\",
	\"runArgs\": [ $runArgs_delimited ],
	\"customizations\": {
		\"vscode\": {
			\"extensions\": [
				\"ms-vscode.cmake-tools\",
				\"ms-vscode.cpptools\",
				\"1YiB.rust-bundle\",
				\"rust-lang.rust-analyzer\",
				\"fill-labs.dependi\"
			]
		}
	}
}
" > .devcontainer/devcontainer.json
echo
echo "Done."

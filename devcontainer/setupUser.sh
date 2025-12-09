(
cd "$(dirname "$0")" || exit
. ./devcontainer.common

echo
echo "Setting up user volumes..."
echo

if [ "$1" == "-c" ]; then
    echo "Cleaning $build_user_data vol..."
    docker volume rm $build_user_data >/dev/null
fi
if ! docker volume inspect $build_user_data >/dev/null 2>&1; then
    echo "Creating $build_user_data vol..."
    docker volume create --name $build_user_data >/dev/null
    echo "Bootstrapping $build_user_data vol..."
    runParams='--rm 
               -u root
               -v '$build_user_data':/tmp/etc'
    container=$cont_tag
    #echo "docker run $runParams $container /bootstrapUser.sh $@"
    eval docker run $runParams $container /bootstrapUserVol.sh >/dev/null
    echo    
fi

if [ "$1" == "-c" ]; then
    echo "Cleaning $build_user_home vol..."
    docker volume rm $build_user_home >/dev/null
fi
if ! docker volume inspect $build_user_home >/dev/null 2>&1; then
    echo "Creating $build_user_home vol..."
    docker volume create --name $build_user_home >/dev/null
fi

echo "Bootstrapping $build_user_home vol..."
envUser=ENV_USER=$(id -un | sed 's/[^\]*\\//')
envGroup=ENV_GROUP=$(id -gn | sed 's/[^\]*\\//' | sed 's/ //g')
envUID=ENV_UID=$(id -u)
envGID=ENV_GID=$(id -g)
envDockerGID=ENV_DOCKER_GID=$docker_gid
envBuildUserHome=ENV_BUILD_USER_HOME=$build_user_home

runParams='--rm -it
           -u root
           -e '$envUser' 
           -e '$envGroup'
           -e '$envUID'
           -e '$envGID'
		   -e '$envDockerGID'
           -e '$envBuildUserHome'		   
           -v '$build_user_data':/etc
           -v '$build_user_home':/tmp/home'

container=$cont_tag
#echo "docker run $runParams $container /bootstrapUser.sh $@"
eval docker run $runParams $container /bootstrapUser.sh "$@"

echo "Generating decontainer.json..."
. ./sourceEnv.sh
mkdir -p ../.devcontainer
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
" > ../.devcontainer/devcontainer.json
echo
echo "Done."
)
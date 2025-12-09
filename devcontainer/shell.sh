(
cd "$(dirname "$0")" || exit
. ./sourceEnv.sh

echo
echo "docker run $runArgs_concatinated $container $@"
echo
docker run "${runArgs[@]}" "$container" "$@"
)
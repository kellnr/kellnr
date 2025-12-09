(
cd "$(dirname "$0")" || exit
. ./devcontainer.common
echo "cont_tag: $cont_tag"

docker build --load --progress=plain -t $cont_tag -f src/Containerfile src

# If you need to rebuild the container, use the following command:
# docker build --no-cache --pull --load --progress=plain -t $cont_tag -f src/Containerfile src

echo "Done."
)
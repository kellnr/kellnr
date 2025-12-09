(
cd "$(dirname "$0")" || exit
. ./devcontainer.common
echo "cont_tag: $cont_tag"

docker pull $cont_tag
echo "Done."
)
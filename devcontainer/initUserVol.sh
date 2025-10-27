#!/bin/bash
source devcontainer.common

container=$cont_tag
volTmpUserData=$build_user_data:/tmp/etc

case $# in
1)
   if [ "y" == "$1" ]; then
      echo

      inspect=$(docker volume inspect $build_user_data 2>/dev/null)
      if [[ $inspect != *"[]"* ]]; then
         echo "Removing existing user data volume..."
         runParams=$build_user_data
         #echo "docker volume rm $runParams"
         docker volume rm $runParams >/dev/null
      fi

      echo "Creating new user data volume..."
      runParams='--name '$build_user_data
      #echo "docker volume create $runParams"
      docker volume create $runParams >/dev/null

      echo "Initializing user data volume..."
      runParams='--rm -u root -v '$volTmpUserData
      #echo "docker run $runParams $container /initUserVol.sh"
      eval docker run $runParams $container /initUserVol.sh >/dev/null
      echo

      echo 'Done.'
   else
      echo "Usage: initUserVol.sh y"
      echo "y = if "$build_user_data" volume exists, rm and recreate... any existing user setup will be lost"
   fi
   ;;
*)
   echo "Usage: initUserVol.sh y"
   echo "y = if "$build_user_data" volume exists, rm and recreate... any existing user setup will be lost"
   ;;
esac

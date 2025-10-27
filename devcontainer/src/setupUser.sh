#!/bin/bash

if [ ! $(getent group $ENV_GID) ]; then
   groupadd -g $ENV_GID $ENV_GROUP
else
   CURR_GROUP=$(getent group $ENV_GID | cut -d: -f1)
   groupmod -n $ENV_GROUP $CURR_GROUP > /dev/null 2>&1
fi

if [ 1 -eq $(id -un $ENV_UID > /dev/null 2>&1; echo $?) ]; then
   useradd -s /bin/bash -m -u $ENV_UID -g $ENV_GROUP $ENV_USER
else
   CURR_USER=$(id -un $ENV_UID)
   usermod --login $ENV_USER $CURR_USER
   usermod -g $ENV_GROUP $ENV_USER > /dev/null 2>&1
   mkdir /home/$ENV_USER
   cp -rT /etc/skel/ /home/$ENV_USER/
fi

# Assumes only docker-cli was installed, so no docker group has been created.
groupadd -g $ENV_DOCKER_GID docker
usermod -a -G docker $ENV_USER

echo 'umask 0002' >> /home/$ENV_USER/.bashrc
sed -i "s/#force_color_prompt=yes/force_color_prompt=yes/g" /home/$ENV_USER/.bashrc
sed -i 's/\[\\033\[00m\\\]\\\$/\[\\033\[00m\\\]\[\\\$\]/g' /home/$ENV_USER/.bashrc

echo 'set nocompatible' >> /home/$ENV_USER/.vimrc

cd /home/$ENV_USER/
ln -s /build/workspace workspace
ln -s /.ssh .ssh
ln -s /.gitconfig .gitconfig

if [ -f /tmp/home/.bashrc ]; then
   if [ "$1" == "-c" ]; then
      echo "The ~/.build-user-home is populated. Cleaning this dir, since \"-c\" was specified."
      rm -rf /tmp/home/* > /dev/null 2>&1
      rm -rf /tmp/home/.* > /dev/null 2>&1
   else
      echo "The ~/.build-user-home is populated. Freshening contents with the latest."
      echo "If you'd like to clean this dir first, re-run as \"setupUser.sh -c\"."
   fi
fi

/bin/bash -c "tar -C /home/$ENV_USER -cf - . | tar -C /tmp/home -xvf -" > /dev/null
chown $ENV_UID:$ENV_GID -R /tmp/home


#!/bin/bash

CURR_GROUP_NAME=$(getent group $ENV_GID | cut -d: -f1)
if [ -z "$CURR_GROUP_NAME" ]; then
   groupadd -g $ENV_GID $ENV_GROUP
elif [ "$CURR_GROUP_NAME" != "$ENV_GROUP" ]; then
   groupmod -n $ENV_GROUP $CURR_GROUP_NAME > /dev/null 2>&1
fi

CURR_USER=$(id -un $ENV_UID 2>/dev/null)
if [ -z "$CURR_USER" ]; then
   sed -i "s/^\(UID_MIN[[:space:]][[:space:]]*\)1000$/\1$ENV_UID/" /etc/login.defs
   useradd -s /bin/bash -m -u $ENV_UID -g $ENV_GROUP $ENV_USER
else    
   usermod --login $ENV_USER -g $ENV_GROUP -d /home/$ENV_USER $CURR_USER > /dev/null 2>&1
   mkdir -p /home/$ENV_USER
   cp -rT /etc/skel/ /home/$ENV_USER/
fi

if [ -n "$ENV_DOCKER_GID" ] && [ "$ENV_DOCKER_GID" != "0" ]; then
   groupmod -g $ENV_DOCKER_GID docker 2>/dev/null || groupadd -g $ENV_DOCKER_GID docker
   usermod -a -G docker $ENV_USER
fi

echo 'umask 0002' >> /home/$ENV_USER/.bashrc
sed -i "s/#force_color_prompt=yes/force_color_prompt=yes/g" /home/$ENV_USER/.bashrc
sed -i 's/\[\\033\[00m\\\]\\\$/\[\\033\[00m\\\]\[\\\$\]/g' /home/$ENV_USER/.bashrc

echo 'set nocompatible' >> /home/$ENV_USER/.vimrc

if [ -f /tmp/home/.bashrc ]; then
   if [ "$1" != "-c" ]; then
      echo "The $ENV_BUILD_USER_HOME vol is populated. Freshening contents with the latest."
      echo "If you'd like to clean this vol first, re-run with \"setupUser.sh -c\"."
   fi
fi

/bin/bash -c "tar -C /home/$ENV_USER -cf - . | tar -C /tmp/home -xvf -" > /dev/null
chown $ENV_UID:$ENV_GID -R /tmp/home

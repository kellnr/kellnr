#!/bin/sh
set -e

# Unprivileged user that runs kellnr. Created in the Dockerfiles.
KELLNR_USER=kellnr
DEFAULT_DATA_DIR=/var/lib/kellnr

# Pin rustup to the toolchain baked into this image so a published
# rust-toolchain.toml in an uploaded crate or a transitive dependency
# can't silently switch the compiler during docs generation. The exact
# version is captured at image build time. See issue #1176.
if [ -r /etc/kellnr/active-toolchain ]; then
    RUSTUP_TOOLCHAIN=$(cat /etc/kellnr/active-toolchain)
    export RUSTUP_TOOLCHAIN
fi

# Resolve the data directory the same way kellnr does (config file and
# KELLNR_* environment variables).
data_dir() {
    dir=$(kellnr config show 2>/dev/null | sed -n 's/^data_dir = "\(.*\)"$/\1/p' | head -n 1)
    echo "${dir:-$DEFAULT_DATA_DIR}"
}

# Data written by older images is owned by root. Hand it over to the kellnr
# user, so upgrades keep working. Only changes anything if a file or
# directory is not owned by kellnr yet.
take_ownership() {
    dir=$1
    mkdir -p "$dir"
    if [ -n "$(find "$dir" ! -user "$KELLNR_USER" -print | head -n 1)" ]; then
        echo "Changing owner of $dir to $KELLNR_USER"
        chown -R "$KELLNR_USER:$KELLNR_USER" "$dir"
    fi
}

drop_privileges() {
    if command -v su-exec >/dev/null 2>&1; then
        exec su-exec "$KELLNR_USER" "$@"
    fi
    exec setpriv --reuid="$KELLNR_USER" --regid="$KELLNR_USER" --init-groups "$@"
}

if [ "$(id -u)" = "0" ]; then
    # Custom CAs mounted to /usr/local/share/ca-certificates need root.
    update-ca-certificates
    take_ownership "$(data_dir)"
    export HOME="/home/$KELLNR_USER"
    drop_privileges kellnr start "$@"
fi

echo "Running as non-root user $(id -u), skipping update-ca-certificates and data directory ownership changes"
exec kellnr start "$@"

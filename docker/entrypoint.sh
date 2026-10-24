#!/bin/sh
set -e

# Unprivileged user that runs kellnr. Created in the Dockerfiles.
KELLNR_USER=kellnr
DEFAULT_DATA_DIR=/var/lib/kellnr
CUSTOM_CA_DIR=/usr/local/share/ca-certificates
SYSTEM_CA_DIR=/usr/share/ca-certificates
SYSTEM_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt

# Pin rustup to the toolchain baked into this image so a published
# rust-toolchain.toml in an uploaded crate or a transitive dependency
# can't silently switch the compiler during docs generation. The exact
# version is captured at image build time. See issue #1176.
if [ -r /etc/kellnr/active-toolchain ]; then
    RUSTUP_TOOLCHAIN=$(cat /etc/kellnr/active-toolchain)
    export RUSTUP_TOOLCHAIN
fi

warn() {
    echo "WARNING: $*" >&2
}

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

as_kellnr() {
    if command -v su-exec >/dev/null 2>&1; then
        su-exec "$KELLNR_USER" "$@"
    else
        setpriv --reuid="$KELLNR_USER" --regid="$KELLNR_USER" --init-groups "$@"
    fi
}

drop_privileges() {
    if command -v su-exec >/dev/null 2>&1; then
        exec su-exec "$KELLNR_USER" "$@"
    fi
    exec setpriv --reuid="$KELLNR_USER" --regid="$KELLNR_USER" --init-groups "$@"
}

# Without root, update-ca-certificates can't write the system bundle. Build a
# bundle of the system and the custom CAs in a writable location and point
# kellnr and cargo at it instead.
use_ca_bundle_without_root() {
    if [ -z "$(find "$CUSTOM_CA_DIR" -name '*.crt' 2>/dev/null | head -n 1)" ] && [ -s "$SYSTEM_CA_BUNDLE" ]; then
        return
    fi
    bundle="${TMPDIR:-/tmp}/kellnr-ca-certificates.crt"
    find "$SYSTEM_CA_DIR" "$CUSTOM_CA_DIR" -name '*.crt' -exec cat {} + > "$bundle" 2>/dev/null || true
    if [ -s "$bundle" ]; then
        echo "Using CA bundle $bundle"
        export SSL_CERT_FILE="$bundle"
        export CARGO_HTTP_CAINFO="$bundle"
    else
        warn "Could not build a CA bundle, using the system CA certificates"
    fi
}

if [ "$(id -u)" = "0" ]; then
    # Custom CAs mounted to /usr/local/share/ca-certificates need root.
    if ! update-ca-certificates; then
        warn "update-ca-certificates failed, custom CA certificates may not be trusted"
    fi
    # Kubernetes setups that drop all capabilities can't change owners or
    # switch users. Keep them running as root instead of failing the
    # upgrade, but make the problem visible.
    if ! take_ownership "$(data_dir)"; then
        warn "Could not change the owner of the data directory (missing CAP_CHOWN?), running kellnr as root." \
            "Grant CHOWN, SETUID and SETGID or run the container as uid 1000."
        exec kellnr start "$@"
    fi
    if ! as_kellnr true 2>/dev/null; then
        warn "Could not switch to user $KELLNR_USER (missing CAP_SETUID/CAP_SETGID?), running kellnr as root." \
            "Grant CHOWN, SETUID and SETGID or run the container as uid 1000."
        exec kellnr start "$@"
    fi
    export HOME="/home/$KELLNR_USER"
    drop_privileges kellnr start "$@"
fi

echo "Running as non-root user $(id -u), skipping update-ca-certificates and data directory ownership changes"
use_ca_bundle_without_root
exec kellnr start "$@"

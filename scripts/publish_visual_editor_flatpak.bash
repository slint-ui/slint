#!/usr/bin/env bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# cspell:ignore appcast awscli flatpakref gnupghome GNUPGHOME gpgconf gpgkey keyid ostree untarring

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
. "$SCRIPT_DIR/helpers.sh"

APP_ID="dev.slint.VisualEditor"
APP_TITLE="Slint Visual Editor"

# Picks the publishing prefix under visual-editor.slint.dev, and doubles as the
# OSTree branch so `flatpak info` says which channel an install came from.
CHANNEL="${SLINT_EDITOR_CHANNEL:-nightly}"
case "$CHANNEL" in
    nightly | stable) ;;
    *) die "unknown channel: $CHANNEL, expected nightly or stable" ;;
esac

BASE_URL="${SLINT_FLATPAK_BASE_URL:-https://visual-editor.slint.dev/$CHANNEL}"
REPO_URL="$BASE_URL/flatpak"
REMOTE_NAME="slint-visual-editor-$CHANNEL"

DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
# Per-architecture repositories land here, one directory each, named repo-<arch>.
ARCH_REPO_DIR="${ARCH_REPO_DIR:-$DIST_DIR/flatpak-arch-repos}"
MERGED_REPO="${MERGED_REPO:-$DIST_DIR/flatpak-repo}"
FLATPAKREF_PATH="$DIST_DIR/slint-visual-editor.flatpakref"

# The trust anchor users' remotes pin. Checked in for the same reason as
# SUPublicEDKey: a diff has to show it, because replacing it silently breaks
# every remote that already trusts the old one.
PUBLIC_KEY_PATH="$ROOT_DIR/tools/editor/packaging/linux/slint-visual-editor.gpg"

# Signing needs both halves, and half of a pair is always a mistake: a signed
# repository whose key nobody has cannot be installed, and a flatpakref naming a
# key the repository was not signed with is rejected by every client.
signing_enabled() {
    local have_private=0 have_public=0
    [ -n "${EDITOR_FLATPAK_GPG_PRIVATE_KEY:-}" ] && have_private=1
    [ -f "$PUBLIC_KEY_PATH" ] && have_public=1

    if [ "$have_private" = 1 ] && [ "$have_public" = 1 ]; then
        return 0
    fi
    if [ "$have_private" != "$have_public" ]; then
        [ "$have_private" = 1 ] &&
            die "EDITOR_FLATPAK_GPG_PRIVATE_KEY is set but $PUBLIC_KEY_PATH is not checked in"
        die "$PUBLIC_KEY_PATH is checked in but EDITOR_FLATPAK_GPG_PRIVATE_KEY is not set"
    fi
    return 1
}

R2_BUCKET="${R2_BUCKET:-visual-editor-updates}"
S3_BASE="s3://$R2_BUCKET/$CHANNEL"
S3_REPO="$S3_BASE/flatpak"

# R2 derives S3 credentials from an ordinary R2 API token, so publishing reuses
# the token wrangler already uses instead of a second credential:
#
#   Access Key ID      the token's id
#   Secret Access Key  SHA-256 of the token's value
#
# https://developers.cloudflare.com/r2/api/tokens/
derive_s3_credentials() {
    [ -n "${AWS_ACCESS_KEY_ID:-}" ] && [ -n "${AWS_SECRET_ACCESS_KEY:-}" ] && return 0
    require_tools curl
    require_env CLOUDFLARE_API_TOKEN CLOUDFLARE_ACCOUNT_ID

    AWS_SECRET_ACCESS_KEY="$(printf "%s" "$CLOUDFLARE_API_TOKEN" | sha256sum | cut -d' ' -f1)"

    # The id is not secret, so it can come from a plain variable. Account-owned
    # tokens cannot be looked up through /user/tokens/verify, which is why the
    # override exists at all.
    if [ -n "${CLOUDFLARE_R2_TOKEN_ID:-}" ]; then
        AWS_ACCESS_KEY_ID="$CLOUDFLARE_R2_TOKEN_ID"
    else
        require_tools jq
        log "Looking up the token id"
        AWS_ACCESS_KEY_ID="$(curl -sS --fail \
            -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" \
            https://api.cloudflare.com/client/v4/user/tokens/verify |
            jq -r '.result.id // empty')" ||
            die "could not verify the API token; set CLOUDFLARE_R2_TOKEN_ID for an account-owned token"
        [ -n "$AWS_ACCESS_KEY_ID" ] ||
            die "the token verified but reported no id; set CLOUDFLARE_R2_TOKEN_ID"
    fi

    AWS_ENDPOINT_URL="${AWS_ENDPOINT_URL:-https://$CLOUDFLARE_ACCOUNT_ID.r2.cloudflarestorage.com}"
    export AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_ENDPOINT_URL
}

aws_s3() {
    derive_s3_credentials
    # R2 rejects the flexible checksums the CLI started sending by default.
    AWS_DEFAULT_REGION="${AWS_DEFAULT_REGION:-auto}" \
    AWS_REQUEST_CHECKSUM_CALCULATION=when_required \
    AWS_RESPONSE_CHECKSUM_VALIDATION=when_required \
        aws s3 "$@" --endpoint-url "$AWS_ENDPOINT_URL" --only-show-errors
}

# Content-addressed, so a given path never changes contents.
IMMUTABLE_CACHE="public, max-age=31536000, immutable"
# The mutable index. Cloudflare replaces a bare no-cache when the zone's browser
# TTL is a fixed value, so the Cache Rule for the domain has to respect origin.
MUTABLE_CACHE="no-cache"

GNUPGHOME_DIR="${RUNNER_TEMP:-$DIST_DIR}/visual-editor-gnupg"

# gpg-agent outlives the command that started it and keeps its socket inside
# GNUPGHOME. Removing the directory without stopping it leaves the next import
# talking to a socket that is gone, so every teardown goes through here.
discard_gnupghome() {
    if [ -d "$GNUPGHOME_DIR" ]; then
        gpgconf --homedir "$GNUPGHOME_DIR" --kill all >/dev/null 2>&1 || true
    fi
    rm -rf "$GNUPGHOME_DIR"
}

require_tools() {
    local tool
    for tool in "$@"; do
        command -v "$tool" >/dev/null || die "$tool is required but not on PATH"
    done
}

# Merge every per-architecture repository into one. The build jobs run on
# separate runners, so this is the first point where all architectures exist
# together, and the summary can only be written once they do.
merge_repos() {
    require_tools ostree
    [ -d "$ARCH_REPO_DIR" ] || die "no per-arch repositories at $ARCH_REPO_DIR"

    local repos=()
    local repo
    for repo in "$ARCH_REPO_DIR"/repo-*; do
        [ -d "$repo" ] && repos+=("$repo")
    done
    [ "${#repos[@]}" -gt 0 ] || die "no repo-<arch> directories under $ARCH_REPO_DIR"

    log "Creating merged repository at $MERGED_REPO"
    rm -rf "$MERGED_REPO"
    # Archive mode stores one compressed file per object, which is what a
    # repository served over plain HTTP has to be.
    ostree init --repo="$MERGED_REPO" --mode=archive

    for repo in "${repos[@]}"; do
        log "Pulling every ref from $(basename "$repo")"
        # No ref arguments: take the app plus whatever .Debug and .Locale refs
        # flatpak-builder produced alongside it.
        ostree --repo="$MERGED_REPO" pull-local "$repo"
    done

    log "Merged repository contains:"
    ostree --repo="$MERGED_REPO" refs
}

gpg_key_id() {
    gpg --homedir "$GNUPGHOME_DIR" --list-secret-keys --with-colons 2>/dev/null |
        awk -F: '/^fpr:/ { print $10; exit }'
}

import_signing_key() {
    require_tools gpg
    require_env EDITOR_FLATPAK_GPG_PRIVATE_KEY
    [ -f "$PUBLIC_KEY_PATH" ] || die "checked-in public key missing: $PUBLIC_KEY_PATH"

    log "Importing the signing key into a throwaway GNUPGHOME"
    discard_gnupghome
    mkdir -p "$GNUPGHOME_DIR"
    chmod 700 "$GNUPGHOME_DIR"
    printf "%s" "$EDITOR_FLATPAK_GPG_PRIVATE_KEY" |
        gpg --homedir "$GNUPGHOME_DIR" --batch --quiet --import

    local key_id
    key_id="$(gpg_key_id)"
    [ -n "$key_id" ] || die "no secret key present after import"

    # The same class of mistake as signing an appcast with a key the app does
    # not trust: it builds green and fails on users' machines.
    local checked_in
    checked_in="$(gpg --homedir "$GNUPGHOME_DIR" --with-colons --import-options show-only \
        --import "$PUBLIC_KEY_PATH" 2>/dev/null | awk -F: '/^fpr:/ { print $10; exit }')"
    [ -n "$checked_in" ] || die "could not read a fingerprint from $PUBLIC_KEY_PATH"
    if [ "$key_id" != "$checked_in" ]; then
        local message="EDITOR_FLATPAK_GPG_PRIVATE_KEY ($key_id) is not the private half of $PUBLIC_KEY_PATH ($checked_in)"
        discard_gnupghome
        die "$message"
    fi
    log "Signing key matches the checked-in public key: $key_id"
    SIGNING_KEY_ID="$key_id"
}

# Signs the commits when a key is configured, then writes the summary either
# way. No --generate-static-deltas: a delta needs an ancestor to diff against,
# and a channel that publishes a fresh repository every run never has one, so
# clients fall back to fetching whole objects regardless.
update_repo() {
    require_tools flatpak
    [ -d "$MERGED_REPO" ] || die "merged repository missing: $MERGED_REPO"

    if ! signing_enabled; then
        log "No signing key configured; publishing an unsigned repository"
        flatpak build-update-repo "$MERGED_REPO"
        return
    fi

    SIGNING_KEY_ID=""
    import_signing_key

    log "Signing every commit in the repository"
    flatpak build-sign "$MERGED_REPO" --gpg-sign="$SIGNING_KEY_ID" --gpg-homedir="$GNUPGHOME_DIR"

    log "Writing the repository summary"
    flatpak build-update-repo "$MERGED_REPO" \
        --gpg-sign="$SIGNING_KEY_ID" --gpg-homedir="$GNUPGHOME_DIR"

    discard_gnupghome
    log "Repository signed"
}

write_flatpakref() {
    mkdir -p "$DIST_DIR"

    local title="$APP_TITLE"
    [ "$CHANNEL" = nightly ] && title="$APP_TITLE (Nightly)"

    # Omitting GPGKey is what tells the client the remote is unsigned; there is
    # no separate opt-out to set.
    local gpg_line=""
    if signing_enabled; then
        gpg_line="GPGKey=$(base64 < "$PUBLIC_KEY_PATH" | tr -d '\n')"
    fi

    log "Writing $FLATPAKREF_PATH"
    {
        cat <<EOF
[Flatpak Ref]
Title=$title
Name=$APP_ID
Branch=$CHANNEL
Url=$REPO_URL
SuggestRemoteName=$REMOTE_NAME
IsRuntime=false
EOF
        [ -n "$gpg_line" ] && printf "%s\n" "$gpg_line"
        printf "RuntimeRepo=%s\n" "https://flathub.org/repo/flathub.flatpakrepo"
    } > "$FLATPAKREF_PATH"
}

# Three passes, and the order is the whole point. Objects go up first and
# nothing is deleted, so the repository stays servable from the old summary
# throughout. The summary then flips to the new commit. Only afterwards is
# anything removed.
publish() {
    require_tools aws
    [ -d "$MERGED_REPO" ] || die "merged repository missing: $MERGED_REPO"
    [ -f "$MERGED_REPO/summary" ] || die "$MERGED_REPO has no summary; run update-repo first"

    # --size-only because untarring the per-arch repositories resets every
    # modification time, and object paths are content hashes: same name is same
    # bytes, so size agreeing is enough to skip an upload.
    log "1/3 uploading objects, adding only"
    aws_s3 sync "$MERGED_REPO/objects" "$S3_REPO/objects" \
        --size-only \
        --cache-control "$IMMUTABLE_CACHE"

    # Not sync and not --size-only: a rewritten summary can be exactly as long
    # as the one it replaces, and skipping it would strand the new commit.
    log "2/3 uploading the summary and refs, which makes the new commit live"
    aws_s3 cp "$MERGED_REPO" "$S3_REPO" \
        --recursive \
        --exclude "objects/*" \
        --exclude "tmp/*" \
        --cache-control "$MUTABLE_CACHE"

    # Everything still referenced kept its content hash, so it is present in the
    # new repository too and survives. Only genuinely dead objects go, and pass
    # one already uploaded everything, so this pass only deletes.
    #
    # A client that read the old summary moments ago can ask for something this
    # deletes. That surfaces as a retryable 404 and the next attempt succeeds;
    # if it ever becomes noticeable, defer this pass by a day rather than trying
    # to narrow the window.
    log "3/3 removing objects the new repository no longer references"
    aws_s3 sync "$MERGED_REPO/objects" "$S3_REPO/objects" \
        --size-only \
        --delete \
        --cache-control "$IMMUTABLE_CACHE"

    log "Published to $REPO_URL"
}

publish_flatpakref() {
    require_tools aws
    [ -f "$FLATPAKREF_PATH" ] || die "flatpakref missing: $FLATPAKREF_PATH"

    log "Uploading $(basename "$FLATPAKREF_PATH")"
    aws_s3 cp "$FLATPAKREF_PATH" "$S3_BASE/" \
        --cache-control "$MUTABLE_CACHE" \
        --content-type "application/vnd.flatpak.ref"
}


# The single-file bundles are a plain download for anyone who wants to install
# once. Writing them here rather than letting the flatpak-builder action do it
# is what gives them somewhere to update from: the action only passes
# --runtime-repo, and a bundle without a repository URL installs an origin
# remote that points nowhere, so `flatpak update` has nothing to talk to.
#
# No --gpg-keys: build-bundle writes a fresh commit into the bundle and does
# not carry the repository's signature over, so a bundle that names a key
# refuses to install at all ("no signatures found"). The remote it creates is
# therefore unverified, which is the trade for a bundle being installable.
write_bundle() {
    require_tools flatpak
    local repo="${1:-}"
    local output="${2:-}"
    [ -d "$repo" ] || die "usage: write-bundle <repo> <output.flatpak>"
    [ -n "$output" ] || die "usage: write-bundle <repo> <output.flatpak>"

    log "Writing $output, updating from $REPO_URL"
    flatpak build-bundle "$repo" "$output" "$APP_ID" "$CHANNEL" \
        --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
        --repo-url="$REPO_URL"
}

publish_bundles() {
    require_tools aws
    local dir="${1:-$DIST_DIR}"
    local bundle found=0
    for bundle in "$dir"/*.flatpak; do
        [ -f "$bundle" ] || continue
        found=1
        log "Uploading $(basename "$bundle")"
        aws_s3 cp "$bundle" "$S3_BASE/" \
            --cache-control "$MUTABLE_CACHE" \
            --content-type "application/vnd.flatpak"
    done
    [ "$found" -eq 1 ] || die "no .flatpak bundles found in $dir"
}

# One-off, for a human setting the channel up. The key belongs in the team
# password manager before it goes anywhere near a CI secret, because GitHub
# secrets cannot be read back out.
generate_key() {
    require_tools gpg
    local out_dir="${1:-$DIST_DIR}"
    mkdir -p "$out_dir"
    local home="$out_dir/gnupg-new-key"
    rm -rf "$home"
    mkdir -p "$home"
    chmod 700 "$home"

    log "Generating a signing key"
    gpg --homedir "$home" --batch --quiet --passphrase "" --quick-generate-key \
        "Slint Visual Editor <info@slint.dev>" ed25519 sign never

    local key_id
    key_id="$(gpg --homedir "$home" --list-secret-keys --with-colons |
        awk -F: '/^fpr:/ { print $10; exit }')"

    gpg --homedir "$home" --export "$key_id" > "$out_dir/slint-visual-editor.gpg"
    gpg --homedir "$home" --export-secret-keys --armor "$key_id" \
        > "$out_dir/slint-visual-editor-private.asc"
    chmod 600 "$out_dir/slint-visual-editor-private.asc"

    cat <<EOF

Key $key_id

  public   $out_dir/slint-visual-editor.gpg
           Check this in at $PUBLIC_KEY_PATH

  private  $out_dir/slint-visual-editor-private.asc
           1. Put it in the password manager first.
           2. gh secret set EDITOR_FLATPAK_GPG_PRIVATE_KEY --repo slint-ui/slint < that file
           3. Delete the local copy, and this GNUPGHOME: rm -rf $home
EOF
}

full_publish() {
    merge_repos
    update_repo
    write_flatpakref
    publish
    publish_flatpakref
}

COMMAND="${1:-full}"

case "$COMMAND" in
    merge-repos) merge_repos ;;
    update-repo) update_repo ;;
    write-flatpakref) write_flatpakref ;;
    write-bundle) write_bundle "${2:-}" "${3:-}" ;;
    publish) publish ;;
    publish-flatpakref) publish_flatpakref ;;
    publish-bundles) publish_bundles "${2:-}" ;;
    generate-key) generate_key "${2:-}" ;;
    repo-url) printf "%s\n" "$REPO_URL" ;;
    full) full_publish ;;
    *) die "unknown command: $COMMAND" ;;
esac

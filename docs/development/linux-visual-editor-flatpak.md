<!-- cspell:ignore awscli flatpakref gpgkey minioadmin ostree untarring -->

# Linux Visual Editor Flatpak

This will become part of the docs later, but for now, this is a placeholder.

## CI entry point

`.github/workflows/visual_editor_linux_flatpak.yaml` builds the app for x86_64
and aarch64. It runs on pull requests against the `visual-editor` branch, on
manual dispatch, and as a reusable workflow.

`.github/workflows/visual_editor_nightly.yaml` calls it once a night alongside
the macOS build, and publishes the result. The two platforms publish from
separate jobs so one failing doesn't hold the other back.

Each build produces two things: a single-file `.flatpak` bundle for people who
just want to install once, and the OSTree repository that bundle was exported
from, which is what makes `flatpak update` work. The repository is tarred before
upload, because an archive-mode repository is tens of thousands of small files
and the artifact upload charges per file.

## What gets published

Into the `visual-editor-updates` bucket, which serves `visual-editor.slint.dev`:

```text
nightly/flatpak/                            OSTree repository
nightly/slint-visual-editor.flatpakref      installs the app and adds the remote
nightly/slint-visual-editor-x86_64.flatpak  one-off download, no updates
nightly/slint-visual-editor-aarch64.flatpak
```

`stable/` is reserved and the channel logic accepts it, but nothing publishes
there until there is a tagged release.

Users install with:

```sh
flatpak install https://visual-editor.slint.dev/nightly/slint-visual-editor.flatpakref
```

## No history

Every run exports into a brand new repository and publishes it wholesale.
Nothing is carried forward, so there is no sync-down at the start of a run and
nothing to prune.

This was measured rather than assumed. A client updating to a commit that shares
no history with the one it has installed succeeds cleanly, with no warning and a
clean `ostree fsck`, because the client keeps only the commit it installed and
has no use for ancestry. OSTree is content-addressed, so unchanged files are
skipped whatever the history: in a test with 30 MB unchanged and a 5 MB file
partly rewritten, neither the connected nor the disconnected client re-fetched
the 30 MB.

What it costs is static deltas. A delta needs an ancestor to diff against, so
the disconnected client fetches whole changed objects instead: 4.8 MB against
0.1 MB for the same change with history. That is the accepted trade for a much
simpler pipeline.

`--generate-static-deltas` is therefore deliberately not passed. The test watched
a disconnected client fetch zero delta objects even with deltas present in the
repository, so generating them would spend build time and repository space on
something nothing reads.

## Publishing

`scripts/publish_visual_editor_flatpak.bash` drives it, and the phases can be
run individually:

```sh
./scripts/publish_visual_editor_flatpak.bash merge-repos
./scripts/publish_visual_editor_flatpak.bash update-repo
./scripts/publish_visual_editor_flatpak.bash write-flatpakref
./scripts/publish_visual_editor_flatpak.bash publish
./scripts/publish_visual_editor_flatpak.bash publish-flatpakref
```

`merge-repos` combines the per-architecture repositories the build jobs produced.
The builds run on separate runners, so this is the first point at which every
architecture exists together, and the summary can only be written once they do.

`publish` uploads in three passes, and the order is the whole point:

1. `aws s3 sync` the objects, without `--delete`. Adding only, so the repository
   stays servable from the old summary throughout. `--size-only`, because
   untarring resets every modification time and an object path is a content
   hash: same name is same bytes.
2. `aws s3 cp --recursive` the summary and refs. The new commit goes live at
   this moment. Deliberately not `sync --size-only`: a rewritten summary can be
   exactly as long as the one it replaces, and skipping it would strand the new
   commit in the bucket with nothing pointing at it.
3. `aws s3 sync --delete` the objects. Everything still referenced kept its
   content hash, so it exists in the new repository too and survives; only
   genuinely dead objects go. Pass one already uploaded everything, so this pass
   only deletes.

`--delete` is the right tool here rather than a hazard, precisely because a
no-history repository is complete and authoritative by construction. Point it at
a partial or half-built local repository and it will empty the published one.

The third pass has a live window: a client that read the old summary moments
earlier can ask for an object it has just deleted. It surfaces as a retryable
404 and the next attempt succeeds. If it ever becomes noticeable, defer the pass
by a day rather than trying to narrow the window.

`wrangler` cannot do this job. It uploads one object at a time, and it has no
way to list objects, so the third pass could not be written at all.

## Caching

Set at upload time, because a repository served from cache is a repository that
lies about which commit is current.

| path | `Cache-Control` |
|---|---|
| `flatpak/objects/**` | `public, max-age=31536000, immutable` |
| everything else under `flatpak/` | `no-cache` |
| `slint-visual-editor.flatpakref` | `no-cache` |

Cloudflare replaces a bare `no-cache` when the zone's browser TTL is set to a
fixed value, so the Cache Rule for `visual-editor.slint.dev` has to be set to
respect origin TTLs. Without it the summary can be served stale for hours.

Do **not** add an R2 lifecycle rule to the flatpak prefix. Objects are shared
across commits, so age-based expiry would delete objects a current commit still
references. Pruning is OSTree's job and only OSTree's. The lifecycle rule on
`nightly/builds/` for the macOS DMGs is correct and this looks superficially
like it, which is exactly why it is worth writing down.

## Signing

**Not enabled yet.** The repository is published unsigned, so clients install
without verifying anything, and the bucket is the trust boundary: HTTPS protects
the transfer, but anyone who can write to `visual-editor-updates` can serve
arbitrary code to every nightly user. That is a deliberate trade to get the
channel running, and it should not survive into a stable channel.

Signing turns on when both halves of a key are present, and the script refuses
to run with only one, because half a pair is always a mistake: a signed
repository whose key nobody has cannot be installed, and a flatpakref naming a
key the repository was not signed with is rejected by every client.

To enable it:

```sh
./scripts/publish_visual_editor_flatpak.bash generate-key
```

Then, in this order:

1. Put the private key in the team password manager. GitHub secrets cannot be
   read back out, so a key that exists only there cannot be backed up later.
2. `gh secret set EDITOR_FLATPAK_GPG_PRIVATE_KEY --repo slint-ui/slint < the-private-key`
3. Check the public key in at `tools/editor/packaging/linux/slint-visual-editor.gpg`.
4. Delete the local copies.

Steps 2 and 3 have to land together. Either alone fails the build, which is the
intent.

The public key is checked in for the same reason as `SUPublicEDKey`: it is a
trust anchor that users' remotes pin, so a diff has to show it. It reaches users
through the `GPGKey=` field of the `.flatpakref`, base64 encoded, and omitting
that field is exactly what tells a client the remote is unsigned.

Once enabled, two different things get signed. `build-sign` attaches a signature
to each commit as `objects/<hash>.commitmeta`, proving the content. The summary
signature proves which commit is *current*, without which anyone who could write
to the bucket could serve an old summary and freeze users on a stale build.

`update-repo` refuses to run if the secret is not the private half of the
checked-in public key. That mismatch is the failure mode worth guarding: it
builds green and fails only on users' machines.

The key is generated without an expiry on purpose. An expired signing key would
make every existing install start rejecting updates on a date nobody remembers
setting, with no build failure to warn you.

## CI secrets

- `VISUAL_EDITOR_R2_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`: the same two the
  macOS job hands to wrangler. Nothing else is needed.
- `EDITOR_FLATPAK_GPG_PRIVATE_KEY`: optional, and unset today. Setting it turns
  signing on, and requires the public key to be checked in at the same time.

There is no separate S3 credential, because R2 derives one from an ordinary API
token: the Access Key ID is the token's id, and the Secret Access Key is the
SHA-256 of the token's value. See
<https://developers.cloudflare.com/r2/api/tokens/>. The script hashes the token
itself and looks the id up through `/user/tokens/verify`.

That lookup only works for user-owned tokens. For an account-owned token, set
the id in the `VISUAL_EDITOR_R2_TOKEN_ID` repository variable instead: it is an
identifier rather than a secret, so it does not need to be one.

## Local verification

The publishing path can be exercised without building the real app, which takes
two hours. Point the script at a local rclone remote and a dummy app, and the
merge, the key guard, the flatpakref, and the three-pass ordering can all be
checked in a container in under a minute:

```sh
export AWS_ENDPOINT_URL=http://localhost:9000 AWS_DEFAULT_REGION=auto
export AWS_ACCESS_KEY_ID=minioadmin AWS_SECRET_ACCESS_KEY=minioadmin
```

Setting both keys short-circuits the credential derivation, so any
S3-compatible endpoint works, MinIO included.

What that cannot check is the flatpak-builder invocation itself.

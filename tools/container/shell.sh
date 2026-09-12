#!/usr/bin/env bash
#
# A Linux dev shell on a macOS host, through Apple's `container` CLI.
#
#     tools/container/shell.sh                    # interactive shell
#     tools/container/shell.sh scripts/check.sh   # run one command in it
#     OJ_ARCH=amd64 tools/container/shell.sh      # x86_64 Linux, under Rosetta
#
# The container is long-lived and holds the nix store, so only the first run
# pays for the toolchain. The repository is bind-mounted at /work rather than
# copied, so an edit on either side is the same file; each guest builds into
# `target-<arch>/` rather than `target/`, which is the host's.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
arch="${OJ_ARCH:-arm64}"
name="oj-linux-$arch"
image="${OJ_CONTAINER_IMAGE:-docker.io/nixos/nix:2.31.2}"

# macOS ships bash 3.2, where `"${empty[@]}"` is an error under `set -u`; an
# array of flags is built as a string and split on purpose instead.
case "$arch" in
  arm64) create_flags="" extra_conf="" ;;
  # Rosetta translates x86-64, but not the seccomp BPF program nix installs
  # around a build, which fails to load and takes the build with it.
  amd64) create_flags="--rosetta" extra_conf="filter-syscalls = false" ;;
  *) echo "OJ_ARCH must be arm64 or amd64, not '$arch'" >&2; exit 2 ;;
esac

exec_flags=""
[ -t 0 ] && exec_flags="--tty --interactive"

if ! container inspect "$name" >/dev/null 2>&1; then
  # shellcheck disable=SC2086
  container run -d --name "$name" --arch "$arch" $create_flags \
    --cpus "${OJ_CONTAINER_CPUS:-8}" --memory "${OJ_CONTAINER_MEMORY:-12g}" \
    -v "$root":/work -w /work \
    "$image" sleep infinity >/dev/null
  # Flakes are not on by default in the image, and `nix develop` is a flake
  # command; `max-jobs` lets the guest use the cores it was given.
  container exec "$name" sh -c \
    "printf 'experimental-features = nix-command flakes\nmax-jobs = auto\n$extra_conf\n' >> /etc/nix/nix.conf"
elif ! container inspect "$name" | grep -q '"state" : "running"'; then
  container start "$name" >/dev/null
fi

# A target directory per architecture, so that the host's Mach-O build tree and
# each guest's ELF one do not evict each other. It stays inside the checkout on
# purpose: `oj` finds the modules it ships by walking up from its own binary,
# and a build tree outside the repository has no `modules/` above it — which
# makes `the_distribution_is_found_without_being_pointed_at` fail, or worse,
# pass only when a sibling test happens to have set `OJ_MODULES` first.
# shellcheck disable=SC2086
exec container exec $exec_flags \
  --env "CARGO_TARGET_DIR=/work/target-$arch" \
  --workdir /work "$name" \
  nix develop /work --command "${@:-bash}"

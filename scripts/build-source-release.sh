#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/build-source-release.sh [GIT_REF] [OUTPUT_DIRECTORY]

Build a deterministic, source-only ADRProof tarball and its SHA-256 checksum.
GIT_REF defaults to HEAD and OUTPUT_DIRECTORY defaults to dist.
EOF
}

if [[ ${1:-} == "-h" || ${1:-} == "--help" ]]; then
  usage
  exit 0
fi

if (( $# > 2 )); then
  usage >&2
  exit 2
fi

git_ref=${1:-HEAD}
output_argument=${2:-dist}
repository_root=$(git rev-parse --show-toplevel)

if [[ $git_ref == -* ]]; then
  echo "error: Git reference must not begin with '-'" >&2
  exit 2
fi

git -C "$repository_root" cat-file -e "${git_ref}^{commit}"

manifest=$(git -C "$repository_root" show "${git_ref}:Cargo.toml")
package_name=$(printf '%s\n' "$manifest" | sed -n 's/^name = "\([^"]*\)"$/\1/p' | head -n 1)
package_version=$(printf '%s\n' "$manifest" | sed -n 's/^version = "\([^"]*\)"$/\1/p' | head -n 1)

if [[ -z $package_name || -z $package_version ]]; then
  echo "error: could not read package name and version from ${git_ref}:Cargo.toml" >&2
  exit 1
fi

if [[ ! $package_name =~ ^[A-Za-z0-9._-]+$ || ! $package_version =~ ^[A-Za-z0-9.+-]+$ ]]; then
  echo "error: package name or version is unsafe for an archive path" >&2
  exit 1
fi

if [[ $output_argument = /* ]]; then
  output_directory=$output_argument
else
  output_directory="$repository_root/$output_argument"
fi

mkdir -p "$output_directory"
temporary_directory=$(mktemp -d)
trap 'rm -rf "$temporary_directory"' EXIT

archive_name="${package_name}-${package_version}-source.tar.gz"
archive_path="$output_directory/$archive_name"
checksum_path="$archive_path.sha256"
prefix="${package_name}-${package_version}/"

git -C "$repository_root" archive \
  --format=tar \
  --prefix="$prefix" \
  "$git_ref" > "$temporary_directory/source.tar"
gzip -n -9 -c "$temporary_directory/source.tar" > "$archive_path"

if command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$output_directory"
    sha256sum "$archive_name" > "${archive_name}.sha256"
  )
elif command -v shasum >/dev/null 2>&1; then
  (
    cd "$output_directory"
    shasum -a 256 "$archive_name" > "${archive_name}.sha256"
  )
else
  echo "error: sha256sum or shasum is required" >&2
  exit 1
fi

echo "Created $archive_path"
echo "Created $checksum_path"

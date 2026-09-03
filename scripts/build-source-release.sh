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

commit=$(git -C "$repository_root" rev-parse --verify "${git_ref}^{commit}")
tree=$(git -C "$repository_root" rev-parse --verify "${commit}^{tree}")

manifest=$(git -C "$repository_root" show "${commit}:Cargo.toml")
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

if ! grep -Eq '^publish = false[[:space:]]*$' <<< "$manifest"; then
  echo "error: ${git_ref}:Cargo.toml must declare publish = false" >&2
  exit 1
fi

tag_name=
if [[ $git_ref == refs/tags/* ]]; then
  tag_name=${git_ref#refs/tags/}
elif git -C "$repository_root" show-ref --verify --quiet "refs/tags/$git_ref"; then
  tag_name=$git_ref
fi
if [[ -n $tag_name && ${tag_name#v} != "$package_version" ]]; then
  echo "error: tag $tag_name does not match package version $package_version" >&2
  exit 1
fi

validate_release_paths() {
  local path base
  while IFS= read -r -d '' path; do
    base=${path##*/}
    case "/$path/" in
      */target/*|*/dist/*|*/.git/*|*/.adrproof/*)
        echo "error: forbidden release path in $git_ref: $path" >&2
        return 1
        ;;
    esac
    case "$base" in
      .env.example)
        ;;
      .env|.env.*|*.key|*.pem|*.p12|*.pfx|*.jks|*.keystore|*signing-key*)
        echo "error: possible secret or private key in $git_ref: $path" >&2
        return 1
        ;;
    esac
  done < <(git -C "$repository_root" ls-tree -r -z --name-only "$commit")
}

validate_release_paths

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
release_manifest_name="${package_name}-${package_version}-release-manifest.json"
release_manifest_path="$output_directory/$release_manifest_name"
prefix="${package_name}-${package_version}/"

git -C "$repository_root" archive \
  --format=tar \
  --prefix="$prefix" \
  "$commit" > "$temporary_directory/source.tar"
gzip -n -9 -c "$temporary_directory/source.tar" > "$archive_path"

if command -v sha256sum >/dev/null 2>&1; then
  archive_sha256=$(sha256sum "$archive_path" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
  archive_sha256=$(shasum -a 256 "$archive_path" | awk '{print $1}')
else
  echo "error: sha256sum or shasum is required" >&2
  exit 1
fi

printf '%s  %s\n' "$archive_sha256" "$archive_name" > "$checksum_path"
printf '{\n  "schema_version": "adrproof-source-release-manifest-v1",\n  "package": "%s",\n  "version": "%s",\n  "git_commit": "%s",\n  "git_tree": "%s",\n  "archive": "%s",\n  "sha256": "%s"\n}\n' \
  "$package_name" \
  "$package_version" \
  "$commit" \
  "$tree" \
  "$archive_name" \
  "$archive_sha256" > "$release_manifest_path"

echo "Created $archive_path"
echo "Created $checksum_path"
echo "Created $release_manifest_path"

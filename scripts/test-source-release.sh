#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
builder="$script_directory/build-source-release.sh"
repository_root=$(git -C "$script_directory" rev-parse --show-toplevel)
test_root=$(mktemp -d)
trap 'rm -rf "$test_root"' EXIT

fail() {
  echo "source release test failed: $*" >&2
  exit 1
}

init_repository() {
  local directory=$1
  local package_name=${2:-fixture}
  local package_version=${3:-1.0.0}
  local publish_value=${4:-false}
  mkdir -p "$directory"
  git init -q "$directory"
  git -C "$directory" config user.name "ADRProof tests"
  git -C "$directory" config user.email "tests@adrproof.invalid"
  printf '[package]\nname = "%s"\nversion = "%s"\npublish = %s\n' \
    "$package_name" "$package_version" "$publish_value" > "$directory/Cargo.toml"
  printf 'tracked\n' > "$directory/tracked.txt"
  git -C "$directory" add Cargo.toml tracked.txt
  git -C "$directory" commit -qm initial
}

commit_all() {
  local directory=$1
  git -C "$directory" add -A
  git -C "$directory" commit -qm fixture
}

expect_failure() {
  local label=$1
  local expected=$2
  local directory=$3
  shift 3
  local output="$test_root/${label}.log"
  if (cd "$directory" && "$builder" "$@") > "$output" 2>&1; then
    fail "$label unexpectedly succeeded"
  fi
  if ! grep -Fq "$expected" "$output"; then
    cat "$output" >&2
    fail "$label did not report: $expected"
  fi
}

first="$test_root/first"
second="$test_root/second"
"$builder" HEAD "$first"
"$builder" HEAD "$second"

head_manifest=$(git -C "$repository_root" show HEAD:Cargo.toml)
package_name=$(printf '%s\n' "$head_manifest" | sed -n 's/^name = "\([^"]*\)"$/\1/p' | head -n 1)
package_version=$(printf '%s\n' "$head_manifest" | sed -n 's/^version = "\([^"]*\)"$/\1/p' | head -n 1)
archive_name="${package_name}-${package_version}-source.tar.gz"
checksum_name="${archive_name}.sha256"
manifest_name="${package_name}-${package_version}-release-manifest.json"
cmp "$first/$archive_name" "$second/$archive_name"
cmp "$first/$checksum_name" "$second/$checksum_name"
cmp "$first/$manifest_name" "$second/$manifest_name"
(cd "$first" && sha256sum -c "$checksum_name")

expected_files="$test_root/expected-files"
archived_files="$test_root/archived-files"
git -C "$repository_root" ls-tree -r --name-only HEAD > "$expected_files"
tar -tzf "$first/$archive_name" \
  | sed "s#^${package_name}-${package_version}/##" \
  | sed -e '/\/$/d' -e '/^$/d' > "$archived_files"
diff -u "$expected_files" "$archived_files"

expected_commit=$(git -C "$repository_root" rev-parse HEAD)
expected_tree=$(git -C "$repository_root" rev-parse 'HEAD^{tree}')
jq -e \
  --arg commit "$expected_commit" \
  --arg tree "$expected_tree" \
  --arg archive "$archive_name" \
  --arg package "$package_name" \
  --arg version "$package_version" \
  '.schema_version == "adrproof-source-release-manifest-v1"
    and .package == $package
    and .version == $version
    and .git_commit == $commit
    and .git_tree == $tree
    and .archive == $archive
    and (.sha256 | test("^[0-9a-f]{64}$"))' \
  "$first/$manifest_name" >/dev/null

positive="$test_root/positive"
init_repository "$positive"
printf 'example only\n' > "$positive/.env.example"
git -C "$positive" add .env.example
git -C "$positive" commit -qm example-environment
printf 'untracked secret\n' > "$positive/.env"
mkdir -p "$positive/target"
printf 'untracked build output\n' > "$positive/target/output"
mkdir -p "$positive/dist" "$positive/.adrproof/evidence"
printf 'untracked distribution\n' > "$positive/dist/output"
printf 'untracked evidence\n' > "$positive/.adrproof/evidence/private.json"
printf 'untracked key\n' > "$positive/secret.pem"
(cd "$positive" && "$builder" HEAD "$test_root/positive-output")
if tar -tzf "$test_root/positive-output/fixture-1.0.0-source.tar.gz" \
  | grep -Eq '(^|/)(target|dist|\.git|\.adrproof)(/|$)|(^|/)\.env$|secret\.pem$'; then
  fail "untracked files entered the archive"
fi
git -C "$positive" tag 1.0.0
(cd "$positive" && "$builder" 1.0.0 "$test_root/matching-tag-output")

missing_manifest="$test_root/missing-manifest"
git init -q "$missing_manifest"
git -C "$missing_manifest" config user.name "ADRProof tests"
git -C "$missing_manifest" config user.email "tests@adrproof.invalid"
printf 'no manifest\n' > "$missing_manifest/README.md"
git -C "$missing_manifest" add README.md
git -C "$missing_manifest" commit -qm initial
expect_failure missing-manifest "Cargo.toml" "$missing_manifest" HEAD "$test_root/missing-output"

unsafe_name="$test_root/unsafe-name"
init_repository "$unsafe_name" "../escape"
expect_failure unsafe-name "unsafe for an archive path" "$unsafe_name" HEAD "$test_root/unsafe-output"

publishable="$test_root/publishable"
init_repository "$publishable" fixture 1.0.0 true
expect_failure publishable "must declare publish = false" "$publishable" HEAD "$test_root/publishable-output"

mismatched_tag="$test_root/mismatched-tag"
init_repository "$mismatched_tag"
git -C "$mismatched_tag" tag 2.0.0
expect_failure mismatched-tag "does not match package version" "$mismatched_tag" 2.0.0 "$test_root/tag-output"

expect_failure invalid-ref "Git reference must not begin" "$positive" --invalid "$test_root/invalid-output"

sensitive_paths=(
  ".env.local"
  "target/private.txt"
  "dist/private.txt"
  ".adrproof/evidence/private.json"
  "keys/release.pem"
  "config/signing-key.txt"
)
counter=0
for sensitive_path in "${sensitive_paths[@]}"; do
  counter=$((counter + 1))
  sensitive_repository="$test_root/sensitive-$counter"
  init_repository "$sensitive_repository"
  mkdir -p "$(dirname "$sensitive_repository/$sensitive_path")"
  printf 'must not ship\n' > "$sensitive_repository/$sensitive_path"
  commit_all "$sensitive_repository"
  expect_failure "sensitive-$counter" "error:" "$sensitive_repository" HEAD "$test_root/sensitive-output-$counter"
done

echo "Source release tests passed"

#!/usr/bin/env bash
# Assemble the runtime data directory of the Linux build. This is the Linux counterpart of the data
# step of upstream/shanjie/scripts/build-app.sh (that script's lines ~20-110): the same file list,
# the same pinned release assets and the same SHA-256 checks, so a Linux run and a macOS bundle hold
# byte-identical data files. It never writes inside upstream/shanjie.
#
#   linux/scripts/assemble-data.sh
#
# Environment (all optional):
#   SHANJIE_DATA_OUT       output directory, relative to the repository (default build/linux/data);
#                          absolute paths and ".." are rejected.
#   SHANJIE_ASSET_CACHE    model asset cache (default ~/.cache/shanjie/assets)
#   SHANJIE_ENGLISH_CACHE  English word list input cache (default ~/.cache/shanjie/english)
#
# The model assets are not in git: they come from the release tags model-v6 (bigram.sjlm),
# classes-v5 (classes.sjc), kn-v1 (kn.sjkn) and tinylm-v5 (tinylm.sjtl), each checked against
# upstream/shanjie/data/<file>.sha256. english-words.txt is generated from the SymSpell input pinned
# in upstream/shanjie/tools/build_english_words.py and checked against its recorded SHA-256. Any
# missing source file or hash mismatch stops the run.
set -euo pipefail

fail() { echo "error: $*" >&2; exit 1; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
UP="$ROOT/upstream/shanjie"
BUILD_APP="$UP/scripts/build-app.sh"

# The staging directory lives beside the output directory, so the final mv is a rename, and rm -rf
# below can only act inside the repository (same rule as build-app.sh's OUT_DIR).
DATA_OUT="${SHANJIE_DATA_OUT:-build/linux/data}"
[[ -n "$DATA_OUT" && "$DATA_OUT" != /* && "/$DATA_OUT/" != */../* ]] \
  || fail "SHANJIE_DATA_OUT must be a relative path inside the repository"
OUT="$ROOT/$DATA_OUT"
OUT_PARENT="$(dirname "$OUT")"

ASSET_CACHE="${SHANJIE_ASSET_CACHE:-$HOME/.cache/shanjie/assets}"
EN_CACHE="${SHANJIE_ENGLISH_CACHE:-$HOME/.cache/shanjie/english}"
REPO_URL="https://github.com/Nanako0129/shanjie/releases/download"

hash_of() { sha256sum "$1" | awk '{ print $1 }'; }
pinned_hash() { awk 'NR == 1 { print $1 }' "$1"; }

[ -d "$UP" ] || fail "upstream/shanjie is missing; initialise the submodule first"
[ -f "$BUILD_APP" ] || fail "$BUILD_APP is missing; the data contract lives there"

# Sources that ship in git (build-app.sh lines 44-46).
LEXICON=(mcbpmf-data.txt overlay-add.tsv sandhi-add.tsv names-add.tsv demote.tsv)
PACKS=(acg-add.tsv acg-sources.tsv acg.json)
LICENSES=(McBopomofo-MIT.txt data.md SCOWL.txt SymSpell-MIT.txt name-suggestion-index-BSD-3-Clause.txt)
for f in "${LEXICON[@]}"; do
  [ -f "$UP/data/lexicon/$f" ] || fail "upstream/shanjie/data/lexicon/$f is missing"
done
for f in "${PACKS[@]}"; do
  [ -f "$UP/data/packs/$f" ] || fail "upstream/shanjie/data/packs/$f is missing"
done
[ -f "$UP/LICENSE" ] || fail "upstream/shanjie/LICENSE is missing"
for f in "${LICENSES[@]}"; do
  [ -f "$UP/LICENSES/$f" ] || fail "upstream/shanjie/LICENSES/$f is missing"
done

mkdir -p "$ASSET_CACHE" "$EN_CACHE" "$OUT_PARENT"
STAGE="$OUT_PARENT/.$(basename "$OUT").stage.$$"
EN_TMP="$(mktemp -d "${TMPDIR:-/tmp}/shanjie-english.XXXXXX")"
cleanup() { rm -rf "$STAGE" "$EN_TMP"; }
trap cleanup EXIT
rm -rf "$STAGE"

# fetch_asset <file> <release>: the four model files, from the cache when its hash still matches.
fetch_asset() {
  local file="$1" release="$2"
  local sha_file="$UP/data/$file.sha256"
  local dest="$ASSET_CACHE/$file" want url
  [ -f "$sha_file" ] || fail "$sha_file is missing (the pinned hash of $file)"
  want="$(pinned_hash "$sha_file")"
  if [ -f "$dest" ] && [ "$(hash_of "$dest")" = "$want" ]; then
    return 0
  fi
  rm -f "$dest" "$dest.part"
  url="$REPO_URL/$release/$file"
  curl -fL --retry 3 -o "$dest.part" "$url" || fail "cannot download $file from $url"
  mv "$dest.part" "$dest"
  [ "$(hash_of "$dest")" = "$want" ] \
    || { rm -f "$dest"; fail "$file from $release does not match SHA-256 $want"; }
}

fetch_asset bigram.sjlm model-v6
fetch_asset classes.sjc classes-v5
fetch_asset kn.sjkn kn-v1
fetch_asset tinylm.sjtl tinylm-v5

# english-words.txt (build-app.sh lines 48-68): the URL, commit and input hash live only in the
# tool, the input is cached, and the generated file is checked against data/english-words.txt.sha256.
# python3 -I (isolated) is safe here: the tool imports the standard library only.
EN_TOOL="$UP/tools/build_english_words.py"
EN_URL="$(python3 -I "$EN_TOOL" --print-url)"
EN_COMMIT="$(python3 -I "$EN_TOOL" --print-commit)"
EN_IN_SHA="$(python3 -I "$EN_TOOL" --print-input-sha256)"
EN_IN="$EN_CACHE/frequency_dictionary_en_82_765.txt"
en_input_ok() { [ -f "$EN_IN" ] && [ "$(hash_of "$EN_IN")" = "$EN_IN_SHA" ]; }
if ! en_input_ok; then
  rm -f "$EN_IN"
  curl -fsSL --retry 3 -o "$EN_IN.part" "$EN_URL" || fail "cannot download the English word list input from $EN_URL"
  mv "$EN_IN.part" "$EN_IN"
  en_input_ok || { rm -f "$EN_IN"; fail "the downloaded English word list input does not match SHA-256 $EN_IN_SHA"; }
fi
python3 -I "$EN_TOOL" --input "$EN_IN" --out "$EN_TMP/english-words.txt" >/dev/null \
  || fail "$EN_TOOL failed"
EN_OUT_SHA="$(hash_of "$EN_TMP/english-words.txt")"
EN_RECORDED="$(pinned_hash "$UP/data/english-words.txt.sha256")"
[ "$EN_OUT_SHA" = "$EN_RECORDED" ] \
  || fail "the generated english-words.txt (SHA-256 $EN_OUT_SHA) does not match upstream/shanjie/data/english-words.txt.sha256"

# Assemble into the staging directory (build-app.sh lines 95-111, 159).
mkdir -p "$STAGE/packs" "$STAGE/LICENSES"
for f in "${LEXICON[@]}"; do
  cp "$UP/data/lexicon/$f" "$STAGE/$f"
done
for f in "${PACKS[@]}"; do
  cp "$UP/data/packs/$f" "$STAGE/packs/$f"
done
# -L: the worktree's model may be a symlink.
cp -L "$ASSET_CACHE/bigram.sjlm" "$ASSET_CACHE/classes.sjc" "$ASSET_CACHE/kn.sjkn" "$ASSET_CACHE/tinylm.sjtl" "$STAGE/"
cp "$EN_TMP/english-words.txt" "$STAGE/"

cp "$UP/LICENSE" "$STAGE/LICENSES/LICENSE"
for f in "${LICENSES[@]}"; do
  cp "$UP/LICENSES/$f" "$STAGE/LICENSES/$f"
done
# CC-BY-SA-4.0-attribution.txt: the heredoc body is taken verbatim out of build-app.sh and the
# name-suggestion-index notice is appended, exactly as build-app.sh does (lines 112-159).
CC_LINE="$(grep -n "CC-BY-SA-4.0-attribution.txt\" <<'EOF'" "$BUILD_APP" | head -1 | cut -d: -f1)"
[ -n "$CC_LINE" ] || fail "cannot find the CC-BY-SA-4.0-attribution heredoc in $BUILD_APP"
awk -v start="$CC_LINE" 'NR > start { if ($0 == "EOF") exit; print }' "$BUILD_APP" \
  > "$STAGE/LICENSES/CC-BY-SA-4.0-attribution.txt"
cat "$UP/LICENSES/name-suggestion-index-BSD-3-Clause.txt" >> "$STAGE/LICENSES/CC-BY-SA-4.0-attribution.txt"
# english-words-attribution.txt: build-app.sh's unquoted heredoc, whose only expansion is $EN_COMMIT
# (it has no backslash or backtick); not CC BY-SA, so not folded into the file above.
EN_LINE="$(grep -n 'english-words-attribution.txt" <<EOF' "$BUILD_APP" | head -1 | cut -d: -f1)"
[ -n "$EN_LINE" ] || fail "cannot find the english-words-attribution heredoc in $BUILD_APP"
awk -v start="$EN_LINE" 'NR > start { if ($0 == "EOF") exit; print }' "$BUILD_APP" \
  | EN_COMMIT="$EN_COMMIT" awk '{ gsub(/\$EN_COMMIT/, ENVIRON["EN_COMMIT"]); print }' \
  > "$STAGE/LICENSES/english-words-attribution.txt"
grep -q '\$' "$STAGE/LICENSES/english-words-attribution.txt" \
  && fail "english-words-attribution heredoc has an expansion other than \$EN_COMMIT"

# SHA256SUMS: the relative path and hash of every file in the output, verifiable with
# `sha256sum -c SHA256SUMS` from inside the output directory.
(
  cd "$STAGE"
  find . -type f ! -name SHA256SUMS | sed 's|^\./||' | LC_ALL=C sort |
    while IFS= read -r f; do sha256sum "$f"; done > SHA256SUMS
)

# Replace the old output only now that everything succeeded.
rm -rf "$OUT"
mv "$STAGE" "$OUT"
echo "assembled $DATA_OUT ($(find "$OUT" -type f | wc -l) files)"

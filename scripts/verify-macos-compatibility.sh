#!/bin/sh

set -eu

app_path=${1:-src-tauri/target/universal-apple-darwin/release/bundle/macos/ClipRiva.app}
package_version=$(node -p 'JSON.parse(require("fs").readFileSync("src-tauri/tauri.conf.json", "utf8")).version')
dmg_path=${2:-src-tauri/target/universal-apple-darwin/release/bundle/dmg/ClipRiva_${package_version}_universal.dmg}
expected_minimum=${3:-13.0}
binary_path="$app_path/Contents/MacOS/clipriva"
info_plist="$app_path/Contents/Info.plist"

if [ ! -f "$binary_path" ] || [ ! -f "$info_plist" ]; then
  echo "ClipRiva bundle is incomplete: $app_path" >&2
  exit 1
fi

architectures=$(lipo -archs "$binary_path")
case " $architectures " in
  *" arm64 "*) ;;
  *)
    echo "Universal bundle is missing arm64: $architectures" >&2
    exit 1
    ;;
esac
case " $architectures " in
  *" x86_64 "*) ;;
  *)
    echo "Universal bundle is missing x86_64: $architectures" >&2
    exit 1
    ;;
esac

plist_minimum=$(/usr/libexec/PlistBuddy -c 'Print :LSMinimumSystemVersion' "$info_plist")
if [ "$plist_minimum" != "$expected_minimum" ]; then
  echo "Info.plist minimum is $plist_minimum, expected $expected_minimum" >&2
  exit 1
fi

compatibility_tmp=$(mktemp -d "${TMPDIR:-/tmp}/clipriva-compatibility.XXXXXX")
trap 'rm -rf "$compatibility_tmp"' EXIT HUP INT TERM

for architecture in arm64 x86_64; do
  thin_binary="$compatibility_tmp/clipriva-$architecture"
  lipo "$binary_path" -thin "$architecture" -output "$thin_binary"
  binary_minimum=$(otool -l "$thin_binary" | awk '
    $1 == "cmd" && $2 == "LC_BUILD_VERSION" { in_build = 1; next }
    in_build && $1 == "minos" { print $2; exit }
  ')
  if [ "$binary_minimum" != "$expected_minimum" ]; then
    echo "$architecture minimum is $binary_minimum, expected $expected_minimum" >&2
    exit 1
  fi
done

if [ -n "$dmg_path" ]; then
  if [ ! -f "$dmg_path" ]; then
    echo "DMG does not exist: $dmg_path" >&2
    exit 1
  fi
  hdiutil verify "$dmg_path" >/dev/null
  checksum=$(shasum -a 256 "$dmg_path" | awk '{ print $1 }')
else
  checksum="not-requested"
fi

echo "architectures=$architectures"
echo "minimum_system=$plist_minimum"
echo "dmg_sha256=$checksum"
echo "compatibility_result=passed"

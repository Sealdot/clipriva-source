#!/bin/sh

set -eu

binary_path=${1:-src-tauri/target/debug/clipriva}
expected_architecture=${EXPECTED_ARCHITECTURE:-$(uname -m)}
expected_minimum=${MACOSX_DEPLOYMENT_TARGET:-13.0}
config_path=${TAURI_CONFIG_PATH:-src-tauri/tauri.conf.json}

if [ ! -f "$binary_path" ]; then
  echo "Native ClipRiva binary does not exist: $binary_path" >&2
  exit 1
fi

architectures=$(lipo -archs "$binary_path")
case " $architectures " in
  *" $expected_architecture "*) ;;
  *)
    echo "Native binary is missing $expected_architecture: $architectures" >&2
    exit 1
    ;;
esac

binary_minimum=$(otool -l "$binary_path" | awk '
  $1 == "cmd" && $2 == "LC_BUILD_VERSION" { in_build = 1; next }
  in_build && $1 == "minos" { print $2; exit }
')
if [ "$binary_minimum" != "$expected_minimum" ]; then
  echo "Native binary minimum is $binary_minimum, expected $expected_minimum" >&2
  exit 1
fi

config_minimum=$(/usr/bin/plutil -extract bundle.macOS.minimumSystemVersion raw -o - "$config_path")
if [ "$config_minimum" != "$expected_minimum" ]; then
  echo "Tauri config minimum is $config_minimum, expected $expected_minimum" >&2
  exit 1
fi

echo "native_architectures=$architectures"
echo "minimum_system=$binary_minimum"
echo "native_compatibility_result=passed"

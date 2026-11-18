#!/bin/sh
# Builds the mockups, runs each in a headless simulator, and writes
# screenshots to out/. Usage: ./render.sh [dark|light] [round]
set -eu
cd "$(dirname "$0")"
appearance="${1:-dark}"
round="${2:-2}"
case "$round" in
  1) phone="pair-A-0 pair-A-1 pair-B-0 pair-B-1 pair-C-0 pair-C-1 home-A-0 home-B-0 home-C-0 chat-A-0 chat-B-0 chat-C-0 lock-A-0 lock-B-0 lock-C-0"
     ipad="" ;;
  2) phone="computers-A-0 computers-B-0 computers-C-0 new-A-0 new-B-0 new-C-0 session-A-0 session-B-0 session-C-0 term-A-0 term-B-0 term-C-0 settings-A-0 settings-B-0 settings-C-0"
     ipad="ipad-A-0 ipad-B-0 ipad-C-0" ;;
  3) phone="connect-Now-0 connect-Now-1 connect-A-0 connect-A-1 connect-B-0 connect-B-1 connect-C-0 connect-C-1 connect-D-0 connect-D-1"
     ipad="" ;;
  sync) phone="sync-A-0 sync-A-1 sync-B-0 sync-B-1 sync-C-0 sync-C-1 sync-D-0 sync-D-1"
     ipad="" ;;
  spaces) phone="spaces-A-0 spaces-A-1 spaces-B-0 spaces-B-1 spaces-C-0 spaces-C-1 spaces-D-0 spaces-D-1"
     ipad="" ;;
  mix) phone="spaces-Mix-0 spaces-Mix-1"
     ipad="" ;;
  forget) phone="forget-A-0 forget-A-1 forget-B-0 forget-B-1 forget-C-0 forget-C-1 forget-D-0 forget-D-1"
     ipad="" ;;
  newtab) phone="newtab-A-0 newtab-B-0 newtab-B-1 newtab-C-0 newtab-C-1 newtab-D-0 newtab-D-1"
     ipad="" ;;
  tabs) phone="tabs-Now-0 tabs-A-0 tabs-B-0 tabs-C-0 tabs-D-0"
     [ -n "${MOCKUP_RECORD:-}" ] && phone="tabs-Now-1 tabs-A-1 tabs-B-1 tabs-C-1 tabs-D-1"
     ipad="" ;;
  counts) phone="counts-A-0 counts-A-1 counts-B-0 counts-B-1 counts-C-0 counts-C-1 counts-D-0 counts-D-1"
     ipad="" ;;
  asked) phone="ask-A-0 ask-A-1 ask-A-2 ask-A-3"
     ipad="" ;;
  ask) phone="ask-A-0 ask-A-1 ask-B-0 ask-B-1 ask-C-0 ask-C-1 ask-D-0 ask-D-1"
     ipad="" ;;
  voice) phone="voice-A-0 voice-A-1 voice-A-2 voice-A-3 voice-B-0 voice-B-1 voice-B-2 voice-B-3 voice-C-0 voice-C-1 voice-C-2 voice-C-3 voice-D-0 voice-D-1 voice-D-2 voice-D-3"
     ipad="" ;;
  github) phone="prhead-A-0 prhead-A-1 prhead-B-0 prhead-B-1 prhead-C-0 prhead-C-1 prhead-D-0 prhead-D-1 prdetail-A-0 prdetail-A-1 prdetail-B-0 prdetail-B-1 prdetail-C-0 prdetail-C-1 prdetail-D-0 prdetail-D-1 prlist-A-0 prlist-A-1 prlist-B-0 prlist-B-1 prlist-C-0 prlist-C-1 prlist-D-0 prlist-D-1 ghsettings-A-0 ghsettings-A-1 ghsettings-A-2 ghsettings-A-3 ghsettings-B-0 ghsettings-B-1 ghsettings-C-0 ghsettings-C-1 ghsettings-D-0 ghsettings-D-1"
     ipad="" ;;
  ghsignin) phone="ghsettings-A-0 ghsettings-A-1 ghsettings-A-2 ghsettings-A-3 ghsettings-B-0 ghsettings-B-1 ghsettings-C-0 ghsettings-C-1 ghsettings-D-0 ghsettings-D-1 ghsignin-A-0 ghsignin-A-1 ghsignin-A-2 ghsignin-A-3 ghsignin-A-4 ghsignin-B-0 ghsignin-B-1 ghsignin-B-2 ghsignin-B-3 ghsignin-B-4 ghsignin-C-0 ghsignin-C-1 ghsignin-C-2 ghsignin-C-3 ghsignin-C-4"
     ipad="" ;;
  agentpr) phone="agentpr-A-0 agentpr-B-0 agentpr-C-0"
     ipad="" ;;
  *) echo "unknown round $round" >&2; exit 2 ;;
esac

udid_of() {
  xcrun simctl list devices available | grep "    $1 (" | head -1 | sed -E 's/.*\(([0-9A-F-]{36})\).*/\1/'
}

# render DEVICE SHOTS...
render() {
  device="$1"; shift
  udid=$(udid_of "$device")
  xcodebuild -project SheprdMockups.xcodeproj -scheme SheprdMockups \
    -destination "platform=iOS Simulator,id=$udid" -derivedDataPath build -quiet build
  xcrun simctl boot "$udid" 2>/dev/null || true
  xcrun simctl bootstatus "$udid" >/dev/null
  xcrun simctl ui "$udid" appearance "$appearance"
  xcrun simctl status_bar "$udid" override --time 9:41 --batteryState charged --batteryLevel 100 --wifiBars 3 --cellularBars 4
  xcrun simctl install "$udid" build/Build/Products/Debug-iphonesimulator/SheprdMockups.app
  for shot in "$@"; do
    screen=${shot%%-*}; rest=${shot#*-}; variant=${rest%%-*}; state=${rest#*-}
    xcrun simctl launch --terminate-running-process "$udid" so.pen.herdr-mockups \
      -screen "$screen" -variant "$variant" -state "$state" >/dev/null
    if [ -n "${MOCKUP_RECORD:-}" ]; then
      # Video of an animated state, for a GIF.
      sleep 1
      xcrun simctl io "$udid" recordVideo --codec h264 --force "out/$shot-$appearance.mov" >/dev/null 2>&1 &
      recorder=$!
      sleep "$MOCKUP_RECORD"
      kill -INT "$recorder"; wait "$recorder" || true
      continue
    fi
    sleep 3
    xcrun simctl io "$udid" screenshot "out/$shot-$appearance.png" >/dev/null 2>&1
  done
}

xcodegen -q
mkdir -p out
# shellcheck disable=SC2086
[ -n "$phone" ] && render "${MOCKUP_DEVICE:-iPhone 17 Pro}" $phone
# shellcheck disable=SC2086
[ -n "$ipad" ] && render "${MOCKUP_IPAD:-iPad Pro 13-inch (M5)}" $ipad
echo "wrote round $round, $appearance"

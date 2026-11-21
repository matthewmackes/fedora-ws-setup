#!/usr/bin/env bash
# Capture mm XFCE look assets into ../assets and pack assets.tar.xz
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_HOME="${SRC_HOME:-/home/mm}"
ASSETS="$ROOT/assets"

mkdir -p "$ASSETS"/{themes,icons,fonts,wallpapers,rpms}

echo "Capturing themes..."
for t in padOS-master Arctodon-Dark Arctodon-Light Arctodon-Qogir; do
  if [[ -d "$SRC_HOME/.themes/$t" ]]; then
    rsync -a --delete "$SRC_HOME/.themes/$t/" "$ASSETS/themes/$t/"
  fi
done

echo "Capturing icons..."
for i in Mackes-Carbon Mackes-Carbon-Light; do
  if [[ -d "$SRC_HOME/.icons/$i" ]]; then
    rsync -a --delete "$SRC_HOME/.icons/$i/" "$ASSETS/icons/$i/"
  fi
done

echo "Capturing fonts..."
if [[ -d "$SRC_HOME/.fonts" ]]; then
  rsync -a --delete "$SRC_HOME/.fonts/" "$ASSETS/fonts/"
fi

echo "Capturing wallpaper..."
WP_XML="$SRC_HOME/.config/xfce4/xfconf/xfce-perchannel-xml/xfce4-desktop.xml"
WP=""
if [[ -f "$WP_XML" ]]; then
  WP="$(grep -o 'value="[^"]*"' "$WP_XML" | grep -E '\.(jpg|jpeg|png|webp|jxl)' | head -1 | sed 's/value="//;s/"$//')"
fi
if [[ -n "$WP" && -f "$WP" ]]; then
  ext="${WP##*.}"
  cp -a "$WP" "$ASSETS/wallpapers/ws-wallpaper.$ext"
  # Canonical name used by look.sh
  if [[ "$ext" == "jpg" || "$ext" == "jpeg" ]]; then
    [[ -e "$ASSETS/wallpapers/ws-wallpaper.jpg" ]] || cp -a "$WP" "$ASSETS/wallpapers/ws-wallpaper.jpg"
  else
    cp -a "$WP" "$ASSETS/wallpapers/ws-wallpaper.jpg"
  fi
  echo "  wallpaper: $WP"
else
  echo "  WARNING: no wallpaper found"
fi


echo "Capturing meowmenu RPM..."
MEOW="$(ls -1 "$SRC_HOME"/Downloads/xfce4-meowmenu-plugin*.rpm 2>/dev/null | head -1 || true)"
if [[ -n "$MEOW" ]]; then
  cp -a "$MEOW" "$ASSETS/rpms/"
  echo "  $MEOW"
else
  echo "  WARNING: meowmenu RPM not found in Downloads"
fi

echo "Writing SHA256SUMS..."
(
  cd "$ASSETS"
  find . -type f -print0 | sort -z | xargs -0 sha256sum
) > "$ROOT/SHA256SUMS"

echo "Building assets.tar.xz..."
tar -C "$ROOT" -cJf "$ROOT/assets.tar.xz" assets SHA256SUMS
sha256sum "$ROOT/assets.tar.xz" | tee "$ROOT/assets.tar.xz.sha256"

echo "Done. Assets at $ASSETS and $ROOT/assets.tar.xz"

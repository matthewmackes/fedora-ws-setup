# Neighbourhood Music (`mde-music`)

Spotify-like GTK4 music client for **Jellyfin** and **Airsonic / Navidrome (Subsonic API)**.

## Features

- Server picker from Network Neighbourhood Video/Audio places (+ manual add)
- Home, Search, Library, Radio (For You / Stations / Tags / Favorites), Podcasts
- GStreamer playback with auto-advance when a track ends (not sample-accurate gapless), Direct/Auto/Transcode
- Zero-latency 10-band EQ (`nmeq`) with an Audiophile Stage tab: curve, dynamics, sketch match, presets, bypass, and delta listen
- MPRIS2 (media keys / AVRCP), Chromecast + DLNA + AirPlay cast
- Bidirectional playlist sync, LAN library snapshot + artwork cache, local + server history
- HTTP control API on port **3847**, bound to **127.0.0.1** by default (`NM_CONTROL_BIND=0.0.0.0` opts into the LAN). `GET /status`, `POST /play|/pause|/toggle|/next`

## Build / install

```bash
cd mde/crates
cargo build --release -p mde-music
sudo ../data/neighbourhood-music/install.sh
```

Binary: `neighbourhood-music`

Neighbourhood Music is licensed under the GNU Affero General Public License v3 or later.
Source for this build is the git tree that produced the binary (`mde/crates/mde-music`), including `LICENSE`.
The EQ engine is a derivative of OpenFilter EQ at the revision in `OPENFILTER.md`. The JUCE UI is not shipped.

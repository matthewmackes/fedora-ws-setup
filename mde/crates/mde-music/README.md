# Neighbourhood Music (`mde-music`)

Spotify-like GTK4 music client for **Jellyfin** and **Airsonic / Navidrome (Subsonic API)**.

## Features

- Server picker from Network Neighbourhood Video/Audio places (+ manual add)
- Home, Search, Library, Radio, Made For You, Podcasts, Radio Stations
- GStreamer playback with gapless advance, graphic EQ, Direct/Auto/Transcode
- MPRIS2 (media keys / AVRCP), Chromecast + DLNA + AirPlay cast
- Bidirectional playlist sync, LAN full-library cache, local + server history
- LAN HTTP control API on port **3847** (`GET /status`, `POST /play|/pause|/toggle|/next`)

## Build / install

```bash
cd mde/crates
cargo build --release -p mde-music
sudo ../data/neighbourhood-music/install.sh
```

Binary: `neighbourhood-music`

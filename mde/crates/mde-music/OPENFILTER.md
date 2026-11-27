# OpenFilter reference pin

The Neighbourhood Music EQ is a Rust derivative of the DSP ideas in
[OpenFilter EQ](https://github.com/asantamarian/openfilter).

Pinned revision reviewed for this port:

`ca74aad146c9539980cf4766d7baf19ed27d58a5`

The JUCE editor is not linked or shipped. Playback uses the in-tree `nmeq`
GStreamer filter and the GTK Audiophile tab. Both this crate and that
reference are AGPL-3.0-or-later.

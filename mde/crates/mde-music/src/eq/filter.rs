//! In-place F32 audio filter inserted on playbin's `audio-filter` pad.

use gstreamer as gst;
use gstreamer_audio as gst_audio;
use gstreamer_base as gst_base;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_audio::subclass::prelude::*;
use gst_base::subclass::prelude::*;
use std::sync::Once;

glib::wrapper! {
    pub struct NmEq(ObjectSubclass<imp::NmEq>) @extends gst_audio::AudioFilter, gst_base::BaseTransform, gst::Element, gst::Object;
}

pub fn ensure_registered() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = gst::Element::register(None, "nmeq", gst::Rank::NONE, NmEq::static_type());
    });
}

mod imp {
    use super::*;
    use gstreamer as gst;
    use gstreamer_audio as gst_audio;
    use gstreamer_base as gst_base;
    use gst::prelude::*;
    use gst::subclass::prelude::*;
    use gst_audio::subclass::prelude::*;
    use gst_base::subclass::prelude::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct NmEq {
        rate: Mutex<f64>,
        channels: Mutex<u32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NmEq {
        const NAME: &'static str = "NmGraphicEq";
        type Type = super::NmEq;
        type ParentType = gst_audio::AudioFilter;
    }

    impl ObjectImpl for NmEq {}
    impl GstObjectImpl for NmEq {}

    impl ElementImpl for NmEq {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static META: std::sync::OnceLock<gst::subclass::ElementMetadata> = std::sync::OnceLock::new();
            Some(META.get_or_init(|| {
                gst::subclass::ElementMetadata::new(
                    "Neighbourhood EQ",
                    "Filter/Effect/Audio",
                    "Zero-latency 10-band graphic EQ",
                    "MDE",
                )
            }))
        }
    }

    impl BaseTransformImpl for NmEq {
        const MODE: gst_base::subclass::BaseTransformMode =
            gst_base::subclass::BaseTransformMode::AlwaysInPlace;
        const PASSTHROUGH_ON_SAME_CAPS: bool = false;
        const TRANSFORM_IP_ON_PASSTHROUGH: bool = false;

        fn transform_ip(&self, buf: &mut gst::BufferRef) -> Result<gst::FlowSuccess, gst::FlowError> {
            let channels = *self.channels.lock().unwrap_or_else(|e| e.into_inner());
            if channels == 0 {
                return Ok(gst::FlowSuccess::Ok);
            }
            let mut map = buf.map_writable().map_err(|_| gst::FlowError::Error)?;
            let bytes = map.as_mut_slice();
            if bytes.len() < 4 || bytes.len() % 4 != 0 {
                return Ok(gst::FlowSuccess::Ok);
            }
            let samples = unsafe {
                std::slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut f32, bytes.len() / 4)
            };
            let mut eq = crate::eq::shared_eq().lock();
            if channels == 1 {
                eq.process_mono(samples);
            } else {
                eq.process_interleaved(samples);
            }
            Ok(gst::FlowSuccess::Ok)
        }
    }

    impl AudioFilterImpl for NmEq {
        fn allowed_caps() -> &'static gst::Caps {
            static CAPS: std::sync::OnceLock<gst::Caps> = std::sync::OnceLock::new();
            CAPS.get_or_init(|| {
                gst_audio::AudioCapsBuilder::new()
                    .format(gst_audio::AudioFormat::F32le)
                    .build()
            })
        }

        fn setup(&self, info: &gst_audio::AudioInfo) -> Result<(), gst::LoggableError> {
            *self.rate.lock().unwrap_or_else(|e| e.into_inner()) = info.rate() as f64;
            *self.channels.lock().unwrap_or_else(|e| e.into_inner()) = info.channels();
            crate::eq::shared_eq()
                .lock()
                .set_rate(info.rate() as f64);
            Ok(())
        }
    }
}

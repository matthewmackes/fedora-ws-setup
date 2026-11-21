//! OSD pill moved from main.rs.
use super::*;
use iced::widget::column;

pub(crate) fn view_osd<'a>(osd: &OsdState, palette: &Palette) -> Element<'a, Message> {
    let icon = match osd.kind {
        osd::OsdKind::Volume if osd.muted => "audio-volume-muted",
        osd::OsdKind::Volume => "audio-volume-high",
        osd::OsdKind::Brightness => "display-brightness",
        osd::OsdKind::Mic if osd.muted => "microphone-sensitivity-muted",
        osd::OsdKind::Mic => "audio-input-microphone",
        osd::OsdKind::Dictation => "audio-input-microphone",
    };
    container(
        row![
            app_face(icon, osd.label(), 20, false, palette.dark),
            column![
                text(osd.label()).size(13),
                level_track(if osd.muted { 0.0 } else { osd.level }, palette),
            ]
            .spacing(4)
            .width(Fill),
        ]
        .spacing(12)
        .padding([10, 14])
        .align_y(iced::Alignment::Center),
    )
    .width(Fill)
    .height(Fill)
    .into()
}

use std::cell::Cell;

use iced::widget::text::{self, IntoFragment};
use iced::widget::Text;
use iced::{Element, Font, Length, Pixels, Theme};

thread_local! {
    static TEXT_SCALE: Cell<f32> = const { Cell::new(1.0) };
}

pub fn set_scale(scale: f32) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    TEXT_SCALE.with(|current| current.set(scale));
}

pub fn scale() -> f32 {
    TEXT_SCALE.with(Cell::get)
}

pub fn scaled_pixels(base: f32) -> f32 {
    base * scale()
}

/// A `text` widget whose explicit size is multiplied by the current text scale.
///
/// The app sets the scale before building the view, so every existing
/// `text(...).size(...)` call follows the persisted text-size setting.
pub fn text<'a>(content: impl IntoFragment<'a>) -> ScaledText<'a> {
    ScaledText {
        inner: Text::new(content),
    }
}

pub struct ScaledText<'a> {
    inner: Text<'a>,
}

impl<'a> ScaledText<'a> {
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        let size = size.into().0;
        self.inner = self.inner.size(Pixels(scaled_pixels(size)));
        self
    }

    pub fn line_height(mut self, line_height: impl Into<text::LineHeight>) -> Self {
        self.inner = self.inner.line_height(line_height);
        self
    }

    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.inner = self.inner.font(font);
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.inner = self.inner.width(width);
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.inner = self.inner.height(height);
        self
    }

    pub fn style(mut self, style: impl Fn(&Theme) -> text::Style + 'a) -> Self {
        self.inner = self.inner.style(style);
        self
    }
}

impl<'a, Message> From<ScaledText<'a>> for Element<'a, Message> {
    fn from(value: ScaledText<'a>) -> Self {
        value.inner.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_pixels_by_the_current_text_scale() {
        set_scale(1.25);
        assert!((scaled_pixels(12.0) - 15.0).abs() < f32::EPSILON);
        set_scale(1.0);
    }

    #[test]
    fn ignores_invalid_scales() {
        set_scale(f32::NAN);
        assert_eq!(scale(), 1.0);
        set_scale(0.0);
        assert_eq!(scale(), 1.0);
        set_scale(1.0);
    }
}

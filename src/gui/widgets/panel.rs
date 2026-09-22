use crate::gui::context::UiContext;
use crate::gui::types::{Color, Margin, PanelPosition};

pub struct Panel {
    id: String,
    position: PanelPosition,
    margin: Margin,
    bg_color: Option<Color>,
}

impl Panel {
    pub fn new(id: impl Into<String>, position: PanelPosition) -> Self {
        Self {
            id: id.into(),
            position,
            margin: Margin::ZERO,
            bg_color: None,
        }
    }

    // --- Constructeurs de commodité ---

    pub fn top(id: impl Into<String>) -> Self {
        Self::new(id, PanelPosition::Top)
    }

    pub fn bottom(id: impl Into<String>) -> Self {
        Self::new(id, PanelPosition::Bottom)
    }

    pub fn left(id: impl Into<String>) -> Self {
        Self::new(id, PanelPosition::Left)
    }

    pub fn right(id: impl Into<String>) -> Self {
        Self::new(id, PanelPosition::Right)
    }

    pub fn central(id: impl Into<String>) -> Self {
        Self::new(id, PanelPosition::Central)
    }

    // --- Configuration ---

    pub fn margin(mut self, margin: Margin) -> Self {
        self.margin = margin;
        self
    }

    pub fn inner_margin(mut self, top: f32, bottom: f32) -> Self {
        self.margin.top = top;
        self.margin.bottom = bottom;
        self
    }

    pub fn bg_color(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    // --- Rendu ---

    pub fn show<R>(
        &self,
        ctx: &mut UiContext,
        add_contents: impl FnOnce(&mut UiContext) -> R,
    ) -> R {
        let mut frame = eframe::egui::Frame::new();

        frame.inner_margin = eframe::egui::Margin {
            left: self.margin.left as i8,
            right: self.margin.right as i8,
            top: self.margin.top as i8,
            bottom: self.margin.bottom as i8,
        };

        if let Some(bg) = self.bg_color {
            frame.fill = bg.into();
        }

        let panel_id = eframe::egui::Id::new(&self.id);

        match self.position {
            PanelPosition::Top => {
                eframe::egui::Panel::top(panel_id)
                    .frame(frame)
                    .show(ctx.egui_ui, |ui| {
                        let mut inner_ctx = UiContext::new(ui);
                        add_contents(&mut inner_ctx)
                    })
                    .inner
            }

            PanelPosition::Bottom => {
                eframe::egui::Panel::bottom(panel_id)
                    .frame(frame)
                    .show(ctx.egui_ui, |ui| {
                        let mut inner_ctx = UiContext::new(ui);
                        add_contents(&mut inner_ctx)
                    })
                    .inner
            }

            PanelPosition::Left => {
                eframe::egui::Panel::left(panel_id)
                    .frame(frame)
                    .show(ctx.egui_ui, |ui| {
                        let mut inner_ctx = UiContext::new(ui);
                        add_contents(&mut inner_ctx)
                    })
                    .inner
            }

            PanelPosition::Right => {
                eframe::egui::Panel::right(panel_id)
                    .frame(frame)
                    .show(ctx.egui_ui, |ui| {
                        let mut inner_ctx = UiContext::new(ui);
                        add_contents(&mut inner_ctx)
                    })
                    .inner
            }

            PanelPosition::Central => {
                eframe::egui::CentralPanel::default()
                    .frame(frame)
                    .show(ctx.egui_ui, |ui| {
                        let mut inner_ctx = UiContext::new(ui);
                        add_contents(&mut inner_ctx)
                    })
                    .inner
            }
        }
    }
}

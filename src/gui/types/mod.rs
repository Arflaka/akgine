// pub type Ui = eframe::egui::Ui;

/** vecteur a deux dimension */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/** Conversion automatique vers egui::Vec2 */
impl From<Vec2> for eframe::egui::Vec2 {
    fn from(val: Vec2) -> Self {
        eframe::egui::Vec2::new(val.x, val.y)
    }
}

/** Couleur en rgba */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/** Conversion automatique vers egui::Color32 */
impl From<Color> for eframe::egui::Color32 {
    fn from(val: Color) -> Self {
        eframe::egui::Color32::from_rgba_unmultiplied(val.r, val.g, val.b, val.a)
    }
}

/** Direction */
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Direction {
    TopDown,
    BottomUp,
    LeftToRight,
    RightToLeft,
}

/** Conversion automatique vers egui::Direction */
impl From<Direction> for eframe::egui::Direction {
    fn from(val: Direction) -> Self {
        match val {
            Direction::TopDown => eframe::egui::Direction::TopDown,
            Direction::BottomUp => eframe::egui::Direction::BottomUp,
            Direction::LeftToRight => eframe::egui::Direction::LeftToRight,
            Direction::RightToLeft => eframe::egui::Direction::RightToLeft,
        }
    }
}

/** alignement des elements */
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    /** Gauche / Haut */
    Min,
    /** Milieu */
    Center,
    /** Droite / Bas */
    Max,
}

/** Conversion automatique vers egui::Align */
impl From<Align> for eframe::egui::Align {
    fn from(val: Align) -> Self {
        match val {
            Align::Min => eframe::egui::Align::Min,
            Align::Center => eframe::egui::Align::Center,
            Align::Max => eframe::egui::Align::Max,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelPosition {
    Top,
    Bottom,
    Left,
    Right,
    Central,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Margin {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Margin {
    pub const ZERO: Self = Self {
        left: 0.0,
        right: 0.0,
        top: 0.0,
        bottom: 0.0,
    };

    pub const fn same(margin: f32) -> Self {
        Self {
            left: margin,
            right: margin,
            top: margin,
            bottom: margin,
        }
    }

    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }

    pub const fn explicit(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }
}

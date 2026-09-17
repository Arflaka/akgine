use crate::gui::context::UiContext;

/// Contexte de création abstrait fourni au démarrage de l'application.
pub struct CreationContext<'a> {
    pub(crate) egui_cc: &'a eframe::CreationContext<'a>,
}

impl<'a> CreationContext<'a> {
    pub fn new(egui_cc: &'a eframe::CreationContext<'a>) -> Self {
        Self { egui_cc }
    }
}

/// Installe les chargeurs d'images de la librairie sans exposer egui_extras.
pub fn install_image_loaders(cc: &CreationContext) {
    egui_extras::install_image_loaders(&cc.egui_cc.egui_ctx);
}

/// Trait d'application propre à akgine.
pub trait AppTrait {
    fn ui(&mut self, ctx: &mut UiContext);
}

/// Wrapper pour satisfaire les règles de traits de Rust.
pub struct AppWrapper<T: AppTrait> {
    pub inner: T,
}

// Implémentation automatique de eframe::App pour tout type implémentant akgine::AppTrait
impl<T: AppTrait> AppWrapper<T> {
    // fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
    //     let mut ctx = UiContext::new(ui);
    //     self.inner.ui(&mut ctx);
    // }
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T: AppTrait> eframe::App for AppWrapper<T> {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        let mut ui_ctx = UiContext::new(ui);
        self.inner.ui(&mut ui_ctx);
    }
}

#[cfg(target_os = "android")]
use android_activity::AndroidApp;

/// Lanceur abstrait pour les plateformes Desktop (Windows/Mac/Linux)
pub fn run_desktop_app<A, F>(
    title: &str,
    icon_bytes: Option<&[u8]>,
    creator: F,
) -> Result<(), String>
where
    A: AppTrait + 'static,
    F: FnOnce(&CreationContext<'_>) -> A + 'static,
{
    let mut options = eframe::NativeOptions::default();

    if let Some(bytes) = icon_bytes {
        if let Ok(icon) = eframe::icon_data::from_png_bytes(bytes) {
            options.viewport =
                eframe::egui::ViewportBuilder::default().with_icon(std::sync::Arc::new(icon));
        } else {
            return Err("Failed to decode icon.png — make sure it is a valid RGBA PNG".to_string());
        }
    }

    eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            let context = CreationContext::new(cc);
            Ok(Box::new(AppWrapper::new(creator(&context))))
        }),
    )
    .map_err(|e| e.to_string())
}

/// Lanceur abstrait pour Android
#[cfg(target_os = "android")]
pub fn run_android_app<A, F>(app: AndroidApp, title: &str, creator: F)
where
    A: AppTrait + 'static,
    F: FnOnce(&CreationContext<'_>) -> A + 'static,
{
    let mut options = eframe::NativeOptions::default();
    options.android_app = Some(app);
    options.viewport = eframe::egui::ViewportBuilder::default().with_fullscreen(true);

    let _ = eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            let context = CreationContext::new(cc);
            Ok(Box::new(AppWrapper::new(creator(&context))))
        }),
    );
}

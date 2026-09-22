use eframe::egui;

/** ui context of the lib */
pub struct UiContext<'a> {
    /** get the egui ui element */
    pub(crate) egui_ui: &'a mut egui::Ui,
}

impl<'a> UiContext<'a> {
    /// Utiliser uniquement en interne dans le moteur de rendu principal
    pub fn new(egui_ui: &'a mut egui::Ui) -> Self {
        Self { egui_ui }
    }

    /// Récupère un état temporaire ou l'initialise s'il n'existe pas encore.
    pub fn get_or_init_state<T: Clone + Send + Sync + 'static>(
        &mut self,
        key: &str,
        init: impl FnOnce() -> T,
    ) -> T {
        self.egui_ui.ctx().data_mut(|d| {
            let id = eframe::egui::Id::new(key);
            if let Some(state) = d.get_temp::<T>(id) {
                state
            } else {
                let new_state = init();
                d.insert_temp(id, new_state.clone());
                new_state
            }
        })
    }

    /// Permet de lire l'état des entrées (delta time, viewport, clavier, souriss, etc.)
    /// a changer vers un fichier propre
    pub fn input<R>(&self, reader: impl FnOnce(&eframe::egui::InputState) -> R) -> R {
        self.egui_ui.input(reader)
    }

    // a changer vers un fichier propre
    pub fn columns<R>(
        &mut self,
        num_columns: usize,
        add_contents: impl FnOnce(&mut [UiContext]) -> R,
    ) -> R {
        self.egui_ui.columns(num_columns, |columns| {
            let mut contexts: Vec<UiContext> =
                columns.iter_mut().map(|ui| UiContext::new(ui)).collect();
            add_contents(&mut contexts)
        })
    }

    /// Retourne la taille disponible du rectangle de contenu (largeur, hauteur)
    pub fn content_size(&self) -> (f32, f32) {
        let rect = self.egui_ui.available_rect_before_wrap();
        (rect.width(), rect.height())
    }

    /// Retourne la taille disponible du rectangle de contenu (largeur, hauteur)
    pub fn request_repaint(&self) {
        self.egui_ui.request_repaint();
    }

    // Méthodes utilitaires agnostiques que tu souhaites exposer
    // pub fn add_space(&mut self, amount: f32) {
    //     self.egui_ui.add_space(amount);
    // }
}

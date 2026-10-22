//! The main window's pages and its first-run modals.

mod cuda_offer;
mod dashboard;
mod dictionary;
mod model_manager;
mod model_picker;
mod models;
mod settings;

pub use cuda_offer::CudaOffer;
pub use dashboard::Dashboard;
pub use dictionary::Dictionary;
pub use model_picker::ModelPicker;
pub use settings::Settings;

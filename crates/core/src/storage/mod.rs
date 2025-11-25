pub mod manager;
pub mod models;
pub mod repository;
pub mod traits;

use gpui::App;
pub use manager::*;
pub use models::*;
pub use repository::*;


pub fn init(cx: &mut App){
    manager::init(cx);
    repository::init(cx);
}
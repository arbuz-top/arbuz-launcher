mod accounts;
mod ai;
mod builder;
mod catalog;
mod commands;
mod content;
mod crash;
mod error;
mod http;
mod java;
mod loaders;
mod modrinth_app;
mod mojang;
mod models;
mod news;
mod packs;
mod paths;
mod prism;
mod sandbox;
mod skins;
mod state;
mod store;
mod studio;

use state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::list_instances,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::set_instance_icon,
            commands::set_instance_icon_url,
            commands::clear_instance_icon,
            commands::open_instance_folder,
            commands::get_system_ram_mb,
            commands::list_mc_versions,
            commands::add_custom_version,
            commands::remove_custom_version,
            commands::list_loader_versions,
            commands::detect_javas,
            commands::account_status,
            commands::catalog_project,
            commands::install_java,
            commands::list_accounts,
            commands::offline_add,
            commands::set_active_account,
            commands::remove_account,
            commands::ely_login,
            commands::little_login,
            commands::ms_begin,
            commands::ms_poll,
            commands::ms_cancel,
            commands::launch_instance,
            commands::stop_instance,
            commands::list_content,
            commands::toggle_content,
            commands::toggle_isolation,
            commands::remove_content,
            commands::list_worlds,
            commands::list_screenshots,
            commands::open_screenshots_folder,
            commands::open_screenshot,
            commands::search_catalog,
            commands::install_catalog_item,
            commands::check_updates,
            commands::apply_updates,
            commands::install_modpack,
            commands::list_modpack_versions,
            commands::import_pack,
            commands::import_prism_instance,
            commands::list_modrinth_profiles,
            commands::import_modrinth_profile,
            commands::diagnose_crash,
            commands::open_crash_reports,
            commands::ai_diagnose_crash,
            commands::ai_pack_plan,
            commands::pack_info,
            commands::save_pack_info,
            commands::pack_configs,
            commands::read_pack_config,
            commands::save_pack_config,
            commands::restore_pack_config,
            commands::validate_pack,
            commands::export_pack,
            commands::open_pack_configs,
            commands::account_skin,
            commands::get_news,
            commands::open_data_folder,
            commands::set_home_wallpaper,
            commands::clear_home_wallpaper,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

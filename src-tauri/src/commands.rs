//! Thin Tauri commands: convert arguments and DTOs, call one use case and map
//! the error. No business rules live here. JS passes camelCase arguments
//! (`modIds`, `profileId`); results use snake_case DTO fields.
//!
//! Reads are plain synchronous commands. Everything that writes (imports,
//! deploys, profiles, settings) is `async`: the blocking use case runs on the
//! blocking pool, never on the main thread, and mutations are serialized
//! through `AppState::write_lock` so they cannot lose each other's updates.
use crate::dto::{
    AdDto, CatalogDto, CommunityStatusDto, ConfigDto, ConflictDto, CurrentSessionDto,
    DeploySessionDto, GameInfoDto, ModDto, ProfileDto, RecoveryStatusDto, StorageUsageDto,
};
use crate::error_ads::unknown_creative;
use crate::{AppState, CommandError};
use motogp_application::{import_mods as import_batch, remove_mod as remove_everywhere};
use motogp_domain::AdSlot;
use std::path::{Path, PathBuf};
use tauri::{Builder, Runtime, State};

type CommandResult<T> = Result<T, CommandError>;

/// Runs a blocking use case off the caller's thread, one mutation at a time.
pub(crate) async fn mutate<T, F>(state: State<'_, AppState>, job: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> CommandResult<T> + Send + 'static,
{
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _serialized = state.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        job(&state)
    })
    .await
    .map_err(|error| CommandError {
        code: "task_failed".to_string(),
        message: format!("the operation did not finish: {error}"),
    })?
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> CommandResult<ConfigDto> {
    Ok(state.settings.config()?.into())
}

#[tauri::command]
pub async fn detect_games(state: State<'_, AppState>) -> CommandResult<Vec<GameInfoDto>> {
    mutate(state, |state| {
        Ok(state
            .settings
            .detect_games()?
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn set_game_dir(state: State<'_, AppState>, path: String) -> CommandResult<GameInfoDto> {
    mutate(state, move |state| {
        Ok(state.settings.set_game_dir(Path::new(&path))?.into())
    })
    .await
}

#[tauri::command]
pub fn get_game(state: State<'_, AppState>) -> CommandResult<Option<GameInfoDto>> {
    Ok(state.settings.get_game()?.map(Into::into))
}

#[tauri::command]
pub fn list_mods(state: State<'_, AppState>) -> CommandResult<Vec<ModDto>> {
    Ok(state.library.list()?.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn import_mods(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> CommandResult<Vec<ModDto>> {
    mutate(state, move |state| {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        Ok(import_batch(&state.library, &paths)?
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn remove_mod(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    mutate(state, move |state| {
        Ok(remove_everywhere(&state.library, &state.profiles, &id)?)
    })
    .await
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> CommandResult<Vec<ProfileDto>> {
    Ok(state.profiles.list()?.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn create_profile(state: State<'_, AppState>, name: String) -> CommandResult<ProfileDto> {
    mutate(state, move |state| Ok(state.profiles.create(&name)?.into())).await
}

#[tauri::command]
pub async fn rename_profile(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CommandResult<ProfileDto> {
    mutate(state, move |state| {
        Ok(state.profiles.rename(&id, &name)?.into())
    })
    .await
}

#[tauri::command]
pub async fn delete_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    mutate(state, move |state| {
        Ok(state.profiles.delete(&id, &state.config)?)
    })
    .await
}

#[tauri::command]
pub async fn set_profile_mods(
    state: State<'_, AppState>,
    id: String,
    mod_ids: Vec<String>,
) -> CommandResult<ProfileDto> {
    mutate(state, move |state| {
        Ok(state.profiles.set_mods(&id, mod_ids)?.into())
    })
    .await
}

/// File conflicts of a candidate load order (read only, stateless).
#[tauri::command]
pub fn list_conflicts(
    state: State<'_, AppState>,
    mod_ids: Vec<String>,
) -> CommandResult<Vec<ConflictDto>> {
    Ok(state
        .library
        .list_conflicts(&mod_ids)?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[tauri::command]
pub async fn set_active_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    mutate(state, move |state| {
        Ok(state.profiles.set_active(&id, &state.config)?)
    })
    .await
}

#[tauri::command]
pub async fn play(state: State<'_, AppState>, profile_id: String) -> CommandResult<()> {
    mutate(state, move |state| Ok(state.play.play(&profile_id)?)).await
}

/// The live session right now (the interface may load after the events were
/// emitted, for example after a page reload).
#[tauri::command]
pub fn current_session(state: State<'_, AppState>) -> CurrentSessionDto {
    state.play.current_session().into()
}

/// Ends the game of the live session. Returns once the game was asked to
/// close; the cleanup that follows is reported through the `session` events.
/// It skips the write lock: it must work while other commands are busy.
#[tauri::command]
pub async fn stop_session(state: State<'_, AppState>) -> CommandResult<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.play.stop_session())
        .await
        .map_err(|error| CommandError {
            code: "task_failed".to_string(),
            message: format!("the operation did not finish: {error}"),
        })?
        .map_err(CommandError::from)
}

/// The startup recovery result. The frontend reads it after subscribing to
/// the session events, so a failure that happened before it listened is kept.
#[tauri::command]
pub fn recovery_status(state: State<'_, AppState>) -> RecoveryStatusDto {
    state.play.recovery_status().into()
}

/// How the deployment manifest relates to the current game install.
#[tauri::command]
pub fn deploy_session(state: State<'_, AppState>) -> CommandResult<DeploySessionDto> {
    Ok(state.play.deploy_session()?.into())
}

/// Recovery: drops the manifest of a previous game install (no file is touched).
#[tauri::command]
pub async fn forget_stale_session(state: State<'_, AppState>) -> CommandResult<()> {
    mutate(state, |state| Ok(state.play.forget_stale_session()?)).await
}

/// Recovery: cleans the previous install's `~mods`; `confirmed_dir` must be the
/// folder the interface showed. Returns the number of files removed.
#[tauri::command]
pub async fn clean_stale_session(
    state: State<'_, AppState>,
    confirmed_dir: String,
) -> CommandResult<usize> {
    mutate(state, move |state| {
        Ok(state.play.clean_stale_session(Path::new(&confirmed_dir))?)
    })
    .await
}

/// Recovery on demand: removes every file the launcher deployed into the
/// current install's `~mods` and returns how many. Refused while a session or
/// the game is running; also retries a failed cleanup.
#[tauri::command]
pub async fn restore_game_files(state: State<'_, AppState>) -> CommandResult<usize> {
    mutate(state, |state| Ok(state.play.restore_game_files()?)).await
}

/// Runs a read-only blocking use case off the caller's thread. It never takes
/// `write_lock`: a long measurement must not wait for, or hold up, a mutation.
pub(crate) async fn read_blocking<T, F>(state: State<'_, AppState>, job: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> CommandResult<T> + Send + 'static,
{
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || job(&state))
        .await
        .map_err(|error| {
            crate::app_log::error(&format!("a background command did not finish: {error}"));
            CommandError {
                code: "task_failed".to_string(),
                message: format!("the operation did not finish: {error}"),
            }
        })?
}

/// Size of the launcher data folder, grouped by category.
#[tauri::command]
pub async fn get_storage_usage(state: State<'_, AppState>) -> CommandResult<StorageUsageDto> {
    read_blocking(state, |state| Ok(state.storage.usage()?.into())).await
}

/// Shows the data folder in the file manager. Takes no argument: the path is
/// never chosen by the frontend.
#[tauri::command]
pub async fn open_data_folder(state: State<'_, AppState>) -> CommandResult<()> {
    read_blocking(state, |state| Ok(state.storage.open_data_folder()?)).await
}

/// The Steam artwork of the configured game as a `data:` URL, or `None` when
/// there is no game, no App ID or nothing cached. Takes no argument: the App
/// ID always comes from the saved configuration, never from the frontend.
#[tauri::command]
pub async fn get_game_artwork(state: State<'_, AppState>) -> CommandResult<Option<String>> {
    read_blocking(state, |state| {
        let app_id = state.settings.get_game()?.and_then(|game| game.app_id);
        Ok(state
            .artwork
            .artwork(app_id)?
            .as_ref()
            .map(crate::dto::artwork_data_url))
    })
    .await
}

/// The online catalog: the cached copy while fresh, else a fetch (offline
/// falls back to a stale copy). Blocking network I/O, so off the async runtime.
#[tauri::command]
pub async fn get_catalog(state: State<'_, AppState>) -> CommandResult<CatalogDto> {
    read_blocking(state, |state| Ok(state.catalog.get()?.into())).await
}

/// Forces a catalog fetch (still bounded by the backoff after a failure).
#[tauri::command]
pub async fn refresh_catalog(state: State<'_, AppState>) -> CommandResult<CatalogDto> {
    read_blocking(state, |state| Ok(state.catalog.refresh()?.into())).await
}

/// Opens the web page of a catalog entry in the default browser. The frontend
/// sends only the entry id; the URL comes from the catalog and is checked
/// against the page policy before it is opened.
#[tauri::command]
pub async fn open_catalog_page(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    read_blocking(state, move |state| {
        Ok(state.catalog.open_page(&id, state.web_opener.as_ref())?)
    })
    .await
}

/// Opens the project's support page in the default browser. Takes no
/// arguments: the URL is a constant checked against its own host policy.
#[tauri::command]
pub async fn open_support_page(state: State<'_, AppState>) -> CommandResult<()> {
    read_blocking(state, |state| {
        Ok(motogp_application::open_support_page(
            state.web_opener.as_ref(),
        )?)
    })
    .await
}

/// The sponsored creative for `slot`, or `null`. An unknown slot name, ads
/// switched off or an empty feed all mean "no ad" and never an error. The
/// frontend sends `slot` and the UI `locale` (e.g. `pt-BR`, optional);
/// `adsEnabled` defaults to on.
#[tauri::command]
pub async fn get_ad(
    state: State<'_, AppState>,
    slot: String,
    ads_enabled: Option<bool>,
    locale: Option<String>,
) -> CommandResult<Option<AdDto>> {
    read_blocking(state, move |state| {
        let Some(slot) = AdSlot::parse(&slot) else {
            return Ok(None);
        };
        let view = state
            .ads
            .get_ad(slot, ads_enabled.unwrap_or(true), locale.as_deref())?;
        Ok(view.map(AdDto::from))
    })
    .await
}

/// Opens a sponsor page in the default browser. The WebView sends only the
/// slot and the creative id; the URL comes from the validated feed.
#[tauri::command]
pub async fn ad_clicked(
    state: State<'_, AppState>,
    slot: String,
    creative_id: String,
) -> CommandResult<()> {
    read_blocking(state, move |state| {
        let slot = AdSlot::parse(&slot).ok_or_else(unknown_creative)?;
        Ok(state
            .ads
            .open_click(slot, &creative_id, state.ad_opener.as_ref())?)
    })
    .await
}

/// The slot really rendered: counts the impression once per creative.
#[tauri::command]
pub async fn ad_shown(
    state: State<'_, AppState>,
    slot: String,
    creative_id: String,
) -> CommandResult<()> {
    read_blocking(state, move |state| {
        let slot = AdSlot::parse(&slot).ok_or_else(unknown_creative)?;
        Ok(state.ads.report_shown(slot, &creative_id)?)
    })
    .await
}

/// The community sign-in state. An unreachable server is `offline` (never an
/// error) and a rejected session is `signed_out`. The token never crosses IPC.
#[tauri::command]
pub async fn community_me(state: State<'_, AppState>) -> CommandResult<CommunityStatusDto> {
    read_blocking(state, |state| Ok(state.community.status()?.into())).await
}

/// Signs in with Discord through the system browser (up to 2 minutes). Runs on
/// the blocking pool and never under `write_lock`.
#[tauri::command]
pub async fn community_sign_in(state: State<'_, AppState>) -> CommandResult<CommunityStatusDto> {
    read_blocking(state, |state| Ok(state.community.sign_in()?.into())).await
}

/// Revokes the session (best effort when offline) and forgets it locally.
#[tauri::command]
pub async fn community_sign_out(state: State<'_, AppState>) -> CommandResult<()> {
    read_blocking(state, |state| Ok(state.community.sign_out()?)).await
}

/// Registers every command on the builder; shared by the real app and the
/// contract tests.
pub fn configure<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        get_config,
        detect_games,
        set_game_dir,
        get_game,
        list_mods,
        import_mods,
        remove_mod,
        list_profiles,
        create_profile,
        rename_profile,
        delete_profile,
        set_profile_mods,
        list_conflicts,
        set_active_profile,
        play,
        current_session,
        stop_session,
        recovery_status,
        deploy_session,
        forget_stale_session,
        clean_stale_session,
        restore_game_files,
        get_storage_usage,
        open_data_folder,
        crate::commands_diagnostics::get_diagnostics,
        crate::commands_diagnostics::open_logs_folder,
        get_game_artwork,
        get_catalog,
        refresh_catalog,
        open_catalog_page,
        open_support_page,
        get_ad,
        ad_clicked,
        ad_shown,
        community_me,
        community_sign_in,
        community_sign_out,
        crate::commands_stars::community_mod_stats,
        crate::commands_stars::community_star,
        crate::commands_stars::community_unstar,
        crate::commands_comments::community_comments_list,
        crate::commands_comments::community_comment_post,
        crate::commands_comments::community_comment_edit,
        crate::commands_comments::community_comment_delete,
        crate::commands_comments::community_report_comment,
        crate::commands_downloads::report_mod_download,
        crate::commands_downloads::get_mod_downloads,
        crate::commands_downloads::get_mod_download_daily,
        crate::commands_install::install_catalog_entry,
        crate::commands_install::cancel_catalog_install,
        crate::commands_publish::community_publish_mod,
        crate::commands_publish::community_cancel_publish,
        crate::commands_publish::community_my_mods
    ])
}

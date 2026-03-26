use crate::libs::audio::{AudioContext, load_soundpack_file};
use crate::libs::soundpack::cache::{SoundpackMetadata, SoundpackRef, SoundpackType};
use crate::state::app::use_app_state;
use crate::utils::config::use_config;
use dioxus::prelude::*;
use futures_timer::Delay;
use lucide_dioxus::{Check, ChevronDown, Keyboard, Mouse, Music, Search};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Props, Clone, PartialEq)]
pub struct SoundpackSelectorProps {
    pub soundpack_type: SoundpackType,
    pub icon: Element,
    pub label: String,
}

#[derive(Clone, PartialEq)]
pub enum DropDownItem {
    ClearSelection,
    Soundpack(SoundpackMetadata),
}

#[component]
pub fn SoundpackSelector(props: SoundpackSelectorProps) -> Element {
    rsx! {
        div { class: "space-y-2",
            div { class: "flex items-center gap-2 text-sm font-bold text-base-content/80",
                span { class: "text-primary", {props.icon} }
                "{props.label}"
            }
            SoundpackDropdown { soundpack_type: props.soundpack_type }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct SoundpackItemProps {
    pub item: SoundpackMetadata,
    pub is_selected: bool,
    pub select_soundpack: Callback<SoundpackRef, ()>,
}

#[component]
fn SoundpackItem(props: SoundpackItemProps) -> Element {
    let item = props.item;
    let select_soundpack = props.select_soundpack;
    let is_selected = props.is_selected;

    rsx! {
        button {
            key: "{item.id}",
            class: "w-full px-4 rounded-none py-2 text-left btn btn-lg justify-start gap-4 border-b border-base-300 last:border-b-0 h-auto btn-ghost",
            disabled: false,
            onclick: move |_| select_soundpack(item.id.clone()),
            div { class: "flex items-center justify-between gap-3 ",
                div { class: "flex-shrink-0 w-8 h-8 rounded-box flex items-center justify-center bg-base-100 overflow-hidden relative",
                    if let Some(icon) = &item.icon {
                        if !icon.is_empty() {
                            img {
                                class: "w-full h-full object-cover bg-blend-multiply",
                                src: "{icon}",
                            }
                        } else {
                            Music { class: "w-4 h-4 text-primary/50 bg-base-100" }
                        }
                    } else {
                        Music { class: "w-4 h-4 text-primary/50 bg-base-100" }
                    }
                    if is_selected {
                        div { class: "absolute inset-0 bg-base-300/70 flex items-center justify-center ",
                            Check { class: "text-white w-6 h-6" }
                        }
                    }
                }
                div { class: "flex-1 min-w-0",
                    div { class: "text-xs font-medium line-clamp-1 text-base-content",
                        "{item.name}"
                    }
                    div { class: "text-xs font-normal line-clamp-1 text-base-content/50",
                        if let Some(author) = &item.author {
                            "by {author}"
                        } else {
                            "by N/A"
                        }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct ClearButtonProps {
    pub clear_soundpack: Callback<(), ()>,
}

#[component]
fn ClearButton(props: ClearButtonProps) -> Element {
    let clear_soundpack = props.clear_soundpack;

    rsx! {
        button {
            class: "w-full px-4 rounded-none py-2 text-left btn btn-lg justify-start gap-4 border-b border-base-300 last:border-b-0 h-auto btn-ghost",
            disabled: false,
            onclick: move |_| clear_soundpack(()),
            div { class: "flex items-center justify-between gap-3 ",
                div { class: "flex-shrink-0 w-8 h-8 rounded-box flex items-center justify-center bg-base-100 overflow-hidden relative",
                    Music { class: "w-4 h-4 text-primary/50 bg-base-100" }
                }
                div { class: "flex-1 min-w-0",
                    div { class: "text-xs font-medium line-clamp-1 text-base-content",
                        "- None -"
                    }
                    div { class: "text-xs font-normal line-clamp-1 text-base-content/50" }
                }
            }
        }
    }
}

#[component]
fn SoundpackDropdown(soundpack_type: SoundpackType) -> Element {
    // Use audio context from the layout provider
    let audio_ctx: Arc<Mutex<AudioContext>> = use_context();

    // Use the new event-driven app state
    let app_state = use_app_state();
    let (config, update_config) = use_config();

    // UI state
    let mut error = use_signal(String::new);
    let mut is_open = use_signal(|| false);
    let mut search_query = use_signal(String::new);
    let mut is_loading = use_signal(|| false);

    // Use global app state for soundpacks
    let soundpacks = use_memo(move || app_state.get_soundpacks());

    // Check if there are any soundpacks available for this type
    let has_soundpacks = use_memo(move || {
        soundpacks()
            .into_iter()
            .any(|pack| pack.id.soundpack_type == soundpack_type)
    });

    // Get current soundpack based on type
    let current = use_memo(move || {
        let config = config();
        SoundpackRef::parse(match soundpack_type {
            SoundpackType::Keyboard => &config.keyboard_soundpack,
            SoundpackType::Mouse => &config.mouse_soundpack,
        })
        .ok()
    });

    // Filter soundpacks based on search query and type, then sort by last_modified
    let dropdown_items = use_memo(move || {
        let query = search_query().to_lowercase();
        let all_packs = soundpacks(); // Filter by type first
        let type_filtered_packs: Vec<_> = all_packs
            .into_iter()
            .filter(|pack| pack.id.soundpack_type == soundpack_type)
            .collect();

        // Then filter by search query
        let mut filtered_packs = if query.is_empty() {
            type_filtered_packs
        } else {
            type_filtered_packs
                .into_iter()
                .filter(|pack| {
                    pack.name.to_lowercase().contains(&query)
                        || pack.id.id.to_lowercase().contains(&query)
                        || pack
                            .tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&query))
                })
                .collect()
        };

        // Sort by name alphabetically
        filtered_packs.sort_by(|a, b| a.name.cmp(&b.name));

        let mut items: Vec<DropDownItem> = filtered_packs
            .into_iter()
            .map(|pack| DropDownItem::Soundpack(pack))
            .collect();

        items.insert(0, DropDownItem::ClearSelection);

        items
    });

    let current_soundpack = use_memo(move || {
        soundpacks()
            .into_iter()
            .find(|pack| current().is_some_and(|id| id == pack.id))
    });

    // Get appropriate placeholder and search text based on type
    let (placeholder_text, search_placeholder, not_found_text, no_soundpack_text) =
        match soundpack_type {
            SoundpackType::Keyboard => (
                "Select a keyboard sound pack...",
                "Search keyboard sound packs...",
                "No keyboard sound packs found",
                "No sound packs available",
            ),
            SoundpackType::Mouse => (
                "Select a mouse sound pack...",
                "Search mouse sound packs...",
                "No mouse sound packs found",
                "No sound packs available",
            ),
        };

    let clear_soundpack = {
        let audio_ctx = audio_ctx.clone();
        let update_config = update_config.clone();
        use_callback(move |_| {
            update_config(Box::new(move |config| {
                match soundpack_type {
                    SoundpackType::Keyboard => {
                        config.keyboard_soundpack = "".to_string();
                    }
                    SoundpackType::Mouse => {
                        config.mouse_soundpack = "".to_string();
                    }
                };
            }));
            {
                let mut audio_ctx = audio_ctx.lock().unwrap();
                match soundpack_type {
                    SoundpackType::Keyboard => {
                        audio_ctx.clear_keyboard_mappings();
                    }
                    SoundpackType::Mouse => {
                        audio_ctx.clear_mouse_mappings();
                    }
                }
            }
            is_open.set(false);
            search_query.set(String::new());
            error.set(String::new());
        })
    };

    let select_soundpack = {
        let audio_ctx = audio_ctx.clone();
        let update_config = update_config.clone();

        use_callback(move |soundpack_id: SoundpackRef| {
            let soundpack_exists = soundpacks().iter().any(|p| p.id == soundpack_id);

            is_open.set(false);
            search_query.set(String::new());
            error.set(String::new());

            if !soundpack_exists {
                return;
            }

            if current().is_some_and(|id| id == soundpack_id) {
                return;
            }

            {
                let soundpack_id = soundpack_id.clone();
                update_config(Box::new(move |config| {
                    match soundpack_id.soundpack_type {
                        SoundpackType::Keyboard => {
                            config.keyboard_soundpack = soundpack_id.to_string();
                        }
                        SoundpackType::Mouse => {
                            config.mouse_soundpack = soundpack_id.to_string();
                        }
                    };
                }));
            }
            {
                let soundpack_id = soundpack_id.clone();
                let audio_ctx = audio_ctx.clone();
                spawn(async move {
                    is_loading.set(true);
                    Delay::new(Duration::from_millis(1)).await;
                    let mut audio_ctx = audio_ctx.lock().unwrap();
                    let result = load_soundpack_file(&mut audio_ctx, &soundpack_id);
                    match result {
                        Ok(_) => {
                            log::info!("✅ Loaded {} soundpack", soundpack_id.to_string());
                        }
                        Err(e) => {
                            log::error!(
                                "❌ Failed to load {} soundpack: {}",
                                soundpack_id.to_string(),
                                e
                            );
                            error.set(format!(
                                "Failed to load soundpack {}",
                                soundpack_id.to_string(),
                            ));
                        }
                    }
                    is_loading.set(false);
                });
            }
        })
    };

    rsx! {
        div { class: "space-y-2",
            div { class: "relative w-full",
                // Dropdown toggle button
                button {
                    id: format!("soundpack-btn-{:?}", soundpack_type),
                    class: format!(
                        "w-full btn btn-soft justify-start gap-3 h-17 rounded-box {}",
                        if is_open() { "btn-active" } else { "" },
                    ),
                    style: format!("anchor-name: --soundpack-anchor-{:?};", soundpack_type),
                    disabled: is_loading() || !has_soundpacks(),
                    onclick: move |_| {
                        if has_soundpacks() {
                            is_open.set(!is_open());
                        }
                    },
                    div { class: "flex items-center gap-3 flex-1 ",
                        if !has_soundpacks() {
                            div { class: "text-base-content/50 text-sm", "{no_soundpack_text}" }
                        } else if let Some(pack) = current_soundpack() {
                            div { class: "flex-shrink-0 overflow-hidden  w-11 h-11 bg-base-200 rounded-box flex items-center justify-center",
                                if is_loading() {
                                    span { class: "loading loading-spinner loading-sm" }
                                } else {
                                    if let Some(icon) = &pack.icon {
                                        if !icon.is_empty() {
                                            img {
                                                class: "w-full h-full bg-blend-multiply object-cover",
                                                src: "{icon}",
                                            }
                                        } else {
                                            Music { class: "w-5 h-5 text-base-content/50" }
                                        }
                                    } else {
                                        Music { class: "w-5 h-5 text-base-content/50" }
                                    }
                                }
                            }
                            div { class: "flex-1 min-w-0 text-left",
                                div { class: "font-medium line-clamp-1 text-base-content  text-sm",
                                    "{pack.name}"
                                }
                                div { class: "text-xs font-normal truncate text-base-content/50",
                                    if let Some(author) = &pack.author {
                                        "by {author}"
                                    } else {
                                        "by N/A"
                                    }
                                }
                            }
                        } else {
                            div { class: "text-base-content/50 text-sm", "{placeholder_text}" }
                        }
                    }
                    ChevronDown {
                        class: format!(
                            "w-4 h-4 transition-transform {}",
                            if is_open() { "rotate-180" } else { "" },
                        ),
                    }
                }
                // Dropdown panel
                if is_open() && has_soundpacks() {
                    div {
                        class: "bg-base-200 border border-base-300 rounded-box shadow-lg z-50 ",
                        style: format!(
                            "position: absolute; position-anchor: --soundpack-anchor-{:?}; position-area: block-end; width: anchor-size(width); margin-top: 4px;",
                            soundpack_type,
                        ),

                        // Search input
                        div { class: "p-3 border-b border-base-200",
                            div { class: "relative",
                                Search { class: "absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-primary/50" }
                                input {
                                    class: "input input-sm w-full px-4 py-2 text-base-content placeholder:text-base-content/40",
                                    placeholder: "{search_placeholder}",
                                    value: "{search_query}",
                                    oninput: move |evt| search_query.set(evt.value()),
                                    autofocus: true,
                                }
                            }
                        }

                        // Soundpack list
                        div { class: "overflow-y-auto max-h-50",
                            if dropdown_items.read().is_empty() {
                                div { class: "p-4 text-center text-base-content/50",
                                    "{not_found_text}"
                                }
                            } else {
                                for item in dropdown_items.read().iter() {
                                    match item {
                                        DropDownItem::Soundpack(pack) => rsx! {
                                            SoundpackItem {
                                                key: "{pack.id}",
                                                item: pack.clone(),
                                                is_selected: current().is_some_and(|id| id == pack.id),
                                                select_soundpack: select_soundpack.clone(),
                                            }
                                        },
                                        DropDownItem::ClearSelection => rsx! {
                                            ClearButton { key: "{\"clear_selection_key\"}", clear_soundpack: clear_soundpack }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Click outside to close
            if is_open() && has_soundpacks() {
                div {
                    class: "fixed inset-0 z-40",
                    onclick: move |_| {
                        is_open.set(false);
                        search_query.set(String::new());
                    },
                }
            }

            // Error display
            if !error().is_empty() {
                div { class: "text-xs text-error mt-1", "{error}" }
            }
        }
    }
}

// Wrapper components for keyboard and mouse soundpack selectors

#[component]
pub fn KeyboardSoundpackSelector() -> Element {
    rsx! {
        SoundpackSelector {
            soundpack_type: SoundpackType::Keyboard,
            label: "Keyboard".to_string(),
            icon: rsx! {
                Keyboard { class: "w-4 h-4" }
            },
        }
    }
}

#[component]
pub fn MouseSoundpackSelector() -> Element {
    rsx! {
        SoundpackSelector {
            soundpack_type: SoundpackType::Mouse,
            label: "Mouse".to_string(),
            icon: rsx! {
                Mouse { class: "w-4 h-4" }
            },
        }
    }
}

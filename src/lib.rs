pub mod data;
pub mod storage;

use data::{fetch_available_lists, load_list, ListInfo, LoadedList};
use storage::{load_state as load_storage_state, save_state as persist_state};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(PartialEq, Clone)]
enum FetchStatus {
    Idle,
    Loading,
    Error(String),
}

#[function_component(App)]
fn app() -> Html {
    let list_status = use_state(|| FetchStatus::Loading);
    let lists = use_state(|| None::<Vec<ListInfo>>);
    let persisted_state = use_state(load_storage_state);

    let initial_selection = (*persisted_state).selected_list.clone();
    let selected_list = use_state(move || initial_selection);

    let items_status = use_state(|| FetchStatus::Idle);
    let loaded_list = use_state(|| None::<LoadedList>);

    {
        let list_status = list_status.clone();
        let lists = lists.clone();
        let selected_list = selected_list.clone();
        let persisted_state = persisted_state.clone();

        use_effect_with_deps(
            move |_| {
                list_status.set(FetchStatus::Loading);

                let list_status = list_status.clone();
                let lists = lists.clone();
                let selected_list = selected_list.clone();
                let previously_selected = (*selected_list).clone();
                let persisted_state = persisted_state.clone();

                spawn_local(async move {
                    match fetch_available_lists().await {
                        Ok(fetched) => {
                            let previous = previously_selected
                                .or_else(|| persisted_state.selected_list.clone());
                            let default_selection =
                                resolve_selection(&fetched, previous);
                            lists.set(Some(fetched));
                            if let Some(selection) = default_selection {
                                selected_list.set(Some(selection));
                            }
                            list_status.set(FetchStatus::Idle);
                        }
                        Err(err) => {
                            list_status.set(FetchStatus::Error(err.to_string()));
                            lists.set(None);
                            selected_list.set(None);
                        }
                    }
                });

                || ()
            },
            (),
        );
    }

    {
        let selected_list = selected_list.clone();
        let items_status = items_status.clone();
        let loaded_list = loaded_list.clone();

        use_effect_with_deps(
            move |selected: &Option<String>| {
                match selected {
                    Some(id) => {
                        items_status.set(FetchStatus::Loading);
                        loaded_list.set(None);

                        let id = id.clone();
                        let items_status = items_status.clone();
                        let loaded_list = loaded_list.clone();

                        spawn_local(async move {
                            match load_list(&id).await {
                                Ok(list) => {
                                    loaded_list.set(Some(list));
                                    items_status.set(FetchStatus::Idle);
                                }
                                Err(err) => {
                                    items_status.set(FetchStatus::Error(err.to_string()));
                                    loaded_list.set(None);
                                }
                            }
                        });
                    }
                    None => {
                        loaded_list.set(None);
                        items_status.set(FetchStatus::Idle);
                    }
                };

                || ()
            },
            (*selected_list).clone(),
        );
    }

    {
        let selected_list = selected_list.clone();
        let persisted_state = persisted_state.clone();

        use_effect_with_deps(
            move |current: &Option<String>| {
                let mut next_state = (*persisted_state).clone();
                if next_state.selected_list != *current {
                    next_state.selected_list = current.clone();
                    persist_state(&next_state);
                    persisted_state.set(next_state);
                }
                || ()
            },
            (*selected_list).clone(),
        );
    }

    html! {
        <div class="app-container">
            <header class="top-bar">
                <h1>{ "Ranking Lists" }</h1>
            </header>
            <main class="content">
                <section class="lists-panel">
                    { render_lists_panel(&list_status, &lists, &selected_list) }
                </section>
                <section class="list-details">
                    { render_list_details(&items_status, &loaded_list) }
                </section>
            </main>
        </div>
    }
}

fn render_lists_panel(
    status: &UseStateHandle<FetchStatus>,
    lists: &UseStateHandle<Option<Vec<ListInfo>>>,
    selected_list: &UseStateHandle<Option<String>>,
) -> Html {
    match &**status {
        FetchStatus::Loading => html! { <p>{ "Loading lists…" }</p> },
        FetchStatus::Error(message) => html! { <p class="error">{ message }</p> },
        FetchStatus::Idle => {
            let Some(list_vec) = &**lists else {
                return html! { <p>{ "No lists available." }</p> };
            };

            if list_vec.is_empty() {
                return html! { <p>{ "No lists available." }</p> };
            }

            let current_selection = (*selected_list).clone();

            html! {
                <div class="list-selector">
                    <h2>{ "Available Lists" }</h2>
                    <div class="list-buttons">
                        { for list_vec.iter().map(|info| render_list_button(info, &current_selection, selected_list)) }
                    </div>
                </div>
            }
        }
    }
}

fn render_list_button(
    info: &ListInfo,
    current_selection: &Option<String>,
    selected_list: &UseStateHandle<Option<String>>,
) -> Html {
    let id = info.id.clone();
    let label = info.label.clone();
    let is_active = current_selection
        .as_ref()
        .map(|selected| selected == &info.id)
        .unwrap_or(false);

    let class = if is_active {
        "list-button active"
    } else {
        "list-button"
    };

    let on_click = {
        let selected_list = selected_list.clone();
        Callback::from(move |_| {
            selected_list.set(Some(id.clone()));
        })
    };

    html! {
        <button class={class} onclick={on_click}>{ label }</button>
    }
}

fn render_list_details(
    status: &UseStateHandle<FetchStatus>,
    loaded: &UseStateHandle<Option<LoadedList>>,
) -> Html {
    match &**status {
        FetchStatus::Loading => html! { <p>{ "Loading list…" }</p> },
        FetchStatus::Error(message) => html! { <p class="error">{ message }</p> },
        FetchStatus::Idle => {
            let Some(list) = &**loaded else {
                return html! { <p>{ "Select a list to begin." }</p> };
            };

            html! {
                <div class="list-preview">
                    <h2>{ format!("Items in {}", list.info.label) }</h2>
                    <ul>
                        { for list.items.iter().map(|item| html!{ <li key={item.id.clone()}>{ &item.label }</li> }) }
                    </ul>
                </div>
            }
        }
    }
}

fn resolve_selection(
    lists: &[ListInfo],
    previous: Option<String>,
) -> Option<String> {
    match previous {
        Some(current) => {
            if lists.iter().any(|info| info.id == current) {
                Some(current)
            } else {
                lists.first().map(|info| info.id.clone())
            }
        }
        None => lists.first().map(|info| info.id.clone()),
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}

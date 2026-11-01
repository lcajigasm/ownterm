//! OSS Settings form that writes `~/.ownterm/provider.env`.
//!
//! The key field is the name of an environment variable. The file never stores the key.

use ownterm_provider::{ProviderForm, model_choices};
use warpui::elements::{ChildView, ConstrainedBox, Container, Flex, ParentElement, Text};
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::appearance::Appearance;
use crate::editor::{EditorView, SingleLineEditorOptions, TextOptions};
use crate::view_components::action_button::{ActionButton, ButtonSize, PrimaryTheme};
use crate::view_components::{Dropdown, DropdownItem};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OwnTermProviderAction {
    SelectProvider(String),
    SelectModel(String),
    SelectCustomModel,
    Save,
}

enum FormNotice {
    Saved(String),
    Error(String),
}

pub(crate) struct OwnTermProviderFormView {
    provider_dropdown: ViewHandle<Dropdown<OwnTermProviderAction>>,
    model_dropdown: ViewHandle<Dropdown<OwnTermProviderAction>>,
    model_editor: ViewHandle<EditorView>,
    base_url_editor: ViewHandle<EditorView>,
    api_key_env_editor: ViewHandle<EditorView>,
    save_button: ViewHandle<ActionButton>,
    provider: String,
    model: String,
    custom_model: bool,
    notice: Option<FormNotice>,
}

impl OwnTermProviderFormView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let (form, notice) = loaded_form();
        let choices = model_choices(&form.provider);
        let custom_model = !choices.iter().any(|id| *id == form.model);
        let displayed_base_url = if form.base_url.is_empty() {
            ownterm_provider::find_preset(&form.provider)
                .and_then(|preset| preset.default_base_url)
                .unwrap_or("")
                .to_owned()
        } else {
            form.base_url.clone()
        };

        let provider = form.provider.clone();
        let provider_dropdown = ctx.add_typed_action_view({
            let provider = provider.clone();
            move |ctx| {
                let mut dropdown = Dropdown::new(ctx);
                dropdown.set_top_bar_max_width(280.);
                let mut selected = 0;
                let items: Vec<_> = ownterm_provider::presets()
                    .iter()
                    .enumerate()
                    .map(|(index, preset)| {
                        if preset.id == provider {
                            selected = index;
                        }
                        DropdownItem::new(
                            preset.display_name,
                            OwnTermProviderAction::SelectProvider(preset.id.to_owned()),
                        )
                    })
                    .collect();
                dropdown.set_items(items, ctx);
                dropdown.set_selected_by_index(selected, ctx);
                dropdown
            }
        });

        let model = form.model.clone();
        let model_dropdown = ctx.add_typed_action_view({
            let provider = provider.clone();
            let model = model.clone();
            move |ctx| {
                let mut dropdown = Dropdown::new(ctx);
                dropdown.set_top_bar_max_width(280.);
                let (items, selected) = model_dropdown_items(&provider, &model);
                dropdown.set_items(items, ctx);
                dropdown.set_selected_by_index(selected, ctx);
                dropdown
            }
        });

        let model_editor = single_line_editor("Model id", &form.model, ctx);
        let base_url_editor =
            single_line_editor("https://example.com/v1", &displayed_base_url, ctx);
        let api_key_env_editor =
            single_line_editor(ownterm_provider::ENV_API_KEY, &form.api_key_env, ctx);
        let save_button = ctx.add_typed_action_view(|_| {
            ActionButton::new("Save provider", PrimaryTheme)
                .with_size(ButtonSize::Small)
                .on_click(|ctx| {
                    ctx.dispatch_typed_action(OwnTermProviderAction::Save);
                })
        });

        Self {
            provider_dropdown,
            model_dropdown,
            model_editor,
            base_url_editor,
            api_key_env_editor,
            save_button,
            provider,
            model,
            custom_model,
            notice,
        }
    }

    fn retarget_base_url(&self, next_provider: &str, ctx: &mut ViewContext<Self>) {
        let previous = ownterm_provider::find_preset(&self.provider)
            .and_then(|preset| preset.default_base_url)
            .unwrap_or("");
        let current = self.base_url_editor.as_ref(ctx).buffer_text(ctx);
        if current.trim().is_empty() || current.trim() == previous {
            let next = ownterm_provider::find_preset(next_provider)
                .and_then(|preset| preset.default_base_url)
                .unwrap_or("");
            self.base_url_editor.update(ctx, |editor, ctx| {
                editor.set_buffer_text(next, ctx);
            });
        }
    }

    fn sync_model_dropdown(&mut self, ctx: &mut ViewContext<Self>) {
        let lookup = if self.custom_model {
            ""
        } else {
            self.model.as_str()
        };
        let (items, selected) = model_dropdown_items(&self.provider, lookup);
        self.custom_model = selected + 1 == items.len();
        self.model_dropdown.update(ctx, |dropdown, ctx| {
            dropdown.set_items(items, ctx);
            dropdown.set_selected_by_index(selected, ctx);
        });
    }
}

impl Entity for OwnTermProviderFormView {
    type Event = ();
}

impl View for OwnTermProviderFormView {
    fn ui_name() -> &'static str {
        "OwnTermProviderFormView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let mut column = Flex::column();
        column.add_child(field_label(appearance, "Provider"));
        column.add_child(ChildView::new(&self.provider_dropdown).finish());
        column.add_child(field_label(appearance, "Model"));
        column.add_child(ChildView::new(&self.model_dropdown).finish());
        if self.custom_model {
            column.add_child(text_field(appearance, &self.model_editor));
        }
        column.add_child(field_label(appearance, "Base URL"));
        column.add_child(text_field(appearance, &self.base_url_editor));
        column.add_child(field_label(appearance, "API key environment variable"));
        column.add_child(field_label(
            appearance,
            "The file stores this name, never the key.",
        ));
        column.add_child(text_field(appearance, &self.api_key_env_editor));
        column.add_child(
            Container::new(ChildView::new(&self.save_button).finish())
                .with_margin_top(12.)
                .finish(),
        );
        if let Some(notice) = &self.notice {
            let (text, error) = match notice {
                FormNotice::Saved(text) => (text.as_str(), false),
                FormNotice::Error(text) => (text.as_str(), true),
            };
            column.add_child(notice_line(appearance, text, error));
        }
        column.finish()
    }
}

impl TypedActionView for OwnTermProviderFormView {
    type Action = OwnTermProviderAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            OwnTermProviderAction::SelectProvider(provider) => {
                self.retarget_base_url(provider, ctx);
                self.provider = provider.clone();
                let choices = model_choices(&self.provider);
                if !self.custom_model && !choices.iter().any(|id| *id == self.model) {
                    match choices.first() {
                        Some(first) => self.model = (*first).to_owned(),
                        None => {
                            self.model.clear();
                            self.custom_model = true;
                        }
                    }
                }
                self.sync_model_dropdown(ctx);
                self.notice = None;
            }
            OwnTermProviderAction::SelectModel(model) => {
                self.model = model.clone();
                self.custom_model = false;
                self.notice = None;
            }
            OwnTermProviderAction::SelectCustomModel => {
                self.custom_model = true;
                self.notice = None;
            }
            OwnTermProviderAction::Save => self.save(ctx),
        }
        ctx.notify();
    }
}

impl OwnTermProviderFormView {
    fn save(&mut self, ctx: &mut ViewContext<Self>) {
        let model = if self.custom_model {
            self.model_editor.as_ref(ctx).buffer_text(ctx)
        } else {
            self.model.clone()
        };
        let form = ProviderForm {
            provider: self.provider.clone(),
            model,
            base_url: self.base_url_editor.as_ref(ctx).buffer_text(ctx),
            api_key_env: self.api_key_env_editor.as_ref(ctx).buffer_text(ctx),
        };
        let path = match ownterm_provider::default_provider_file_path() {
            Ok(path) => path,
            Err(err) => {
                self.notice = Some(FormNotice::Error(err.to_string()));
                return;
            }
        };
        match ownterm_provider::save_provider_form(&path, &form) {
            Ok(()) => {
                self.model = form.model.trim().to_owned();
                let env_name = form.api_key_env.trim();
                self.notice = Some(FormNotice::Saved(format!(
                    "Saved {}. The API key stays in {env_name}.",
                    path.display()
                )));
            }
            Err(message) => self.notice = Some(FormNotice::Error(message)),
        }
    }
}

fn loaded_form() -> (ProviderForm, Option<FormNotice>) {
    let Ok(path) = ownterm_provider::default_provider_file_path() else {
        return (ProviderForm::default(), None);
    };
    match ownterm_provider::load_provider_form(&path) {
        Ok(Some(form)) => (form, None),
        Ok(None) => (ProviderForm::default(), None),
        Err(message) => (ProviderForm::default(), Some(FormNotice::Error(message))),
    }
}

fn model_dropdown_items(
    provider: &str,
    model: &str,
) -> (Vec<DropdownItem<OwnTermProviderAction>>, usize) {
    let models = model_choices(provider);
    let mut items: Vec<_> = models
        .iter()
        .map(|id| DropdownItem::new(*id, OwnTermProviderAction::SelectModel((*id).to_owned())))
        .collect();
    items.push(DropdownItem::new(
        "Custom",
        OwnTermProviderAction::SelectCustomModel,
    ));
    let selected = models
        .iter()
        .position(|id| *id == model)
        .unwrap_or(items.len() - 1);
    (items, selected)
}

fn single_line_editor(
    placeholder: &str,
    text: &str,
    ctx: &mut ViewContext<OwnTermProviderFormView>,
) -> ViewHandle<EditorView> {
    let placeholder = placeholder.to_owned();
    let text = text.to_owned();
    ctx.add_typed_action_view(move |ctx| {
        let appearance = Appearance::handle(ctx);
        let options = SingleLineEditorOptions {
            text: TextOptions::ui_font_size(appearance.as_ref(ctx)),
            ..Default::default()
        };
        let mut editor = EditorView::single_line(options, ctx);
        editor.set_placeholder_text(&placeholder, ctx);
        if !text.is_empty() {
            editor.set_buffer_text(&text, ctx);
        }
        editor
    })
}

fn field_label(appearance: &Appearance, label: &'static str) -> Box<dyn Element> {
    Container::new(
        Text::new_inline(label, appearance.ui_font_family(), 13.)
            .with_color(
                appearance
                    .theme()
                    .sub_text_color(appearance.theme().surface_2())
                    .into_solid(),
            )
            .finish(),
    )
    .with_margin_top(8.)
    .with_margin_bottom(4.)
    .finish()
}

fn text_field(appearance: &Appearance, editor: &ViewHandle<EditorView>) -> Box<dyn Element> {
    let theme = appearance.theme();
    appearance
        .ui_builder()
        .text_input(editor.clone())
        .with_style(UiComponentStyles {
            height: Some(30.),
            padding: Some(Coords::uniform(7.)),
            margin: Some(Coords::default().bottom(4.)),
            font_size: Some(appearance.ui_font_size()),
            background: Some(theme.surface_2().into()),
            ..Default::default()
        })
        .build()
        .finish()
}

fn notice_line(appearance: &Appearance, text: &str, error: bool) -> Box<dyn Element> {
    let color = if error {
        appearance.theme().ui_warning_color()
    } else {
        appearance
            .theme()
            .sub_text_color(appearance.theme().surface_2())
            .into_solid()
    };
    ConstrainedBox::new(
        Container::new(
            Text::new_inline(text.to_owned(), appearance.ui_font_family(), 13.)
                .with_color(color)
                .finish(),
        )
        .with_margin_top(8.)
        .finish(),
    )
    .with_max_width(420.)
    .finish()
}

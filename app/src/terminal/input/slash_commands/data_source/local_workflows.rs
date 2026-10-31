use ordered_float::OrderedFloat;
use warp_core::ui::appearance::Appearance;
use warpui::{AppContext, Entity, SingletonEntity};

use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::DataSourceRunErrorWrapper;
use crate::search::{FuzzyMatchWorkflowResult, SyncDataSource};
use crate::terminal::input::slash_commands::{AcceptSlashCommandOrSavedPrompt, InlineItem};
use crate::user_config::WarpConfig;
use crate::workflows::workflow::Workflow;

const ZERO_STATE_LIMIT: usize = 20;
const MIN_MATCH_SCORE: f64 = 25.0;

pub struct LocalWorkflowSlashSource;

impl Entity for LocalWorkflowSlashSource {
    type Event = ();
}

impl LocalWorkflowSlashSource {
    pub fn menu_items(query_text: &str, app: &AppContext) -> Vec<InlineItem> {
        let font_family = Appearance::as_ref(app).ui_font_family();
        let query_text = query_text.trim();
        let mut items = Vec::new();
        for workflow in WarpConfig::as_ref(app).local_user_workflows() {
            if query_text.is_empty() {
                if items.len() == ZERO_STATE_LIMIT {
                    break;
                }
                items.push(item(workflow, font_family, None, None, OrderedFloat(100.0)));
                continue;
            }
            let Some(matched) = FuzzyMatchWorkflowResult::try_match(query_text, workflow, "")
            else {
                continue;
            };
            let score = matched.score();
            if score <= OrderedFloat(MIN_MATCH_SCORE) {
                continue;
            }
            items.push(item(
                workflow,
                font_family,
                matched.name_match_result,
                matched.content_match_result,
                score,
            ));
        }
        items
    }
}

fn item(
    workflow: &Workflow,
    font_family: warpui::fonts::FamilyId,
    name_match_result: Option<fuzzy_match::FuzzyMatchResult>,
    description_match_result: Option<fuzzy_match::FuzzyMatchResult>,
    score: OrderedFloat<f64>,
) -> InlineItem {
    InlineItem {
        action: AcceptSlashCommandOrSavedPrompt::LocalWorkflow {
            workflow: Box::new(workflow.clone()),
        },
        icon_path: Some("bundled/svg/prompt.svg"),
        name: workflow.name().to_owned(),
        description: workflow.description().cloned(),
        font_family,
        name_match_result,
        description_match_result,
        score,
        compact_layout: false,
    }
}

impl SyncDataSource for LocalWorkflowSlashSource {
    type Action = AcceptSlashCommandOrSavedPrompt;

    fn run_query(
        &self,
        query: &Query,
        app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        if query.text.trim().is_empty() {
            return Ok(vec![]);
        }
        Ok(Self::menu_items(&query.text, app)
            .into_iter()
            .map(Into::into)
            .collect())
    }
}

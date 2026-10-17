//! Draft refresh operations shared by mouse editing and the control API.
use crate::ui::states::state::{
    NewFormationRuleDialog, RemoveFormationRuleDialog, WordFormationDialog,
};
use tdector_app::Session;

pub(crate) fn refresh_word_formation(session: &Session, dialog: &mut WordFormationDialog) {
    dialog.related_words = session.related_words(
        &dialog.base_word,
        crate::consts::domain::DEFAULT_RELATED_WORDS_COUNT,
    );
    dialog.preview = if dialog.base_word.is_empty() {
        String::new()
    } else {
        dialog
            .selected_rule
            .and_then(|i| session.project().formation_rules.get(i))
            .map(|rule| {
                Session::preview_rule(rule, &dialog.base_word)
                    .unwrap_or_else(|error| format!("Error: {error}"))
            })
            .unwrap_or_default()
    };
}

pub(crate) fn refresh_new_formation(dialog: &mut NewFormationRuleDialog) {
    dialog.preview = if dialog.command.is_empty() || dialog.test_word.is_empty() {
        String::new()
    } else {
        let rule = tdector_eval::FormationRule {
            description: dialog.description.clone(),
            rule_type: dialog.rule_type,
            command: dialog.command.clone(),
            cached_ast: tdector_eval::default_cached_ast(),
        };
        Session::preview_rule(&rule, &dialog.test_word)
            .unwrap_or_else(|error| format!("Error: {error}"))
    };
}

/// Refresh the removal confirmation from its live token rather than its opening snapshot.
pub(crate) fn refresh_remove_formation(
    session: &Session,
    dialog: &mut RemoveFormationRuleDialog,
) -> bool {
    let Some(token) = session
        .project()
        .segments
        .get(dialog.sentence_idx)
        .and_then(|segment| segment.tokens.get(dialog.word_idx))
    else {
        dialog.formatted_word.clear();
        dialog.base_word.clear();
        dialog.rule_description.clear();
        return false;
    };
    dialog.formatted_word.clone_from(&token.original);
    dialog.base_word = token
        .base_word
        .as_deref()
        .unwrap_or(&token.original)
        .to_owned();
    let rule = token
        .formation_rule_indices
        .last()
        .and_then(|index| session.project().formation_rules.get(*index));
    dialog.rule_description = rule
        .map(|rule| rule.description.clone())
        .unwrap_or_default();
    rule.is_some()
}

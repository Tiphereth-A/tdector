//! GUI host adapter: translates user intents and platform I/O into application calls.
use std::future::Future;
use std::sync::{Arc, Mutex};

use eframe::egui;
use tdector_app::{Command, SaveToken};
use tdector_eval::{AppError, AppResult, TokenizationRule};

use crate::enums::{AppAction, PinnedPopup};
use crate::platform::{self, FileIO, FileType};
use crate::ui::states::state::DecryptionApp;

/// Deliver platform results and wake the UI, including when the browser is otherwise idle.
fn run_file_task<T: 'static>(
    ctx: &egui::Context,
    pending: Arc<Mutex<Option<AppResult<T>>>>,
    future: impl Future<Output = AppResult<T>> + 'static,
) {
    let ctx = ctx.clone();
    FileIO::spawn(async move {
        let result = future.await;
        if let Ok(mut guard) = pending.lock() {
            *guard = Some(result);
        }
        ctx.request_repaint();
    });
}

impl DecryptionApp {
    pub(crate) fn apply_command(&mut self, command: Command, ctx: &egui::Context) -> bool {
        let text_revision = self.session.text_revision();
        match self.session.execute(command) {
            Ok(changed) => {
                if changed {
                    self.filter_dirty = true;
                    self.update_title(ctx);
                    if text_revision != self.session.text_revision() {
                        self.refresh_similarity_views();
                    }
                    ctx.request_repaint();
                }
                true
            }
            Err(error) => {
                self.error_message = Some(error.to_string());
                ctx.request_repaint();
                false
            }
        }
    }

    fn refresh_similarity_views(&mut self) {
        use crate::consts::domain::DEFAULT_SIMILARITY_RESULTS;
        if let Some((target, _)) = self.similar_popup.as_ref() {
            self.compute_similar_segments(*target);
        }
        if let Some((word, _)) = self.similar_tokens_popup.as_ref() {
            let word = word.clone();
            self.similar_tokens_popup = Some((word.clone(), self.session.similar_tokens(&word)));
        }
        for popup in &mut self.pinned_popups {
            if let PinnedPopup::Similar(target, scores, _, _) = popup {
                match self
                    .session
                    .similar_segments(*target, DEFAULT_SIMILARITY_RESULTS)
                {
                    Ok(updated) => *scores = updated,
                    Err(error) => {
                        scores.clear();
                        self.error_message = Some(error.to_string());
                    }
                }
            }
        }
    }

    pub(crate) fn import_text(
        &mut self,
        content: &str,
        name: &str,
        rule: &TokenizationRule,
        ctx: &egui::Context,
    ) -> bool {
        match self.session.import_text(content, name, rule) {
            Ok(()) => {
                self.project_filename = None;
                self.reset_project_view(ctx);
                true
            }
            Err(error) => {
                self.error_message = Some(error.to_string());
                ctx.request_repaint();
                false
            }
        }
    }

    fn reset_project_view(&mut self, ctx: &egui::Context) {
        self.custom_font_name = None;
        self.pending_import = None;
        self.confirmation = None;
        self.current_page = 0;
        self.filter_text.clear();
        self.cached_filtered_indices.clear();
        self.filter_dirty = true;
        self.clear_popups();
        self.update_title(ctx);
    }

    pub(crate) fn load_text_file(&mut self, ctx: &egui::Context) {
        let revision = self.session.revision();
        run_file_task(ctx, self.pending_text_file.clone(), async move {
            let kind = FileType::Text;
            let (bytes, name, _) = FileIO::pick_file(kind.filter_name(), kind.extensions()).await?;
            let text = String::from_utf8(bytes)
                .map_err(|error| AppError::IoError(format!("Failed to decode file: {error}")))?;
            Ok((revision, text, name))
        });
    }

    pub(crate) fn load_project(&mut self, ctx: &egui::Context) {
        let revision = self.session.revision();
        run_file_task(ctx, self.pending_project_file.clone(), async move {
            let kind = FileType::Json;
            let (bytes, name, path) =
                FileIO::pick_file(kind.filter_name(), kind.extensions()).await?;
            let text = String::from_utf8(bytes)
                .map_err(|error| AppError::IoError(format!("Failed to decode file: {error}")))?;
            Ok((revision, text, name, path))
        });
    }

    pub(crate) fn save_project(&mut self, ctx: &egui::Context) {
        let snapshot = match self.session.save_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.error_message = Some(error.to_string());
                return;
            }
        };
        let name = if self.session.project().project_name.is_empty() {
            "project.json".to_owned()
        } else {
            format!("{}.json", self.session.project().project_name)
        };
        let destination = self.project_filename.clone();
        run_file_task(ctx, self.pending_save_result.clone(), async move {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(ref filename) = destination {
                FileIO::save_file_to_path(&snapshot.bytes, std::path::Path::new(filename)).await?;
                return Ok((snapshot.token, Some(filename.clone())));
            }
            let filename = destination.as_deref().unwrap_or(&name);
            let path =
                FileIO::save_file_with_path(&snapshot.bytes, filename, "JSON", &["json"]).await?;
            Ok((snapshot.token, path))
        });
    }

    pub(crate) fn load_font_file(&mut self, ctx: &egui::Context) {
        run_file_task(ctx, self.pending_font_file.clone(), async {
            let kind = FileType::Font;
            let (data, name, _) = FileIO::pick_file(kind.filter_name(), kind.extensions()).await?;
            Ok((data, name))
        });
    }

    pub(crate) fn load_custom_font_from_bytes(
        &mut self,
        ctx: &egui::Context,
        data: Vec<u8>,
        font_name: &str,
    ) {
        platform::register_custom_font(ctx, data);
        self.custom_font_name = Some(font_name.to_owned());
    }

    pub fn initialize_fonts(ctx: &egui::Context) {
        platform::initialize_fonts(ctx);
    }

    pub(crate) fn export_typst(&mut self, ctx: &egui::Context) {
        let content = self.session.export_typst();
        let name = &self.session.project().project_name;
        let filename = format!("{}.typ", if name.is_empty() { "export" } else { name });
        run_file_task(ctx, self.pending_export_result.clone(), async move {
            let kind = FileType::Typst;
            FileIO::save_file(
                content.as_bytes(),
                &filename,
                kind.filter_name(),
                kind.extensions(),
            )
            .await
        });
    }

    pub(crate) fn update_title(&self, ctx: &egui::Context) {
        let dirty = self.session.is_dirty();
        self.unsaved_changes.set(dirty);
        let dirty_mark = if dirty { "*" } else { "" };
        let name = &self.session.project().project_name;
        let title = if name.is_empty() {
            format!("Text Decryption Helper{dirty_mark}")
        } else {
            format!("Text Decryption Helper - {name}{dirty_mark}")
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    pub(crate) fn trigger_action(&mut self, action: AppAction, ctx: &egui::Context) {
        if self.session.is_dirty() && !matches!(action, AppAction::Export) {
            let message = if action == AppAction::Quit {
                "You have unsaved changes. Are you sure you want to quit?"
            } else {
                "You have unsaved changes. Continue anyway?"
            };
            self.confirmation = Some((message.to_owned(), action));
        } else {
            self.execute_action(action, ctx);
        }
    }

    pub(crate) fn execute_action(&mut self, action: AppAction, ctx: &egui::Context) {
        match action {
            AppAction::Import => self.load_text_file(ctx),
            AppAction::Open => self.load_project(ctx),
            AppAction::Export => self.export_typst(ctx),
            AppAction::Quit => {
                self.closing = true;
                self.unsaved_changes.set(false);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    pub(crate) fn find_related_words(&self, prefix: &str) -> Vec<String> {
        self.session
            .related_words(prefix, crate::consts::domain::DEFAULT_RELATED_WORDS_COUNT)
    }

    pub(crate) fn process_pending_file_operations(&mut self, ctx: &egui::Context) {
        let result = self
            .pending_text_file
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());
        if let Some(result) = result {
            match result {
                Ok((revision, _, _)) if revision != self.session.revision() => {
                    self.error_message = Some(
                        "The project changed while the text file was loading. Please try Import again."
                            .into(),
                    );
                }
                Ok((_, content, name)) => self.pending_import = Some((content, name)),
                Err(AppError::OperationCancelled) => {}
                Err(error) => self.error_message = Some(error.to_string()),
            }
        }

        let result = self
            .pending_project_file
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());
        if let Some(result) = result {
            match result {
                Ok((revision, _, _, _)) if revision != self.session.revision() => {
                    self.error_message = Some(
                        "The project changed while the project file was loading. Please try Open again."
                            .into(),
                    );
                }
                Ok((_, content, name, path)) => match self.session.load_json(&content) {
                    Ok(()) => {
                        self.project_filename = path.or(Some(name));
                        self.reset_project_view(ctx);
                    }
                    Err(error) => self.error_message = Some(error.to_string()),
                },
                Err(AppError::OperationCancelled) => {}
                Err(error) => self.error_message = Some(error.to_string()),
            }
        }

        let result = self
            .pending_save_result
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());
        if let Some(result) = result {
            match result {
                Ok((token, path)) => self.complete_save(token, path, ctx),
                Err(AppError::OperationCancelled) => {}
                Err(error) => self.error_message = Some(error.to_string()),
            }
        }

        let result = self
            .pending_export_result
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());
        if let Some(Err(error)) = result
            && !matches!(error, AppError::OperationCancelled)
        {
            self.error_message = Some(error.to_string());
        }

        let result = self
            .pending_font_file
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());
        if let Some(result) = result {
            match result {
                Ok((data, name)) => self.load_custom_font_from_bytes(ctx, data, &name),
                Err(AppError::OperationCancelled) => {}
                Err(error) => self.error_message = Some(error.to_string()),
            }
        }
    }

    fn complete_save(&mut self, token: SaveToken, path: Option<String>, ctx: &egui::Context) {
        if self.session.acknowledge_saved(token) {
            if let Some(path) = path {
                self.project_filename = Some(path);
            }
            self.update_title(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tdector_eval::FormationType;

    fn imported_app(content: &str, ctx: &egui::Context) -> DecryptionApp {
        let mut app = DecryptionApp::default();
        assert!(app.import_text(
            content,
            "Example",
            &TokenizationRule::default_whitespace(),
            ctx,
        ));
        app
    }

    #[test]
    fn dirty_mirror_tracks_commands_and_only_current_save_completions() {
        let ctx = egui::Context::default();
        let mut app = imported_app("cat", &ctx);
        let original = app.session.save_snapshot().expect("snapshot");
        app.complete_save(original.token, Some("original.json".into()), &ctx);
        assert!(!app.session.is_dirty());
        assert!(!app.unsaved_changes.get());

        assert!(app.apply_command(
            Command::SetTranslation {
                segment: 0,
                translation: "a cat".into()
            },
            &ctx,
        ));
        assert!(app.session.is_dirty());
        assert!(app.unsaved_changes.get());
        app.complete_save(original.token, Some("stale.json".into()), &ctx);
        assert!(app.session.is_dirty());
        assert!(app.unsaved_changes.get());
        assert_eq!(app.project_filename.as_deref(), Some("original.json"));

        let current = app.session.save_snapshot().expect("current snapshot");
        app.complete_save(current.token, Some("current.json".into()), &ctx);
        assert!(!app.session.is_dirty());
        assert!(!app.unsaved_changes.get());
        assert_eq!(app.project_filename.as_deref(), Some("current.json"));

        let revision = app.session.revision();
        assert!(app.apply_command(
            Command::SetTranslation {
                segment: 0,
                translation: "a cat".into()
            },
            &ctx,
        ));
        assert!(!app.apply_command(
            Command::SetTranslation {
                segment: 99,
                translation: "invalid".into()
            },
            &ctx,
        ));
        assert_eq!(app.session.revision(), revision);
        assert!(!app.session.is_dirty());
        assert!(!app.unsaved_changes.get());
        assert!(app.error_message.is_some());
    }

    #[test]
    fn stale_save_cannot_change_an_imported_or_loaded_projects_path() {
        let ctx = egui::Context::default();
        let mut app = imported_app("cat", &ctx);
        let old = app.session.save_snapshot().expect("old snapshot");
        assert!(app.import_text(
            "dog",
            "Replacement",
            &TokenizationRule::default_whitespace(),
            &ctx,
        ));
        app.complete_save(old.token, Some("old.json".into()), &ctx);
        assert!(app.project_filename.is_none());
        assert!(app.unsaved_changes.get());
        assert_eq!(app.session.project().project_name, "Replacement");

        let imported = app.session.save_snapshot().expect("replacement snapshot");
        let json = String::from_utf8(old.bytes).expect("JSON");
        app.pending_import = Some(("stale text".into(), "Old import".into()));
        *app.pending_project_file.lock().expect("project result") = Some(Ok((
            app.session.revision(),
            json,
            "loaded.json".into(),
            Some("loaded.json".into()),
        )));
        *app.pending_save_result.lock().expect("save result") =
            Some(Ok((imported.token, Some("replacement.json".into()))));
        app.process_pending_file_operations(&ctx);
        assert_eq!(app.project_filename.as_deref(), Some("loaded.json"));
        assert!(app.pending_import.is_none());
        assert_eq!(app.session.project().project_name, "Example");
        assert!(!app.session.is_dirty());
        assert!(!app.unsaved_changes.get());
    }

    #[test]
    fn independent_gui_instances_keep_separate_dirty_mirrors() {
        let ctx = egui::Context::default();
        let mut first = imported_app("cat", &ctx);
        let mut second = DecryptionApp::default();
        assert!(!std::rc::Rc::ptr_eq(
            &first.unsaved_changes,
            &second.unsaved_changes
        ));
        assert!(first.unsaved_changes.get());
        assert!(!second.unsaved_changes.get());
        assert!(second.import_text(
            "dog",
            "Second",
            &TokenizationRule::default_whitespace(),
            &ctx,
        ));
        let snapshot = first.session.save_snapshot().expect("first snapshot");
        first.complete_save(snapshot.token, None, &ctx);
        assert!(!first.unsaved_changes.get());
        assert!(second.unsaved_changes.get());
        assert!(second.session.is_dirty());
    }

    #[test]
    fn only_surface_changes_refresh_pinned_and_unpinned_similarity_results() {
        let ctx = egui::Context::default();
        let mut app = imported_app("cats fish\ncats bird\ncat fish", &ctx);
        for command in [
            Command::SetGloss {
                word: "cat".into(),
                meaning: "animal".into(),
            },
            Command::CreateFormationRule {
                description: "Plural".into(),
                rule_type: FormationType::Inflection,
                command: r#"fn transform(word) { word + "s" }"#.into(),
            },
            Command::ApplyFormationRule {
                word: "cats".into(),
                base_word: "cat".into(),
                rule: 0,
            },
        ] {
            assert!(app.apply_command(command, &ctx));
        }
        // A recognizable snapshot lets this test observe whether a query was rerun.
        let displayed_scores = vec![(2, -1.0)];
        app.similar_popup = Some((0, displayed_scores.clone()));
        app.pinned_popups.push(PinnedPopup::Similar(
            1,
            displayed_scores.clone(),
            7,
            "Pinned similarity".into(),
        ));
        let text_revision = app.session.text_revision();
        assert!(app.apply_command(
            Command::SetTranslation {
                segment: 0,
                translation: "cats and fish".into()
            },
            &ctx,
        ));
        assert_eq!(app.session.text_revision(), text_revision);
        assert_eq!(
            app.similar_popup.as_ref().expect("open popup").1,
            displayed_scores
        );
        let PinnedPopup::Similar(_, pinned_scores, _, _) = &app.pinned_popups[0] else {
            panic!("expected pinned similarity");
        };
        assert_eq!(pinned_scores, &displayed_scores);

        assert!(app.apply_command(
            Command::RemoveFormationRule {
                segment: 0,
                token: 0
            },
            &ctx
        ));
        assert!(app.session.text_revision() > text_revision);
        assert_eq!(app.session.project().segments[0].tokens[0].original, "cat");
        let unpinned_expected = app
            .session
            .similar_segments(0, crate::consts::domain::DEFAULT_SIMILARITY_RESULTS)
            .expect("unpinned query");
        let pinned_expected = app
            .session
            .similar_segments(1, crate::consts::domain::DEFAULT_SIMILARITY_RESULTS)
            .expect("pinned query");
        assert!(
            unpinned_expected
                .iter()
                .any(|(index, score)| *index == 2 && *score > 0.99)
        );
        assert_eq!(
            app.similar_popup.as_ref().expect("open popup").1,
            unpinned_expected
        );
        let PinnedPopup::Similar(target, scores, id, title) = &app.pinned_popups[0] else {
            panic!("expected pinned similarity");
        };
        assert_eq!(*target, 1);
        assert_eq!(scores, &pinned_expected);
        assert_eq!(*id, 7);
        assert_eq!(title, "Pinned similarity");
        assert!(app.error_message.is_none());
    }

    #[test]
    fn delayed_project_load_preserves_newer_edits_and_replacements_until_retried() {
        let ctx = egui::Context::default();
        for replace_project in [false, true] {
            let mut app = imported_app("cat", &ctx);
            let requested_revision = app.session.revision();
            let incoming = String::from_utf8(app.session.save_snapshot().expect("snapshot").bytes)
                .expect("JSON");
            if replace_project {
                assert!(app.import_text(
                    "dog",
                    "Replacement",
                    &TokenizationRule::default_whitespace(),
                    &ctx,
                ));
            } else {
                assert!(app.apply_command(
                    Command::SetTranslation {
                        segment: 0,
                        translation: "newer edit".into(),
                    },
                    &ctx
                ));
            }
            app.project_filename = Some("current.json".into());
            let before = app.session.save_snapshot().expect("current state").bytes;
            let current_revision = app.session.revision();
            *app.pending_project_file.lock().expect("project result") = Some(Ok((
                requested_revision,
                incoming.clone(),
                "incoming.json".into(),
                Some("incoming.json".into()),
            )));
            app.process_pending_file_operations(&ctx);
            assert_eq!(
                app.session.save_snapshot().expect("preserved state").bytes,
                before
            );
            assert_eq!(app.session.revision(), current_revision);
            assert_eq!(app.project_filename.as_deref(), Some("current.json"));
            assert!(app.session.is_dirty());
            assert!(app.unsaved_changes.get());
            assert!(
                app.error_message
                    .as_deref()
                    .is_some_and(|error| error.contains("try Open again"))
            );

            // A new request against the current revision is allowed to finish normally.
            app.error_message = None;
            *app.pending_project_file
                .lock()
                .expect("retried project result") = Some(Ok((
                current_revision,
                incoming,
                "incoming.json".into(),
                Some("incoming.json".into()),
            )));
            app.process_pending_file_operations(&ctx);
            assert_eq!(app.session.project().segments[0].tokens[0].original, "cat");
            assert_eq!(app.project_filename.as_deref(), Some("incoming.json"));
            assert!(!app.session.is_dirty());
            assert!(!app.unsaved_changes.get());
            assert!(app.error_message.is_none());
        }
    }

    #[test]
    fn delayed_text_read_preserves_newer_edits_replacements_and_pending_import() {
        let ctx = egui::Context::default();
        for replace_project in [false, true] {
            let mut app = imported_app("cat", &ctx);
            let requested_revision = app.session.revision();
            if replace_project {
                assert!(app.import_text(
                    "dog",
                    "Replacement",
                    &TokenizationRule::default_whitespace(),
                    &ctx,
                ));
            } else {
                assert!(app.apply_command(
                    Command::SetTranslation {
                        segment: 0,
                        translation: "newer edit".into(),
                    },
                    &ctx
                ));
            }
            let current_import = ("current text".to_owned(), "current.txt".to_owned());
            app.pending_import = Some(current_import.clone());
            let before = app.session.save_snapshot().expect("current state").bytes;
            let current_revision = app.session.revision();
            *app.pending_text_file.lock().expect("text result") = Some(Ok((
                requested_revision,
                "stale text".into(),
                "stale.txt".into(),
            )));
            app.process_pending_file_operations(&ctx);
            assert_eq!(app.pending_import, Some(current_import));
            assert_eq!(
                app.session.save_snapshot().expect("preserved state").bytes,
                before
            );
            assert_eq!(app.session.revision(), current_revision);
            assert!(app.session.is_dirty());
            assert!(app.unsaved_changes.get());
            assert!(
                app.error_message
                    .as_deref()
                    .is_some_and(|error| error.contains("try Import again"))
            );

            app.error_message = None;
            *app.pending_text_file.lock().expect("retried text result") = Some(Ok((
                current_revision,
                "retried text".into(),
                "retry.txt".into(),
            )));
            app.process_pending_file_operations(&ctx);
            assert_eq!(
                app.pending_import,
                Some(("retried text".into(), "retry.txt".into()))
            );
            assert_eq!(
                app.session
                    .save_snapshot()
                    .expect("unchanged project")
                    .bytes,
                before
            );
            assert_eq!(app.session.revision(), current_revision);
            assert!(app.error_message.is_none());
        }
    }
}

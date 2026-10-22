//! The built-in commands.

use crate::command::{ArgSpec, Args, Command, Prefix, Preview};
use crate::key::{Key, KeyCode};
use crate::{Core, Effect};

/// Completes the first prompt over the projects' names and leaves the others free.
fn projects_then_free(core: &Core, args: &Args) -> Vec<String> {
    match args.strings.len() {
        0 => core.project_names(false),
        _ => Vec::new(),
    }
}

/// The file a file prompt's candidate names; a directory gives none.
fn preview_file(core: &Core, candidate: &str) -> Option<Preview> {
    let base = core.prompts.completion_base()?;
    (!candidate.ends_with('/')).then(|| Preview::File(base.join(candidate)))
}

/// The built-in commands, each with its spec.
pub(crate) fn builtins() -> Vec<Command> {
    vec![
        Command {
            name: "self-insert-command",
            spec: ArgSpec::default(),
            handler: |ctx, args| {
                let c = match ctx.core.this_keys.last().map(|key| key.code) {
                    Some(KeyCode::Char(c)) => c,
                    _ => return Err("self-insert-command must be run by a character key".into()),
                };
                ctx.insert_n(c.encode_utf8(&mut [0; 4]), args.count(), false)
            },
        },
        Command {
            name: "newline",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.insert_n("\n", args.count(), false),
        },
        // shortcut: no indentation yet, so the same as newline; the indentation issue indents here.
        Command {
            name: "electric-newline-and-maybe-indent",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.insert_n("\n", args.count(), false),
        },
        Command {
            name: "open-line",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.insert_n("\n", args.count(), true),
        },
        Command {
            name: "delete-char",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.delete_char(args.prefix.is_none(), args.count()),
        },
        Command {
            name: "delete-backward-char",
            spec: ArgSpec::default(),
            handler: |ctx, args| {
                ctx.delete_char(args.prefix.is_none(), args.count().saturating_neg())
            },
        },
        Command {
            name: "minibuffer-directory-delete-char",
            spec: ArgSpec::default(),
            handler: |ctx, args| match ctx.core.component_start() {
                Some(start) if args.prefix.is_none() && !ctx.core.window.active() => {
                    ctx.delete_to(start)
                }
                _ => ctx.delete_char(args.prefix.is_none(), args.count().saturating_neg()),
            },
        },
        Command {
            name: "kill-word",
            spec: ArgSpec {
                clipboard: true,
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.kill_to(ctx.word_end(args.count())),
        },
        Command {
            name: "backward-kill-word",
            spec: ArgSpec {
                clipboard: true,
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.kill_to(ctx.word_end(args.count().saturating_neg())),
        },
        Command {
            name: "kill-line",
            spec: ArgSpec {
                clipboard: true,
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.kill_line(args.prefix),
        },
        Command {
            name: "kill-region",
            spec: ArgSpec {
                region: true,
                clipboard: true,
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.kill_region(args.region, false),
        },
        Command {
            name: "kill-ring-save",
            spec: ArgSpec {
                region: true,
                clipboard: true,
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.kill_region(args.region, true),
        },
        Command {
            name: "yank",
            spec: ArgSpec {
                clipboard: true,
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.yank(args.prefix),
        },
        Command {
            name: "yank-pop",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.yank_pop(args.count()),
        },
        Command {
            name: "forward-char",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.forward_char(args.count()),
        },
        Command {
            name: "backward-char",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.forward_char(args.count().saturating_neg()),
        },
        Command {
            name: "next-line",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.next_line(args.count()),
        },
        Command {
            name: "previous-line",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.next_line(args.count().saturating_neg()),
        },
        Command {
            name: "forward-word",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.forward_word(args.count()),
        },
        Command {
            name: "backward-word",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.forward_word(args.count().saturating_neg()),
        },
        Command {
            name: "move-beginning-of-line",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.line_end(args.count(), false),
        },
        Command {
            name: "move-end-of-line",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.line_end(args.count(), true),
        },
        Command {
            name: "beginning-of-buffer",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.buffer_end(args.prefix, false),
        },
        Command {
            name: "end-of-buffer",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.buffer_end(args.prefix, true),
        },
        Command {
            name: "scroll-up-command",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.scroll_command(args.prefix, 1),
        },
        Command {
            name: "scroll-down-command",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.scroll_command(args.prefix, -1),
        },
        Command {
            name: "recenter-top-bottom",
            spec: ArgSpec::default(),
            handler: |ctx, args| {
                ctx.recenter(args.prefix);
                Ok(())
            },
        },
        Command {
            name: "undo",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                ctx.undo(false);
                Ok(())
            },
        },
        Command {
            name: "undo-redo",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                ctx.undo(true);
                Ok(())
            },
        },
        Command {
            name: "save-buffer",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.resolve_conflict("save-buffer", args),
        },
        Command {
            name: "save-buffers-kill-terminal",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.save_buffers_kill_terminal(args),
        },
        Command {
            name: "find-file",
            spec: ArgSpec {
                prompts: &["Find file: "],
                files: Some(0),
                preview: Some(preview_file),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.find_file(&args.strings[0]),
        },
        Command {
            name: "project-create",
            spec: ArgSpec {
                prompts: &["Project name: ", "Primary root: "],
                files: Some(1),
                directories: true,
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.create_project(&args.strings[0], &args.strings[1]),
        },
        Command {
            name: "project-save",
            spec: ArgSpec {
                prompts: &["Save project: "],
                completion: Some(|core, _| core.project_names(true)),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.save_project(&args.strings[0]),
        },
        Command {
            name: "project-add-root",
            spec: ArgSpec {
                prompts: &["Project: ", "Add root: ", "Role: "],
                completion: Some(projects_then_free),
                files: Some(1),
                directories: true,
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.add_root(&args),
        },
        Command {
            name: "project-remove-root",
            spec: ArgSpec {
                prompts: &["Project: ", "Remove root: "],
                completion: Some(|core, args| match args.strings.len() {
                    0 => core.project_names(false),
                    _ => core.root_paths(args, false),
                }),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.remove_root(&args),
        },
        Command {
            name: "project-set-role",
            spec: ArgSpec {
                prompts: &["Project: ", "Root: ", "Role: "],
                completion: Some(|core, args| match args.strings.len() {
                    0 => core.project_names(false),
                    1 => core.root_paths(args, true),
                    _ => Vec::new(),
                }),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.set_role(&args),
        },
        Command {
            name: "project-set-default-agent",
            spec: ArgSpec {
                prompts: &["Project: ", "Default agent: "],
                completion: Some(projects_then_free),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.set_default_agent(&args),
        },
        Command {
            name: "project-set-watch",
            spec: ArgSpec {
                prompts: &["Project: ", "Watch: "],
                completion: Some(|core, args| match args.strings.len() {
                    0 => core.project_names(false),
                    _ => vec!["true".into(), "false".into()],
                }),
                preselect: Some(|core, args| {
                    args.strings
                        .first()
                        .and_then(|name| core.project(name).ok())
                        .map_or(0, |i| usize::from(!core.projects[i].project.watch))
                }),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.set_watch(&args),
        },
        Command {
            name: "project-open-file",
            spec: ArgSpec {
                prompts: &["Project: "],
                completion: Some(projects_then_free),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.open_project_file(&args.strings[0]),
        },
        Command {
            name: "project-switch",
            spec: ArgSpec {
                prompts: &["Switch to project: "],
                completion: Some(projects_then_free),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.core.switch_project(&args.strings[0]),
        },
        Command {
            name: "project-index",
            spec: ArgSpec {
                prompts: &["Project: "],
                completion: Some(projects_then_free),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.index_project(&args.strings[0], false),
        },
        Command {
            name: "project-find-file",
            // Asks its own question, so it can start indexing before the prompt opens.
            spec: ArgSpec {
                preview: Some(|core, c| {
                    let project = &core.current_workspace().project;
                    core.project_file(project, c).ok().map(Preview::File)
                }),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.project_find_file(args),
        },
        Command {
            name: "project-grep",
            // Asks its own question, which names the project; the hits are matched already.
            spec: ArgSpec {
                filter: Some(|_, items| (0..items.len()).collect()),
                preview: Some(crate::grep::preview),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.project_grep(args),
        },
        Command {
            name: "workspace-new",
            spec: ArgSpec {
                prompts: &["New workspace for project: ", "Workspace name: "],
                completion: Some(projects_then_free),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.core.workspace_for(&args.strings[0], &args.strings[1]),
        },
        Command {
            name: "workspace-switch",
            spec: ArgSpec {
                prompts: &["Switch to workspace: "],
                completion: Some(|core, _| core.workspace_names(false)),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.core.switch_to_workspace(&args.strings[0]),
        },
        Command {
            name: "workspace-next",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.core.cycle_workspace(args.count()),
        },
        Command {
            name: "workspace-previous",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.core.cycle_workspace(args.count().saturating_neg()),
        },
        Command {
            name: "workspace-rename",
            spec: ArgSpec {
                prompts: &["Rename workspace to: "],
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.core.rename_workspace(&args.strings[0]),
        },
        Command {
            name: "workspace-kill",
            spec: ArgSpec {
                prompts: &["Kill workspace: "],
                completion: Some(|core, _| core.workspace_names(true)),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.kill_workspace(args),
        },
        Command {
            name: "workspace-kill-current",
            spec: ArgSpec::default(),
            handler: |ctx, args| {
                let strings = vec![ctx.core.current_workspace().name.clone()];
                ctx.call("workspace-kill", Args { strings, ..args });
                Ok(())
            },
        },
        Command {
            name: "workspace-list",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                ctx.core.list_workspaces();
                Ok(())
            },
        },
        Command {
            name: "reload-settings",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                ctx.effects.push(Effect::LoadSettings);
                Ok(())
            },
        },
        Command {
            name: "universal-argument",
            spec: ArgSpec::default(),
            handler: |ctx, args| {
                let prefix = match args.prefix {
                    None => Prefix::Universal(4),
                    Some(Prefix::Universal(n)) => Prefix::Universal(n.saturating_mul(4)),
                    Some(Prefix::Minus) => Prefix::Universal(-4),
                    // After digits C-u ends the number, so the next digit is not part of it.
                    number @ Some(Prefix::Number(_)) => {
                        ctx.core.prefix_arg = number;
                        return Ok(());
                    }
                };
                ctx.set_prefix(prefix);
                Ok(())
            },
        },
        Command {
            name: "digit-argument",
            spec: ArgSpec::default(),
            handler: |ctx, args| {
                let digit = match ctx.core.this_keys.last().map(|key| key.code) {
                    Some(KeyCode::Char(c)) => c.to_digit(10),
                    _ => None,
                }
                .ok_or("digit-argument must be run by a digit key")?;
                let digit = i64::from(digit);
                let prefix = match args.prefix {
                    Some(Prefix::Number(n)) => {
                        let digit = if n < 0 { -digit } else { digit };
                        Prefix::Number(n.saturating_mul(10).saturating_add(digit))
                    }
                    Some(Prefix::Minus) if digit == 0 => Prefix::Minus,
                    Some(Prefix::Minus) => Prefix::Number(-digit),
                    _ => Prefix::Number(digit),
                };
                ctx.set_prefix(prefix);
                Ok(())
            },
        },
        Command {
            name: "negative-argument",
            spec: ArgSpec::default(),
            handler: |ctx, args| {
                match args.prefix {
                    Some(Prefix::Number(n)) => ctx.set_prefix(Prefix::Number(n.saturating_neg())),
                    Some(Prefix::Minus) => {}
                    _ => ctx.set_prefix(Prefix::Minus),
                }
                Ok(())
            },
        },
        Command {
            name: "set-mark-command",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.set_mark_command(args.prefix),
        },
        Command {
            name: "exchange-point-and-mark",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.exchange_point_and_mark(),
        },
        Command {
            name: "split-window-below",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.split_window(false),
        },
        Command {
            name: "split-window-right",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.split_window(true),
        },
        Command {
            name: "other-window",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.other_window(args.count()),
        },
        Command {
            name: "delete-window",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.delete_window(),
        },
        Command {
            name: "delete-other-windows",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.delete_other_windows(),
        },
        Command {
            name: "xref-go-back",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.core.xref_go(false),
        },
        Command {
            name: "xref-go-forward",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.core.xref_go(true),
        },
        Command {
            name: "switch-to-buffer",
            spec: ArgSpec {
                prompts: &["Switch to buffer: "],
                completion: Some(|core, _| core.buffer_names(false)),
                preview: Some(|core, name| core.buffers.find_here(name).map(Preview::Buffer)),
                ..ArgSpec::default()
            },
            handler: |ctx, args| {
                ctx.core.switch_to_name(&args.strings[0]);
                Ok(())
            },
        },
        Command {
            name: "switch-to-buffer-other-window",
            spec: ArgSpec {
                prompts: &["Switch to buffer in other window: "],
                completion: Some(|core, _| core.buffer_names(false)),
                ..ArgSpec::default()
            },
            handler: |ctx, args| {
                if ctx.core.tree.leaves().len() == 1 {
                    ctx.split_window(false)?;
                }
                ctx.other_window(1)?;
                ctx.core.switch_to_name(&args.strings[0]);
                Ok(())
            },
        },
        Command {
            name: "kill-buffer",
            spec: ArgSpec {
                prompts: &["Kill buffer: "],
                completion: Some(|core, _| core.buffer_names(true)),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.kill_buffer(args),
        },
        Command {
            name: "revert-buffer",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.revert_buffer(args),
        },
        Command {
            name: "resolve-conflict",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.resolve_conflict("resolve-conflict", args),
        },
        Command {
            name: "diff-buffer-with-file",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.diff_buffer_with_file(),
        },
        Command {
            name: "recover-buffers",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.recover_buffers(args),
        },
        Command {
            name: "exit-minibuffer",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.exit_minibuffer(false),
        },
        Command {
            name: "minibuffer-exit-input",
            spec: ArgSpec::default(),
            handler: |ctx, _| ctx.exit_minibuffer(true),
        },
        Command {
            name: "abort-minibuffers",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                ctx.core.close_minibuffer().ok_or("Not in a minibuffer")?;
                ctx.core.echo("Quit".into());
                Ok(())
            },
        },
        Command {
            name: "minibuffer-next-completion",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.next_completion(args.count()),
        },
        Command {
            name: "minibuffer-previous-completion",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.next_completion(args.count().saturating_neg()),
        },
        Command {
            name: "minibuffer-complete",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                let text = ctx.core.prompts.selected_candidate().ok_or("No match")?;
                ctx.set_text(&text)
            },
        },
        Command {
            name: "swiper",
            spec: ArgSpec {
                prompts: &["Swiper: "],
                completion: Some(crate::swiper::candidates),
                filter: Some(crate::swiper::filter),
                preselect: Some(crate::swiper::preselect),
                preview: Some(crate::swiper::preview),
                ..ArgSpec::default()
            },
            handler: |ctx, args| ctx.swiper(&args.strings[0]),
        },
        Command {
            name: "swiper-C-s",
            spec: ArgSpec::default(),
            handler: |ctx, args| match ctx.core.text().is_empty() {
                true => ctx.history_element(1),
                false => ctx.next_completion(args.count()),
            },
        },
        Command {
            name: "previous-history-element",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.history_element(args.count()),
        },
        Command {
            name: "next-history-element",
            spec: ArgSpec::default(),
            handler: |ctx, args| ctx.history_element(args.count().saturating_neg()),
        },
        Command {
            name: "execute-extended-command",
            spec: ArgSpec {
                prompts: &["M-x "],
                completion: Some(|core, _| {
                    let mut names: Vec<String> =
                        core.commands.keys().map(|n| n.to_string()).collect();
                    names.sort();
                    names
                }),
                // shortcut: where_is walks every keymap per command, commands × bindings per M-x; invert into one walk when M-x opens noticeably slower.
                annotation: Some(|core, name| {
                    let keys = core.where_is(name)?;
                    Some(
                        keys.iter()
                            .map(Key::to_string)
                            .collect::<Vec<_>>()
                            .join(" "),
                    )
                }),
                ..ArgSpec::default()
            },
            handler: |ctx, args| {
                let name = (ctx.core.commands.get_key_value(args.strings[0].as_str()))
                    .map(|(name, _)| *name)
                    .ok_or_else(|| format!("No command named {}", args.strings[0]))?;
                ctx.core.this_command = Some(name);
                let args = Args {
                    prefix: args.prefix,
                    ..Args::default()
                };
                ctx.call(name, args);
                Ok(())
            },
        },
        Command {
            name: "keyboard-quit",
            spec: ArgSpec::default(),
            handler: |ctx, _| {
                ctx.core.window.set_active(false);
                ctx.core.echo("Quit".into());
                Ok(())
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use crate::command::Args;
    use crate::harness::Harness;

    #[test]
    fn builtins_are_callable_by_name() {
        let mut h = Harness::new(&[("a.txt", ""), ("settings.toml", "font_size = 18")]);
        h.open("a.txt");
        h.keys("ab");
        h.call("undo", Args::default());
        assert_eq!(h.text(), "");
        h.call("undo-redo", Args::default());
        assert_eq!(h.text(), "ab");
        h.call("save-buffer", Args::default());
        assert_eq!(h.disk("a.txt"), "ab\n");
        h.call("reload-settings", Args::default());
        assert_eq!(h.core.settings().font_size, 18.0);
    }
}

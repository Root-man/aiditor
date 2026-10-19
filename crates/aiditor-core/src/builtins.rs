//! The built-in commands.

use std::collections::HashMap;

use crate::Effect;
use crate::command::{self, ArgSpec, Args, Command, Prefix};
use crate::key::{Key, KeyCode};

pub(crate) fn builtins() -> HashMap<&'static str, Command> {
    let builtins: [(&'static str, command::Handler); 57] = [
        ("self-insert-command", |ctx, args| {
            let c = match ctx.core.this_keys.last().map(|key| key.code) {
                Some(KeyCode::Char(c)) => c,
                _ => return Err("self-insert-command must be run by a character key".into()),
            };
            ctx.insert_n(c.encode_utf8(&mut [0; 4]), args.count(), false)
        }),
        ("newline", |ctx, args| {
            ctx.insert_n("\n", args.count(), false)
        }),
        // shortcut: no indentation yet, so the same as newline; the indentation issue indents here.
        ("electric-newline-and-maybe-indent", |ctx, args| {
            ctx.insert_n("\n", args.count(), false)
        }),
        ("open-line", |ctx, args| {
            ctx.insert_n("\n", args.count(), true)
        }),
        ("delete-char", |ctx, args| {
            ctx.delete_char(args.prefix.is_none(), args.count())
        }),
        ("delete-backward-char", |ctx, args| {
            ctx.delete_char(args.prefix.is_none(), args.count().saturating_neg())
        }),
        ("minibuffer-directory-delete-char", |ctx, args| {
            match ctx.core.component_start() {
                Some(start) if args.prefix.is_none() && !ctx.core.window.active() => {
                    ctx.delete_to(start)
                }
                _ => ctx.delete_char(args.prefix.is_none(), args.count().saturating_neg()),
            }
        }),
        ("kill-word", |ctx, args| {
            ctx.kill_to(ctx.word_end(args.count()))
        }),
        ("backward-kill-word", |ctx, args| {
            ctx.kill_to(ctx.word_end(args.count().saturating_neg()))
        }),
        ("kill-line", |ctx, args| ctx.kill_line(args.prefix)),
        ("kill-region", |ctx, args| {
            ctx.kill_region(args.region, false)
        }),
        ("kill-ring-save", |ctx, args| {
            ctx.kill_region(args.region, true)
        }),
        ("yank", |ctx, args| ctx.yank(args.prefix)),
        ("yank-pop", |ctx, args| ctx.yank_pop(args.count())),
        ("forward-char", |ctx, args| ctx.forward_char(args.count())),
        ("backward-char", |ctx, args| {
            ctx.forward_char(args.count().saturating_neg())
        }),
        ("next-line", |ctx, args| ctx.next_line(args.count())),
        ("previous-line", |ctx, args| {
            ctx.next_line(args.count().saturating_neg())
        }),
        ("forward-word", |ctx, args| ctx.forward_word(args.count())),
        ("backward-word", |ctx, args| {
            ctx.forward_word(args.count().saturating_neg())
        }),
        ("move-beginning-of-line", |ctx, args| {
            ctx.line_end(args.count(), false)
        }),
        ("move-end-of-line", |ctx, args| {
            ctx.line_end(args.count(), true)
        }),
        ("beginning-of-buffer", |ctx, args| {
            ctx.buffer_end(args.prefix, false)
        }),
        ("end-of-buffer", |ctx, args| {
            ctx.buffer_end(args.prefix, true)
        }),
        ("scroll-up-command", |ctx, args| {
            ctx.scroll_command(args.prefix, 1)
        }),
        ("scroll-down-command", |ctx, args| {
            ctx.scroll_command(args.prefix, -1)
        }),
        ("recenter-top-bottom", |ctx, args| {
            ctx.recenter(args.prefix);
            Ok(())
        }),
        ("undo", |ctx, _| {
            ctx.undo(false);
            Ok(())
        }),
        ("undo-redo", |ctx, _| {
            ctx.undo(true);
            Ok(())
        }),
        ("save-buffer", |ctx, _| ctx.save(ctx.core.window.buffer())),
        ("save-buffers-kill-terminal", |ctx, args| {
            ctx.save_buffers_kill_terminal(args)
        }),
        ("find-file", |ctx, args| ctx.find_file(&args.strings[0])),
        ("reload-settings", |ctx, _| {
            ctx.effects.push(Effect::LoadSettings);
            Ok(())
        }),
        ("universal-argument", |ctx, args| {
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
        }),
        ("digit-argument", |ctx, args| {
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
        }),
        ("negative-argument", |ctx, args| {
            match args.prefix {
                Some(Prefix::Number(n)) => ctx.set_prefix(Prefix::Number(n.saturating_neg())),
                Some(Prefix::Minus) => {}
                _ => ctx.set_prefix(Prefix::Minus),
            }
            Ok(())
        }),
        ("set-mark-command", |ctx, args| {
            ctx.set_mark_command(args.prefix)
        }),
        ("exchange-point-and-mark", |ctx, _| {
            ctx.exchange_point_and_mark()
        }),
        ("split-window-below", |ctx, _| ctx.split_window(false)),
        ("split-window-right", |ctx, _| ctx.split_window(true)),
        ("other-window", |ctx, args| ctx.other_window(args.count())),
        ("delete-window", |ctx, _| ctx.delete_window()),
        ("delete-other-windows", |ctx, _| ctx.delete_other_windows()),
        ("switch-to-buffer", |ctx, args| {
            ctx.core.switch_to_name(&args.strings[0]);
            Ok(())
        }),
        ("switch-to-buffer-other-window", |ctx, args| {
            if ctx.core.tree.leaves().len() == 1 {
                ctx.split_window(false)?;
            }
            ctx.other_window(1)?;
            ctx.core.switch_to_name(&args.strings[0]);
            Ok(())
        }),
        ("kill-buffer", |ctx, args| ctx.kill_buffer(args)),
        ("recover-buffers", |ctx, args| ctx.recover_buffers(args)),
        ("exit-minibuffer", |ctx, _| ctx.exit_minibuffer(false)),
        ("minibuffer-exit-input", |ctx, _| ctx.exit_minibuffer(true)),
        ("abort-minibuffers", |ctx, _| {
            ctx.core.close_minibuffer().ok_or("Not in a minibuffer")?;
            ctx.core.echo("Quit".into());
            Ok(())
        }),
        ("minibuffer-next-completion", |ctx, args| {
            ctx.next_completion(args.count())
        }),
        ("minibuffer-previous-completion", |ctx, args| {
            ctx.next_completion(args.count().saturating_neg())
        }),
        ("minibuffer-complete", |ctx, _| {
            let text = ctx.core.prompts.selected_candidate().ok_or("No match")?;
            ctx.set_text(&text)
        }),
        ("previous-history-element", |ctx, args| {
            ctx.history_element(args.count())
        }),
        ("next-history-element", |ctx, args| {
            ctx.history_element(args.count().saturating_neg())
        }),
        ("execute-extended-command", |ctx, args| {
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
        }),
        ("keyboard-quit", |ctx, _| {
            ctx.core.window.set_active(false);
            ctx.core.echo("Quit".into());
            Ok(())
        }),
    ];
    let mut commands: HashMap<_, _> = builtins
        .map(|(name, handler)| {
            let spec = ArgSpec::default();
            (
                name,
                Command {
                    name,
                    spec,
                    handler,
                },
            )
        })
        .into();
    let kills = [
        "kill-word",
        "backward-kill-word",
        "kill-line",
        "kill-region",
    ];
    for name in kills.into_iter().chain(["kill-ring-save", "yank"]) {
        let spec: &mut ArgSpec = &mut commands.get_mut(name).expect("a builtin").spec;
        spec.clipboard = true;
        spec.region = matches!(name, "kill-region" | "kill-ring-save");
    }
    commands
        .get_mut("execute-extended-command")
        .expect("a builtin")
        .spec = ArgSpec {
        prompts: &["M-x "],
        completion: Some(|core, _| {
            let mut names: Vec<String> = core.commands.keys().map(|n| n.to_string()).collect();
            names.sort();
            names
        }),
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
    };
    for name in ["switch-to-buffer", "switch-to-buffer-other-window"] {
        commands.get_mut(name).expect("a builtin").spec = ArgSpec {
            prompts: if name == "switch-to-buffer" {
                &["Switch to buffer: "]
            } else {
                &["Switch to buffer in other window: "]
            },
            completion: Some(|core, _| core.buffer_names(false)),
            ..ArgSpec::default()
        };
    }
    commands.get_mut("find-file").expect("a builtin").spec = ArgSpec {
        prompts: &["Find file: "],
        files: true,
        ..ArgSpec::default()
    };
    commands
        .get_mut("save-buffers-kill-terminal")
        .expect("a builtin")
        .spec = ArgSpec {
        // Both prompts are opened by the handler; an odd count is mid-pair, a `Save file` one.
        completion: Some(|_, args| match args.strings.len() % 2 {
            1 => vec!["Yes".into(), "No".into()],
            _ => vec!["No".into(), "Yes".into()],
        }),
        ..ArgSpec::default()
    };
    commands.get_mut("recover-buffers").expect("a builtin").spec = ArgSpec {
        // The prompt is opened by the handler.
        completion: Some(|_, _| vec!["Yes".into(), "No".into()]),
        ..ArgSpec::default()
    };
    commands.get_mut("kill-buffer").expect("a builtin").spec = ArgSpec {
        prompts: &["Kill buffer: "],
        // The second prompt, opened by the handler, confirms killing a modified buffer.
        completion: Some(|core, args| match args.strings.is_empty() {
            true => core.buffer_names(true),
            false => vec!["no".into(), "yes".into()],
        }),
        ..ArgSpec::default()
    };
    commands
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

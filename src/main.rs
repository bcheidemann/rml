#![feature(io_error_more)]

use std::cmp::Ordering;
use std::env;
use std::fs;
use std::io;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process;

use cursive::event::Event;
use cursive::event::Key;
use cursive::theme::BaseColor;
use cursive::theme::Color;
use cursive::theme::ColorStyle;
use cursive::theme::ColorType;
use cursive::theme::Effect;
use cursive::theme::Style;
use cursive::utils::markup::StyledString;
use cursive::views::LinearLayout;

use walkdir::DirEntry;
use walkdir::WalkDir;

use cursive::traits::*;
use cursive::views::{Dialog, EditView, SelectView, TextView};

fn main() -> io::Result<()> {
    fn get_entries(dir: &PathBuf) -> Vec<DirEntry> {
        match get_dir_entries(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                eprintln!(
                    "Failed to read entries from current directory with error {}",
                    error
                );
                process::exit(1);
            }
        }
    }

    fn populate_select_view(select_view: &mut SelectView<DirEntry>, filter: &str) {
        select_view.clear();

        let cwd = env::current_dir().unwrap();
        let mut entries = get_entries(&cwd);

        entries.sort_by(|a, b| {
            if a.path().eq(&cwd) {
                return Ordering::Less;
            } else if b.path().eq(&cwd) {
                return Ordering::Greater;
            }

            let file_type_a = a.file_type();
            let file_type_b = b.file_type();

            if file_type_a.is_dir() {
                if file_type_b.is_file() {
                    return Ordering::Less;
                }
            } else if file_type_b.is_dir() {
                return Ordering::Greater;
            }

            a.file_name().cmp(b.file_name())
        });

        let entries = entries.into_iter();
        let entries = entries.filter(|entry| {
            if entry.path().eq(&cwd) {
                return filter.len() == 0 || filter == ".";
            }
            return entry.file_name().to_str().unwrap().starts_with(filter);
        });

        for entry in entries {
            if entry.path().eq(&cwd) {
                select_view.add_item(".", entry.clone());
            } else {
                let file_name = entry.file_name().to_str().unwrap();
                let display_name = if file_name.starts_with(".") {
                    if entry.file_type().is_dir() {
                        StyledString::styled(
                            file_name,
                            Style::merge(&[
                                Effect::Dim.into(),
                                Effect::Bold.into(),
                                ColorStyle::new(
                                    ColorType::Color(Color::Dark(BaseColor::Blue)),
                                    ColorType::default(),
                                )
                                .into(),
                            ]),
                        )
                    } else {
                        StyledString::styled(file_name, Effect::Dim)
                    }
                } else if entry.file_type().is_dir() {
                    StyledString::styled(
                        file_name,
                        Style::merge(&[
                            Effect::Bold.into(),
                            ColorStyle::new(
                                ColorType::Color(Color::Dark(BaseColor::Blue)),
                                ColorType::default(),
                            )
                            .into(),
                        ]),
                    )
                } else {
                    StyledString::styled(file_name, Effect::Italic)
                };
                select_view.add_item(display_name, entry.clone());
            }
        }
    }

    let mut select_view: SelectView<DirEntry> = SelectView::new();

    populate_select_view(&mut select_view, "");

    let select_view = select_view
        .autojump()
        .on_submit(|siv, entry| {
            let mut select_view = siv.find_name::<SelectView<DirEntry>>("select").unwrap();
            let mut edit_view = siv.find_name::<EditView>("edit").unwrap();

            edit_view.set_content("");

            let file_type = entry.file_type();

            if file_type.is_file() {
                let result = fs::remove_file(entry.path());
                if result.is_err() {
                    siv.add_layer(Dialog::info(format!(
                        "Failed to remove file {}: {}",
                        entry.path().display(),
                        result.err().unwrap()
                    )));
                }
            } else {
                let entry_to_remove = entry.clone();
                let result = fs::remove_dir(entry.path());
                match result {
                    Err(error) => match error.kind() {
                        ErrorKind::DirectoryNotEmpty => {
                            siv.add_layer(
                                Dialog::info(format!(
                                    "Failed to remove directory {}: Directory not empty.",
                                    entry_to_remove.path().display(),
                                ))
                                .button(
                                    "Delete Anyway",
                                    move |siv| {
                                        fs::remove_dir_all(entry_to_remove.path()).unwrap();
                                        siv.pop_layer();

                                        let mut select_view = siv
                                            .find_name::<SelectView<DirEntry>>("select")
                                            .unwrap();
                                        match select_view.selected_id() {
                                            None => {
                                                siv.add_layer(Dialog::info("No name to remove"))
                                            }
                                            Some(focused_id) => {
                                                populate_select_view(&mut select_view, "");
                                                select_view.set_selection(focused_id);
                                            }
                                        }
                                    },
                                ),
                            );
                        }
                        _ => {
                            siv.add_layer(Dialog::info(format!(
                                "Failed to remove directory {}: {}",
                                entry_to_remove.path().display(),
                                error,
                            )));
                        }
                    },
                    Ok(_) => {}
                }
            }

            match select_view.selected_id() {
                None => siv.add_layer(Dialog::info("No name to remove")),
                Some(focused_id) => {
                    populate_select_view(&mut select_view, "");
                    select_view.set_selection(focused_id);
                }
            }
        })
        .with_name("select")
        .full_screen()
        .scrollable();

    let edit_view = EditView::new()
        .on_edit(|siv, filter_text, _| {
            let mut select_view = siv.find_name::<SelectView<DirEntry>>("select").unwrap();
            populate_select_view(&mut select_view, filter_text);
        })
        .on_submit(|siv, _| {
            let mut layout_view = siv.find_name::<LinearLayout>("layout").unwrap();
            layout_view.set_focus_index(0).unwrap();
        })
        .with_name("edit");

    let layout = LinearLayout::vertical()
        .child(select_view)
        .child(edit_view)
        .child(TextView::new(
            "Tab = toggle search bar focus\t\tEnter = delete\t\tCtrl + C = exit",
        ))
        .with_name("layout");

    let mut siv = cursive::default();
    siv.set_on_pre_event(Event::Key(Key::Tab), |siv| {
        let mut layout_view = siv.find_name::<LinearLayout>("layout").unwrap();
        let current_focus = layout_view.get_focus_index();
        layout_view
            .set_focus_index(match current_focus {
                0 => 1,
                1 => 0,
                _ => 1,
            })
            .unwrap();
    });
    siv.add_fullscreen_layer(layout);
    siv.run();

    Ok(())
}

fn get_dir_entries(dir: &PathBuf) -> io::Result<Vec<DirEntry>> {
    let dir = WalkDir::new(dir).max_depth(1).min_depth(0);

    Ok(dir
        .into_iter()
        .map(|item| match item {
            Ok(item) => item,
            Err(error) => {
                eprintln!(
                    "Failed to read entries from current directory with error {}",
                    error
                );
                process::exit(1);
            }
        })
        .collect())
}

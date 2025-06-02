use anyhow::Error;
use mdbook::book::{Book, BookItem};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark;

use std::path::Path;

mod cmark_events;
mod code_args;
mod output;

use code_args::*;

// use serde::Deserialize;

// #[derive(Debug, Deserialize)]
// struct NadiConfig {
//     network: Option<String>,
//     ignore: String,
// }

// impl Default for NadiConfig {
//     fn default() -> Self {
//         Self {
//             network: None,
//             ignore: "!".into(),
//         }
//     }
// }

/// A NADI preprocessor.
pub struct Nadi;

impl Default for Nadi {
    fn default() -> Self {
        Self::new()
    }
}

impl Nadi {
    pub fn new() -> Nadi {
        Nadi
    }
}

impl Preprocessor for Nadi {
    fn name(&self) -> &str {
        "nadi-preprocessor"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let src_dir = ctx.root.clone().join(&ctx.config.book.src);
        book.for_each_mut(|item| {
            let _ = process_book_item(item, &src_dir).map_err(|err| {
                eprintln!("{}", err);
            });
        });
        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer != "not-supported"
    }
}

fn process_book_item(item: &mut BookItem, pwd: &Path) -> anyhow::Result<()> {
    if let BookItem::Chapter(ref mut chapter) = item {
        chapter.content = run_chapter(&chapter.content, pwd)?;
    }
    Ok(())
}

fn run_chapter(chap: &str, pwd: &Path) -> anyhow::Result<String> {
    enum State {
        None,
        Open,
        Gather,
    }

    let mut state = State::None;
    let mut buf = String::with_capacity(chap.len() * 2);
    let mut parser = mdbook::utils::new_cmark_parser(chap, false);
    let mut args = String::new();
    let mut handler: CodeHandler = run_task;

    let mut task_script = String::new();
    let events = parser.try_fold(vec![], |mut acc, ref e| -> anyhow::Result<Vec<Event<'_>>> {
        use CodeBlockKind::*;
        use CowStr::*;
        use Event::*;
        use State::*;
        match (e, &mut state) {
            (Start(Tag::CodeBlock(Fenced(Borrowed(mark)))), None) => {
                acc.push(Start(Tag::CodeBlock(Fenced(Borrowed(
                    mark.split(' ').next().unwrap_or_default().into(),
                )))));
                if let Some((h, a)) = nadi_code_args(mark) {
                    state = Open;
                    args = a;
                    handler = h;
                }
            }
            (Text(Borrowed(txt)), Open) => {
                acc.push(e.clone());
                task_script.clear();
                task_script.push_str(txt);
                state = Gather;
            }
            (Text(Borrowed(txt)), Gather) => {
                task_script.push_str(txt);
                acc.push(e.clone());
            }
            (End(TagEnd::CodeBlock), Gather) => {
                state = None;
                acc.push(e.clone());
                let response = handler(&task_script, &args, pwd)?;
                acc.extend(response);
            }
            _ => {
                acc.push(e.clone());
            }
        };
        Ok(acc)
    })?;
    Ok(cmark(events.iter(), &mut buf).map(|_| buf)?)
}

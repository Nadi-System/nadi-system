use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag};

// https://github.com/pulldown-cmark/pulldown-cmark/pull/967
// merged, remove this once the crate uses the updated version
pub fn event_to_static(event: Event<'_>) -> Event<'static> {
    match event {
        // ones with lifetime
        Event::Start(t) => Event::Start(tag_to_static(t)),
        Event::Text(t) => Event::Text(t.to_string().into()),
        Event::Code(t) => Event::Code(t.to_string().into()),
        Event::Html(t) => Event::Html(t.to_string().into()),
        Event::InlineHtml(t) => Event::InlineHtml(t.to_string().into()),
        Event::FootnoteReference(t) => Event::FootnoteReference(t.to_string().into()),
        // without lifetime; you can't do e => e, as enum has lifetime
        Event::End(t) => Event::End(t),
        Event::SoftBreak => Event::SoftBreak,
        Event::HardBreak => Event::HardBreak,
        Event::Rule => Event::Rule,
        Event::TaskListMarker(b) => Event::TaskListMarker(b),
    }
}

fn tag_to_static(tag: Tag<'_>) -> Tag<'static> {
    match tag {
        // with lifetime
        Tag::Heading {
            level,
            id,
            classes,
            attrs,
        } => Tag::Heading {
            level,
            id: id.map(|s| CowStr::from(s.to_string())),
            classes: classes
                .into_iter()
                .map(|s| CowStr::from(s.to_string()))
                .collect(),
            attrs: attrs
                .into_iter()
                .map(|(k, v)| {
                    (
                        CowStr::from(k.to_string()),
                        v.map(|s| CowStr::from(s.to_string())),
                    )
                })
                .collect(),
        },
        Tag::CodeBlock(kb) => Tag::CodeBlock(codeblockkind_to_static(kb)),
        Tag::FootnoteDefinition(a) => Tag::FootnoteDefinition(a.to_string().into()),
        Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        } => Tag::Link {
            link_type,
            dest_url: dest_url.to_string().into(),
            title: title.to_string().into(),
            id: id.to_string().into(),
        },
        Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        } => Tag::Image {
            link_type,
            dest_url: dest_url.to_string().into(),
            title: title.to_string().into(),
            id: id.to_string().into(),
        },
        // without lifetime
        Tag::Paragraph => Tag::Paragraph,
        Tag::BlockQuote => Tag::BlockQuote,
        Tag::HtmlBlock => Tag::HtmlBlock,
        Tag::List(v) => Tag::List(v),
        Tag::Item => Tag::Item,
        Tag::Table(v) => Tag::Table(v),
        Tag::TableHead => Tag::TableHead,
        Tag::TableRow => Tag::TableRow,
        Tag::TableCell => Tag::TableCell,
        Tag::Emphasis => Tag::Emphasis,
        Tag::Strong => Tag::Strong,
        Tag::Strikethrough => Tag::Strikethrough,
        Tag::MetadataBlock(v) => Tag::MetadataBlock(v),
    }
}

fn codeblockkind_to_static(cbkind: CodeBlockKind<'_>) -> CodeBlockKind<'static> {
    match cbkind {
        CodeBlockKind::Indented => CodeBlockKind::Indented,
        CodeBlockKind::Fenced(s) => CodeBlockKind::Fenced(s.to_string().into()),
    }
}

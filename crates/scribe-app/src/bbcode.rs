//! Safe BBCode/HTML renderer for ESOUI addon descriptions and changelogs.
//!
//! Produces a small block model from MMOUI description markup (BBCode with
//! occasional HTML) and renders it onto the Scribe Glass tokens with
//! `gpui::StyledText`/`gpui::InteractiveText`. The parser is iterative (no
//! recursion), never panics on adversarial input, caps input at 12,000 chars,
//! and degrades unmatched or malformed tags to literal text. Only http/https
//! links and images are honored; every other scheme renders as plain text.

use std::ops::Range;

use gpui::prelude::*;
use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, InteractiveText, ObjectFit, SharedString,
    StrikethroughStyle, StyledText, TextStyle, UnderlineStyle, div, img, px, relative,
};
use gpui_component::{StyledExt as _, scroll::ScrollableElement as _};

use crate::theme::*;

const INPUT_CAP_CHARS: usize = 12_000;
const MAX_BLOCKS: usize = 200;
const MAX_IMAGES: usize = 6;
const MAX_QUOTE_DEPTH: usize = 4;

// ---------------------------------------------------------------------------
// Block model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    color: Option<u32>,
    size: Option<f32>,
    mono: bool,
}

#[derive(Clone, Debug)]
struct InlineRun {
    text: String,
    style: InlineStyle,
    link: Option<String>,
}

#[derive(Debug)]
enum Block {
    Paragraph(Vec<InlineRun>),
    List {
        ordered: bool,
        items: Vec<Vec<InlineRun>>,
    },
    Quote {
        attribution: Option<String>,
        blocks: Vec<Block>,
    },
    Code(String),
    Image(String),
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum StyleTag {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Color(u32),
    Size(f32),
    Mono,
    /// Consumed but presentation-neutral tags: [font], [center], [left], [right].
    Neutral,
}

enum Container {
    Quote {
        attribution: Option<String>,
        blocks: Vec<Block>,
    },
    List {
        ordered: bool,
        items: Vec<Vec<InlineRun>>,
    },
}

#[derive(Default)]
struct Parser {
    root_blocks: Vec<Block>,
    stack: Vec<Container>,
    styles: Vec<StyleTag>,
    runs: Vec<InlineRun>,
    buffer: String,
    /// Open `[url]` tags: (byte length of paragraph text at open, explicit href).
    urls: Vec<(usize, Option<String>)>,
    images: usize,
}

impl Parser {
    fn text_len(&self) -> usize {
        self.runs.iter().map(|run| run.text.len()).sum::<usize>() + self.buffer.len()
    }

    fn effective_style(&self) -> InlineStyle {
        let mut style = InlineStyle::default();
        for tag in &self.styles {
            match *tag {
                StyleTag::Bold => style.bold = true,
                StyleTag::Italic => style.italic = true,
                StyleTag::Underline => style.underline = true,
                StyleTag::Strikethrough => style.strikethrough = true,
                StyleTag::Color(color) => style.color = Some(color),
                StyleTag::Size(size) => style.size = Some(size),
                StyleTag::Mono => style.mono = true,
                StyleTag::Neutral => {}
            }
        }
        style
    }

    fn current_link(&self) -> Option<String> {
        self.urls.iter().rev().find_map(|(_, href)| href.clone())
    }

    fn flush_run(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        self.runs.push(InlineRun {
            text: std::mem::take(&mut self.buffer),
            style: self.effective_style(),
            link: self.current_link(),
        });
    }

    fn push_style(&mut self, tag: StyleTag) {
        self.flush_run();
        self.styles.push(tag);
    }

    fn pop_style(&mut self, matches: fn(&StyleTag) -> bool, literal: &str) {
        if let Some(position) = self.styles.iter().rposition(matches) {
            // The buffered text belongs to the style being closed.
            self.flush_run();
            self.styles.remove(position);
        } else {
            self.push_literal(literal);
        }
    }

    fn push_literal(&mut self, literal: &str) {
        self.buffer.push_str(literal);
    }

    fn paragraph_has_content(&self) -> bool {
        self.runs
            .iter()
            .any(|run| run.text.chars().any(|c| !c.is_whitespace()))
            || self.buffer.chars().any(|c| !c.is_whitespace())
    }

    fn take_paragraph(&mut self) -> Option<Vec<InlineRun>> {
        self.flush_run();
        if !self.paragraph_has_content() {
            self.runs.clear();
            return None;
        }
        Some(std::mem::take(&mut self.runs))
    }

    fn flush_paragraph(&mut self) {
        if let Some(runs) = self.take_paragraph() {
            self.push_block(Block::Paragraph(runs));
        }
    }

    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(Container::Quote { blocks, .. }) => blocks.push(block),
            Some(Container::List { items, .. }) => {
                if let Block::Paragraph(runs) = block {
                    items.push(runs);
                }
            }
            None => {
                if self.root_blocks.len() < MAX_BLOCKS {
                    self.root_blocks.push(block);
                }
            }
        }
    }

    fn close_url(&mut self) {
        let Some((start, explicit)) = self.urls.pop() else {
            self.push_literal("[/url]");
            return;
        };
        self.flush_run();
        let href = explicit.or_else(|| {
            // `[url]href[/url]`: the inner text is the address.
            let mut collected = String::new();
            let mut offset = 0usize;
            for run in &self.runs {
                let run_end = offset + run.text.len();
                if run_end > start {
                    collected.push_str(&run.text[start.saturating_sub(offset)..]);
                }
                offset = run_end;
            }
            Some(collected.trim().to_string())
        });
        let Some(href) = href.filter(|href| is_valid_url(href)) else {
            return;
        };
        let mut offset = 0usize;
        for run in &mut self.runs {
            let run_end = offset + run.text.len();
            if run_end > start {
                run.link = Some(href.clone());
            }
            offset = run_end;
        }
    }

    fn finish(mut self, in_code: Option<String>) -> Vec<Block> {
        if let Some(code) = in_code {
            self.push_block(Block::Code(code));
        }
        self.flush_paragraph();
        while let Some(container) = self.stack.pop() {
            match container {
                Container::Quote {
                    attribution,
                    blocks,
                } => self.push_block(Block::Quote {
                    attribution,
                    blocks,
                }),
                Container::List { ordered, mut items } => {
                    if let Some(runs) = self.take_paragraph() {
                        items.push(runs);
                    }
                    self.push_block(Block::List { ordered, items });
                }
            }
        }
        self.root_blocks
    }
}

fn is_valid_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn parse_color(arg: &str) -> Option<u32> {
    let arg = arg.trim().trim_matches(['"', '\'']).to_ascii_lowercase();
    if let Some(hex) = arg.strip_prefix('#') {
        let value = u32::from_str_radix(hex, 16).ok()?;
        return match hex.len() {
            3 => {
                let r = (value >> 8) & 0xf;
                let g = (value >> 4) & 0xf;
                let b = value & 0xf;
                Some((r * 0x11) << 16 | (g * 0x11) << 8 | (b * 0x11))
            }
            6 => Some(value & 0x00ff_ffff),
            _ => None,
        };
    }
    Some(match arg.as_str() {
        "red" => 0xff453a,
        "green" => 0x30d158,
        "blue" => 0x409cff,
        "yellow" => 0xe0a020,
        "orange" => 0xff9f0a,
        "purple" | "violet" => 0xbf5af2,
        "pink" => 0xff7eb6,
        "cyan" | "teal" => 0x64d2ff,
        "white" => 0xf2f4f8,
        "gray" | "grey" => 0x9a9da1,
        "black" => 0x9a9da1,
        _ => return None,
    })
}

/// Keeps author colors legible on the dark glass: near-black falls back to
/// the primary text color; other dim colors are brightened toward it.
fn legible_color(rgb: u32) -> u32 {
    let channel = |shift: u32| ((rgb >> shift) & 0xff) as f64 / 255.0;
    let linear = |v: f64| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance =
        0.2126 * linear(channel(16)) + 0.7152 * linear(channel(8)) + 0.0722 * linear(channel(0));
    if luminance >= 0.10 {
        return rgb;
    }
    if luminance < 0.01 {
        return SCRIBE_FOREGROUND;
    }
    let mix = |shift: u32| -> u32 {
        let fg = ((rgb >> shift) & 0xff) as f64;
        let hi = ((SCRIBE_FOREGROUND >> shift) & 0xff) as f64;
        (fg * 0.45 + hi * 0.55).round() as u32
    };
    (mix(16) << 16) | (mix(8) << 8) | mix(0)
}

fn map_size(arg: &str) -> Option<f32> {
    let value: f32 = arg.trim().trim_matches(['"', '\'']).parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    Some(if value <= 12.0 {
        12.0
    } else if value <= 14.0 {
        13.0
    } else if value <= 16.0 {
        15.0
    } else {
        17.0
    })
}

fn decode_entity(input: &[char], at: usize) -> Option<(char, usize)> {
    let rest: String = input[at..].iter().take(7).collect();
    for (entity, value) in [
        ("&amp;", '&'),
        ("&quot;", '"'),
        ("&#39;", '\''),
        ("&#x27;", '\''),
        ("&lt;", '<'),
        ("&gt;", '>'),
        ("&nbsp;", '\u{00a0}'),
    ] {
        if rest.starts_with(entity) {
            return Some((value, entity.len()));
        }
    }
    None
}

/// Scans for a case-insensitive `closer` starting at `chars[start]`,
/// returning its index. Used for the verbatim contents of [img] and [code].
fn find_closer(chars: &[char], start: usize, closer: &str) -> Option<usize> {
    let mut scan = start;
    while scan < chars.len() {
        if chars[scan] == '[' {
            let rest: String = chars[scan..].iter().take(closer.len()).collect();
            if rest.to_ascii_lowercase() == closer {
                return Some(scan);
            }
        }
        scan += 1;
    }
    None
}

fn parse(input: &str) -> Vec<Block> {
    let chars: Vec<char> = input.chars().take(INPUT_CAP_CHARS).collect();
    let mut parser = Parser::default();
    let mut index = 0usize;
    let mut in_code: Option<String> = None;

    while index < chars.len() {
        let character = chars[index];

        if let Some(code) = in_code.as_mut() {
            // Inside block [code]: everything is literal until [/code].
            if character == '[' && {
                let rest: String = chars[index..].iter().take(7).collect();
                rest.to_ascii_lowercase().starts_with("[/code]")
            } {
                let code = in_code.take().unwrap_or_default();
                parser.push_block(Block::Code(code));
                index += 7;
                continue;
            }
            if character == '&'
                && let Some((value, len)) = decode_entity(&chars, index)
            {
                code.push(value);
                index += len;
                continue;
            }
            code.push(character);
            index += 1;
            continue;
        }

        match character {
            '[' => {
                let Some(end) = chars[index + 1..]
                    .iter()
                    .position(|c| *c == ']')
                    .map(|offset| index + 1 + offset)
                else {
                    parser.push_literal("[");
                    index += 1;
                    continue;
                };
                let token: String = chars[index + 1..end].iter().collect();
                let token_lower = token.to_ascii_lowercase();
                let (closing, body_lower, body_original) = match token_lower.strip_prefix('/') {
                    Some(_) => (true, token_lower[1..].trim(), token[1..].trim()),
                    None => (false, token_lower.trim(), token.trim()),
                };
                let (name, arg) = match body_original.split_once('=') {
                    Some((_, arg)) => (
                        body_lower
                            .split_once('=')
                            .map(|(name, _)| name.trim())
                            .unwrap_or(body_lower),
                        Some(arg.trim().to_string()),
                    ),
                    None => (body_lower, None),
                };
                index = end + 1;
                match (closing, name) {
                    (false, "b") => parser.push_style(StyleTag::Bold),
                    (true, "b") => parser.pop_style(|t| matches!(t, StyleTag::Bold), "[/b]"),
                    (false, "i") => parser.push_style(StyleTag::Italic),
                    (true, "i") => parser.pop_style(|t| matches!(t, StyleTag::Italic), "[/i]"),
                    (false, "u") => parser.push_style(StyleTag::Underline),
                    (true, "u") => parser.pop_style(|t| matches!(t, StyleTag::Underline), "[/u]"),
                    (false, "s") => parser.push_style(StyleTag::Strikethrough),
                    (true, "s") => {
                        parser.pop_style(|t| matches!(t, StyleTag::Strikethrough), "[/s]")
                    }
                    (false, "color") => {
                        if let Some(color) = arg.as_deref().and_then(parse_color) {
                            parser.push_style(StyleTag::Color(legible_color(color)));
                        }
                    }
                    (true, "color") => {
                        parser.pop_style(|t| matches!(t, StyleTag::Color(_)), "[/color]")
                    }
                    (false, "size") => {
                        if let Some(size) = arg.as_deref().and_then(map_size) {
                            parser.push_style(StyleTag::Size(size));
                        }
                    }
                    (true, "size") => {
                        parser.pop_style(|t| matches!(t, StyleTag::Size(_)), "[/size]")
                    }
                    (false, "font" | "center" | "left" | "right") => {
                        parser.push_style(StyleTag::Neutral)
                    }
                    (true, "font" | "center" | "left" | "right") => {
                        parser.pop_style(|t| matches!(t, StyleTag::Neutral), "")
                    }
                    (false, "url") => {
                        parser.flush_run();
                        let href = arg
                            .map(|href| href.trim_matches(['"', '\'']).to_string())
                            .filter(|href| is_valid_url(href));
                        parser.urls.push((parser.text_len(), href));
                    }
                    (true, "url") => parser.close_url(),
                    (false, "img") => {
                        let Some(close) = find_closer(&chars, index, "[/img]") else {
                            parser.push_literal("[img]");
                            continue;
                        };
                        let url: String = chars[index..close].iter().collect();
                        index = close + 6;
                        let url = url.trim().trim_matches(['"', '\'']);
                        if is_valid_url(url) && parser.images < MAX_IMAGES {
                            parser.images += 1;
                            parser.flush_paragraph();
                            parser.push_block(Block::Image(url.to_string()));
                        }
                    }
                    (true, "img") => parser.push_literal("[/img]"),
                    (false, "code") => {
                        if parser.text_len() > 0 {
                            // Mid-paragraph [code] stays an inline mono span.
                            parser.push_style(StyleTag::Mono);
                        } else {
                            parser.flush_paragraph();
                            in_code = Some(String::new());
                        }
                    }
                    (true, "code") => parser.pop_style(|t| matches!(t, StyleTag::Mono), "[/code]"),
                    (false, "quote") => {
                        parser.flush_paragraph();
                        if parser.stack.len() >= MAX_QUOTE_DEPTH {
                            parser.push_literal(&token);
                        } else {
                            parser.stack.push(Container::Quote {
                                attribution: arg.filter(|name| !name.is_empty()),
                                blocks: Vec::new(),
                            });
                        }
                    }
                    (true, "quote") => {
                        parser.flush_paragraph();
                        match parser.stack.pop() {
                            Some(Container::Quote {
                                attribution,
                                blocks,
                            }) => parser.push_block(Block::Quote {
                                attribution,
                                blocks,
                            }),
                            Some(other @ Container::List { .. }) => {
                                parser.stack.push(other);
                                parser.push_literal("[/quote]");
                            }
                            None => parser.push_literal("[/quote]"),
                        }
                    }
                    (false, "list") => {
                        parser.flush_paragraph();
                        parser.stack.push(Container::List {
                            ordered: arg.as_deref() == Some("1"),
                            items: Vec::new(),
                        });
                    }
                    (true, "list") => {
                        parser.flush_paragraph();
                        match parser.stack.pop() {
                            Some(Container::List { ordered, mut items }) => {
                                if let Some(runs) = parser.take_paragraph() {
                                    items.push(runs);
                                }
                                parser.push_block(Block::List { ordered, items });
                            }
                            Some(other @ Container::Quote { .. }) => {
                                parser.stack.push(other);
                                parser.push_literal("[/list]");
                            }
                            None => parser.push_literal("[/list]"),
                        }
                    }
                    (false, "*") => {
                        if matches!(parser.stack.last(), Some(Container::List { .. })) {
                            if let Some(runs) = parser.take_paragraph()
                                && let Some(Container::List { items, .. }) = parser.stack.last_mut()
                            {
                                items.push(runs);
                            }
                        } else {
                            parser.push_literal("[*]");
                        }
                    }
                    (false, "br") => parser.push_literal("\n"),
                    _ => {
                        // Unknown or malformed tag: degrade to literal text.
                        parser.push_literal(&token);
                    }
                }
            }
            '<' => {
                let Some(end) = chars[index + 1..]
                    .iter()
                    .position(|c| *c == '>')
                    .map(|offset| index + 1 + offset)
                else {
                    parser.push_literal("<");
                    index += 1;
                    continue;
                };
                let tag: String = chars[index + 1..end]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_ascii_lowercase();
                match tag.trim_end_matches('/').trim() {
                    "br" => parser.push_literal("\n"),
                    "p" | "/p" => parser.flush_paragraph(),
                    _ => {}
                }
                index = end + 1;
            }
            '&' => {
                if let Some((value, len)) = decode_entity(&chars, index) {
                    parser.buffer.push(value);
                    index += len;
                } else {
                    parser.buffer.push('&');
                    index += 1;
                }
            }
            _ => {
                parser.buffer.push(character);
                index += 1;
            }
        }
    }

    parser.finish(in_code)
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

fn base_text_style(size: f32, muted: bool) -> TextStyle {
    TextStyle {
        color: if muted {
            gpui::rgba(SCRIBE_TEXT_SECONDARY_RGBA).into()
        } else {
            gpui::rgb(SCRIBE_FOREGROUND).into()
        },
        font_family: ".Segoe UI Variable Text".into(),
        font_size: px(size).into(),
        line_height: relative(1.45),
        ..Default::default()
    }
}

struct RenderedSegment {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    font_overrides: Vec<(Range<usize>, SharedString)>,
    links: Vec<(Range<usize>, String)>,
}

fn build_segment(runs: &[InlineRun]) -> RenderedSegment {
    let mut segment = RenderedSegment {
        text: String::new(),
        highlights: Vec::new(),
        font_overrides: Vec::new(),
        links: Vec::new(),
    };
    for run in runs {
        if run.text.is_empty() {
            continue;
        }
        let start = segment.text.len();
        segment.text.push_str(&run.text);
        let range = start..segment.text.len();
        let is_link = run.link.is_some();
        let highlight = HighlightStyle {
            color: run
                .style
                .color
                .map(|color| gpui::rgb(color).into())
                .or(if is_link {
                    Some(gpui::rgb(SCRIBE_PRIMARY).into())
                } else {
                    None
                }),
            font_weight: run.style.bold.then_some(FontWeight::BOLD),
            font_style: run.style.italic.then_some(FontStyle::Italic),
            underline: (run.style.underline || is_link).then_some(UnderlineStyle {
                thickness: px(1.0),
                color: Some(gpui::rgb(SCRIBE_PRIMARY).into()),
                wavy: false,
            }),
            strikethrough: run.style.strikethrough.then_some(StrikethroughStyle {
                thickness: px(1.0),
                color: None,
            }),
            background_color: run
                .style
                .mono
                .then(|| gpui::rgba(SCRIBE_SURFACE_ACTIVE_RGBA).into()),
            fade_out: None,
        };
        if highlight != HighlightStyle::default() {
            segment.highlights.push((range.clone(), highlight));
        }
        if run.style.mono {
            segment
                .font_overrides
                .push((range.clone(), SharedString::from("Consolas")));
        }
        if let Some(url) = run.link.clone() {
            segment.links.push((range, url));
        }
    }
    segment
}

fn render_segment(
    runs: &[InlineRun],
    size: f32,
    muted: bool,
    id: (&'static str, usize),
) -> AnyElement {
    let segment = build_segment(runs);
    let style = base_text_style(size, muted);
    let mut styled =
        StyledText::new(segment.text).with_default_highlights(&style, segment.highlights);
    if !segment.font_overrides.is_empty() {
        styled = styled.with_font_family_overrides(segment.font_overrides);
    }
    if segment.links.is_empty() {
        return styled.into_any_element();
    }
    let ranges: Vec<Range<usize>> = segment
        .links
        .iter()
        .map(|(range, _)| range.clone())
        .collect();
    let urls: Vec<String> = segment.links.iter().map(|(_, url)| url.clone()).collect();
    InteractiveText::new(id, styled)
        .on_click(ranges, move |index, _, cx| {
            if let Some(url) = urls.get(index) {
                cx.open_url(url);
            }
        })
        .into_any_element()
}

fn render_paragraph(
    runs: &[InlineRun],
    muted: bool,
    id_prefix: &'static str,
    counter: &mut usize,
) -> AnyElement {
    // StyledText shares one font size per layout, so [size] changes split the
    // paragraph into size-uniform blocks.
    let mut segments: Vec<(f32, Vec<InlineRun>)> = Vec::new();
    for run in runs {
        let size = run.style.size.unwrap_or(if muted { 12.0 } else { 13.0 });
        match segments.last_mut() {
            Some((last_size, last_runs)) if *last_size == size => last_runs.push(run.clone()),
            _ => segments.push((size, vec![run.clone()])),
        }
    }
    let children: Vec<AnyElement> = segments
        .iter()
        .map(|(size, runs)| {
            *counter += 1;
            render_segment(runs, *size, muted, (id_prefix, *counter))
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .children(children)
        .into_any_element()
}

fn render_blocks(
    blocks: &[Block],
    muted: bool,
    id_prefix: &'static str,
    counter: &mut usize,
) -> Vec<AnyElement> {
    blocks
        .iter()
        .map(|block| -> AnyElement {
            match block {
                Block::Paragraph(runs) => render_paragraph(runs, muted, id_prefix, counter),
                Block::List { ordered, items } => div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .children(items.iter().enumerate().map(|(index, item)| {
                        let marker = if *ordered {
                            format!("{}.", index + 1)
                        } else {
                            "•".to_string()
                        };
                        div()
                            .flex()
                            .items_start()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .w(px(18.0))
                                    .flex_none()
                                    .text_size(px(12.0))
                                    .text_color(if *ordered {
                                        gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA)
                                    } else {
                                        gpui::rgb(SCRIBE_PRIMARY)
                                    })
                                    .child(marker),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .child(render_paragraph(item, muted, id_prefix, counter)),
                            )
                            .into_any_element()
                    }))
                    .into_any_element(),
                Block::Quote {
                    attribution,
                    blocks,
                } => div()
                    .w_full()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                    .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                    .p(px(10.0))
                    .flex()
                    .gap(px(9.0))
                    .child(
                        div()
                            .w(px(3.0))
                            .flex_none()
                            .rounded(px(1.5))
                            .bg(gpui::rgba(SCRIBE_ACCENT_SOFT_RGBA)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .when_some(attribution.clone(), |column, name| {
                                column.child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_semibold()
                                        .text_color(gpui::rgba(SCRIBE_TEXT_TERTIARY_RGBA))
                                        .child(format!("{name} wrote:")),
                                )
                            })
                            .children(render_blocks(blocks, true, id_prefix, counter)),
                    )
                    .into_any_element(),
                Block::Code(code) => div()
                    .w_full()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(gpui::rgba(SCRIBE_HAIRLINE_RGBA))
                    .bg(gpui::rgba(SCRIBE_SURFACE_RGBA))
                    .p(px(10.0))
                    .child(
                        div().overflow_x_scrollbar().child(
                            div()
                                .flex()
                                .flex_col()
                                .font_family("Consolas")
                                .text_size(px(12.0))
                                .text_color(gpui::rgb(SCRIBE_FOREGROUND))
                                .children(code.lines().map(|line| {
                                    div().whitespace_nowrap().child(if line.is_empty() {
                                        " ".to_string()
                                    } else {
                                        line.to_string()
                                    })
                                })),
                        ),
                    )
                    .into_any_element(),
                Block::Image(url) => div()
                    .w_full()
                    .max_h(px(220.0))
                    .overflow_hidden()
                    .flex()
                    .child(
                        img(url.clone())
                            .max_w(px(560.0))
                            .max_h(px(220.0))
                            .object_fit(ObjectFit::Contain)
                            .rounded(px(8.0))
                            .with_fallback(|| div().into_any_element()),
                    )
                    .into_any_element(),
            }
        })
        .collect()
}

/// Renders an ESOUI BBCode/HTML description or changelog onto the glass
/// tokens. `id_prefix` namespaces interactive text element ids per sheet.
pub(crate) fn render_bbcode(id_prefix: &'static str, input: &str) -> AnyElement {
    let blocks = parse(input);
    let mut counter = 0usize;
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .children(render_blocks(&blocks, false, id_prefix, &mut counter))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph_runs(blocks: &[Block]) -> Vec<&Vec<InlineRun>> {
        blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(runs) => Some(runs),
                _ => None,
            })
            .collect()
    }

    fn paragraph_text(blocks: &[Block]) -> String {
        paragraph_runs(blocks)
            .into_iter()
            .flat_map(|runs| runs.iter().map(|run| run.text.clone()))
            .collect()
    }

    #[test]
    fn inline_styles_produce_correct_runs() {
        let blocks = parse("[b]bold[/b] plain [i]it[/i][u]u[/u][s]s[/s]");
        let runs = paragraph_runs(&blocks);
        assert_eq!(runs.len(), 1);
        let runs = runs[0];
        assert_eq!(runs.len(), 5);
        assert_eq!(runs[0].text, "bold");
        assert!(runs[0].style.bold && !runs[0].style.italic);
        assert_eq!(runs[1].text, " plain ");
        assert_eq!(runs[1].style, InlineStyle::default());
        assert_eq!(runs[2].text, "it");
        assert!(runs[2].style.italic);
        assert_eq!(runs[3].text, "u");
        assert!(runs[3].style.underline);
        assert_eq!(runs[4].text, "s");
        assert!(runs[4].style.strikethrough);
        // Byte ranges in the flattened paragraph.
        let segment = build_segment(runs);
        assert_eq!(segment.text, "bold plain itus");
        assert!(segment.highlights.iter().any(|(range, _)| *range == (0..4)));
        assert!(
            segment
                .highlights
                .iter()
                .any(|(range, _)| *range == (11..13))
        );
    }

    #[test]
    fn nested_styles_merge() {
        let blocks = parse("[b]a[i]b[/i]c[/b]");
        let runs = paragraph_runs(&blocks);
        let runs = runs[0];
        assert_eq!(runs.len(), 3);
        assert!(runs[0].style.bold && !runs[0].style.italic);
        assert!(runs[1].style.bold && runs[1].style.italic);
        assert!(runs[2].style.bold && !runs[2].style.italic);
    }

    #[test]
    fn color_and_size_runs() {
        let blocks = parse(
            "[color=#ff0000]red[/color][color=#000000]dark[/color][size=20]big[/size][size=5]small[/size]",
        );
        let runs = paragraph_runs(&blocks);
        let runs = runs[0];
        assert_eq!(runs[0].style.color, Some(0xff0000));
        // Near-black author colors fall back to the primary text color.
        assert_eq!(runs[1].style.color, Some(SCRIBE_FOREGROUND));
        assert_eq!(runs[2].style.size, Some(17.0));
        assert_eq!(runs[3].style.size, Some(12.0));
    }

    #[test]
    fn named_colors_parse() {
        let blocks = parse("[color=red]x[/color][color=blue]y[/color]");
        let runs = paragraph_runs(&blocks);
        let runs = runs[0];
        assert_eq!(runs[0].style.color, Some(0xff453a));
        assert_eq!(runs[1].style.color, Some(0x409cff));
    }

    #[test]
    fn explicit_url_linkifies_label() {
        let blocks = parse("[url=https://example.test/page]the label[/url]");
        let runs = paragraph_runs(&blocks);
        let runs = runs[0];
        assert_eq!(runs[0].text, "the label");
        assert_eq!(runs[0].link.as_deref(), Some("https://example.test/page"));
    }

    #[test]
    fn quoted_url_arg_is_accepted() {
        let blocks = parse("[URL=\"https://example.test\"]safe[/URL]");
        let runs = paragraph_runs(&blocks);
        assert_eq!(runs[0][0].link.as_deref(), Some("https://example.test"));
    }

    #[test]
    fn bare_url_uses_inner_text_as_href() {
        let blocks = parse("[url]https://example.test[/url]");
        let runs = paragraph_runs(&blocks);
        let runs = runs[0];
        assert_eq!(runs[0].text, "https://example.test");
        assert_eq!(runs[0].link.as_deref(), Some("https://example.test"));
    }

    #[test]
    fn dangerous_link_schemes_render_as_plain_text() {
        for input in [
            "[url=javascript:alert(1)]click[/url]",
            "[url=file:///c:/windows]click[/url]",
            "[url=data:text/html;base64,x]click[/url]",
            "[url]javascript:alert(1)[/url]",
            "[url=ftp://example.test]click[/url]",
        ] {
            let blocks = parse(input);
            let runs = paragraph_runs(&blocks);
            assert_eq!(runs.len(), 1, "{input}");
            assert!(runs[0].iter().all(|run| run.link.is_none()), "{input}");
        }
    }

    #[test]
    fn entities_decode() {
        let blocks = parse("a &amp; b &quot;q&quot; &#39;c&#39; &lt;tag&gt; &nbsp;z");
        let text = paragraph_text(&blocks);
        assert_eq!(text, "a & b \"q\" 'c' <tag> \u{00a0}z");
    }

    #[test]
    fn html_breaks_split_paragraphs() {
        let blocks = parse("one<br>two<p>three</p>four");
        let runs = paragraph_runs(&blocks);
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0][0].text, "one\ntwo");
        assert_eq!(runs[1][0].text, "three");
        assert_eq!(runs[2][0].text, "four");
    }

    #[test]
    fn unordered_and_ordered_lists() {
        let blocks = parse("[list][*]one[*]two[/list]");
        match &blocks[0] {
            Block::List { ordered, items } => {
                assert!(!ordered);
                assert_eq!(items.len(), 2);
                assert_eq!(items[0][0].text, "one");
                assert_eq!(items[1][0].text, "two");
            }
            other => panic!("expected list, got {other:?}"),
        }
        let blocks = parse("[list=1][*]a[*]b[/list]");
        match &blocks[0] {
            Block::List { ordered, items } => {
                assert!(ordered);
                assert_eq!(items.len(), 2);
            }
            other => panic!("expected ordered list, got {other:?}"),
        }
    }

    #[test]
    fn quote_with_attribution() {
        let blocks = parse("[quote=Bob]hello there[/quote]");
        match &blocks[0] {
            Block::Quote {
                attribution,
                blocks,
            } => {
                assert_eq!(attribution.as_deref(), Some("Bob"));
                assert_eq!(paragraph_text(blocks), "hello there");
            }
            other => panic!("expected quote, got {other:?}"),
        }
    }

    #[test]
    fn code_block_keeps_contents_verbatim() {
        let blocks = parse("[code]fn x() { [b]not bold[/b] }[/code]");
        match &blocks[0] {
            Block::Code(code) => assert_eq!(code, "fn x() { [b]not bold[/b] }"),
            other => panic!("expected code block, got {other:?}"),
        }
    }

    #[test]
    fn inline_code_mid_paragraph_is_mono() {
        let blocks = parse("use [code]cargo check[/code] often");
        let runs = paragraph_runs(&blocks);
        let runs = runs[0];
        assert!(
            runs.iter()
                .any(|run| run.style.mono && run.text == "cargo check")
        );
        assert!(matches!(blocks[0], Block::Paragraph(_)));
    }

    #[test]
    fn images_extract_and_cap() {
        let blocks = parse("[img]https://cdn.test/a.png[/img]");
        assert!(matches!(&blocks[0], Block::Image(url) if url == "https://cdn.test/a.png"));

        let blocks = parse("[img]javascript:alert(1)[/img]");
        assert!(blocks.is_empty());

        let many = "[img]https://cdn.test/a.png[/img]".repeat(9);
        let blocks = parse(&many);
        let images = blocks
            .iter()
            .filter(|block| matches!(block, Block::Image(_)))
            .count();
        assert_eq!(images, 6);
    }

    #[test]
    fn malformed_input_degrades_to_literal_text() {
        assert_eq!(paragraph_text(&parse("a [ b")), "a [ b");
        assert_eq!(paragraph_text(&parse("x [/b] y")), "x [/b] y");
        assert_eq!(paragraph_text(&parse("x [/url] y")), "x [/url] y");
        // Unclosed opener applies through the end without leaking the tag.
        let blocks = parse("[b]bold forever");
        let runs = paragraph_runs(&blocks);
        assert_eq!(runs[0][0].text, "bold forever");
        assert!(runs[0][0].style.bold);
        // Unclosed containers still emit.
        assert!(matches!(parse("[quote]open")[0], Block::Quote { .. }));
        assert!(matches!(parse("[list][*]open")[0], Block::List { .. }));
        assert!(matches!(parse("[code]open")[0], Block::Code(_)));
    }

    #[test]
    fn adversarial_input_never_panics() {
        for input in [
            "[[[[[[",
            "]]]]]]",
            "][",
            "&amp",
            "&",
            "<",
            "<p",
            "[img]",
            "[url=]",
            "[=]",
            "[]",
            "[b][i][u][s][color=[size=[quote=[list=",
            "\u{0}\u{1}\u{2}",
            "[url=https://x.test][b]mixed[/url]nesting[/b]",
        ] {
            let blocks = parse(input);
            // Construction of the element tree must also be panic-free.
            let _ = render_blocks(&blocks, false, "test", &mut 0);
        }
    }

    #[test]
    fn input_is_capped() {
        let input = "x".repeat(20_000);
        let text = paragraph_text(&parse(&input));
        assert_eq!(text.len(), 12_000);
    }

    #[test]
    fn bbcode_section_flattens_like_the_old_stripper() {
        let blocks = parse(
            "<p>[SIZE=4][B]Useful[/B][/SIZE] &amp; [URL=\"https://example.test\"]safe[/URL]</p>",
        );
        assert_eq!(paragraph_text(&blocks), "Useful & safe");
    }
}

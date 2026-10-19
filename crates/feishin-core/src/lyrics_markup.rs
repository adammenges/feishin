//! The lyric HTML allowlist shared by plain lines and timed word twins.
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub href: Option<String>,
    pub ruby: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub spans: Vec<Span>,
    /// Collapsed paragraph margin in em units.
    pub margin_before: f32,
}

#[derive(Default)]
struct Flow {
    blocks: Vec<Block>,
    margin: f32,
}
impl Flow {
    fn flush(&mut self, spans: &mut Vec<Span>) {
        if spans.iter().all(|span| {
            span.ruby.is_none()
                && span
                    .text
                    .chars()
                    .all(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0c'))
        }) {
            spans.clear();
            return;
        }
        self.blocks.push(Block {
            spans: std::mem::take(spans),
            margin_before: self.margin,
        });
        self.margin = 0.;
    }
}

pub fn parse_layout(input: &str) -> Vec<Block> {
    if !input.contains(['<', '&']) {
        return vec![Block {
            spans: parse(input),
            margin_before: 0.,
        }];
    }
    let dom = parse_document(RcDom::default(), Default::default())
        .one(input.chars().take(1_000_000).collect::<String>());
    let mut spans = vec![];
    let mut flow = Flow::default();
    visit(
        &dom.document,
        &Span::default(),
        &mut spans,
        0,
        Some(&mut flow),
    );
    flow.flush(&mut spans);
    if flow.margin > 0. {
        flow.blocks.push(Block {
            spans: vec![],
            margin_before: flow.margin,
        });
    }
    flow.blocks
}

pub fn parse(input: &str) -> Vec<Span> {
    if !input.contains(['<', '&']) {
        return vec![Span {
            text: input.into(),
            ..Default::default()
        }];
    }
    let dom = parse_document(RcDom::default(), Default::default())
        .one(input.chars().take(1_000_000).collect::<String>());
    let mut output = vec![];
    visit(&dom.document, &Span::default(), &mut output, 0, None);
    output
}
fn push(output: &mut Vec<Span>, mut style: Span, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = output.last_mut().filter(|last| {
        last.bold == style.bold
            && last.italic == style.italic
            && last.href == style.href
            && last.ruby.is_none()
            && style.ruby.is_none()
    }) {
        last.text.push_str(text);
    } else {
        style.text = text.into();
        output.push(style);
    }
}
fn visit(
    node: &Handle,
    style: &Span,
    output: &mut Vec<Span>,
    depth: usize,
    mut flow: Option<&mut Flow>,
) {
    if depth >= 128 {
        return;
    }
    match &node.data {
        NodeData::Text { contents } => push(output, style.clone(), &contents.borrow()),
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref();
            if matches!(
                tag,
                "head"
                    | "script"
                    | "style"
                    | "iframe"
                    | "object"
                    | "embed"
                    | "template"
                    | "noscript"
                    | "xmp"
                    | "noembed"
                    | "noframes"
            ) {
                return;
            }
            let mut style = style.clone();
            match tag {
                "b" | "strong" => style.bold = true,
                "i" | "em" => style.italic = true,
                "a" => {
                    style.href = attrs
                        .borrow()
                        .iter()
                        .find(|attr| attr.name.local.as_ref() == "href")
                        .and_then(|attr| {
                            let value = attr.value.to_string();
                            let value = if value.starts_with("//") {
                                format!("https:{value}")
                            } else {
                                value
                            };
                            url::Url::parse(&value)
                                .ok()
                                .filter(|url| {
                                    matches!(url.scheme(), "http" | "https")
                                        && url.host_str().is_some()
                                })
                                .map(|_| value)
                        })
                }
                "ruby" => {
                    let mut base = vec![];
                    let mut reading = vec![];
                    for child in node.children.borrow().iter() {
                        match &child.data {
                            NodeData::Element { name, .. } if name.local.as_ref() == "rt" => {
                                visit(child, &Span::default(), &mut reading, depth + 1, None)
                            }
                            NodeData::Element { name, .. } if name.local.as_ref() == "rp" => {}
                            _ => visit(child, &style, &mut base, depth + 1, None),
                        }
                    }
                    style.ruby = Some(reading.iter().map(|span| span.text.as_str()).collect());
                    push(
                        output,
                        style,
                        &base
                            .iter()
                            .map(|span| span.text.as_str())
                            .collect::<String>(),
                    );
                    return;
                }
                _ => {}
            }
            let block = matches!(tag, "div" | "p");
            if block {
                if let Some(flow) = flow.as_deref_mut() {
                    flow.flush(output);
                    if tag == "p" {
                        flow.margin = flow.margin.max(1.);
                    }
                } else if output.last().is_some_and(|span| !span.text.ends_with('\n')) {
                    push(output, Span::default(), "\n");
                }
            }
            for child in node.children.borrow().iter() {
                visit(child, &style, output, depth + 1, flow.as_deref_mut());
            }
            if block {
                if let Some(flow) = flow.as_deref_mut() {
                    flow.flush(output);
                    if tag == "p" {
                        flow.margin = flow.margin.max(1.);
                    }
                } else {
                    push(output, Span::default(), "\n");
                }
            }
        }
        _ => {
            for child in node.children.borrow().iter() {
                visit(child, style, output, depth + 1, flow.as_deref_mut());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lyric_markup_preserves_allowed_formatting_and_readings() {
        let spans = super::parse(
            "<b>A &amp; <i>B</i></b> <ruby>君<rp>(</rp><rt>きみ</rt><rp>)</rp></ruby> <a href='//example.com/music' onclick='bad()'>link</a><a href='javascript:bad()'>plain</a><script>hidden</script><img src='file:///private/secret'><br><span style='color:red'>end</span>",
        );
        assert_eq!(
            spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>(),
            "A & B 君 linkplainend"
        );
        assert!(
            spans
                .iter()
                .any(|span| span.text == "B" && span.bold && span.italic)
        );
        assert!(
            spans
                .iter()
                .any(|span| span.text == "君" && span.ruby.as_deref() == Some("きみ"))
        );
        assert!(
            spans.iter().any(|span| span.text == "link"
                && span.href.as_deref() == Some("https://example.com/music"))
        );
        assert!(
            spans
                .iter()
                .find(|span| span.text.contains("plain"))
                .unwrap()
                .href
                .is_none()
        );
        assert_eq!(
            super::parse("literal <3 & unknown"),
            vec![super::Span {
                text: "literal <3 & unknown".into(),
                ..Default::default()
            }]
        );
    }

    #[test]
    fn lyric_markup_matches_original_sanitizer() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/lyrics-markup-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            assert_eq!(
                serde_json::to_value(super::parse(case["input"].as_str().unwrap())).unwrap(),
                case["spans"],
                "{case}"
            );
        }
    }
}

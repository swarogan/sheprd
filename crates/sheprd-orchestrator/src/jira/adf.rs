//! Jira's descriptions, which are Atlassian Document Format JSON, as the
//! Markdown every other provider's body is written in. A node it does not
//! know keeps its text; a malformed one is skipped rather than failing the
//! listing. Nesting past [`MAX_DEPTH`] is dropped and the output stops at
//! [`MAX_OUTPUT`] bytes, so one description costs time linear in its size.

use serde_json::Value;

/// The deepest node nesting read.
pub const MAX_DEPTH: usize = 24;
/// The longest Markdown one description yields.
pub const MAX_OUTPUT: usize = 64 * 1024;
/// A link is kept only to these schemes.
const SCHEMES: [&str; 3] = ["http://", "https://", "mailto:"];

/// `description` as Markdown: the text of a plain string, the conversion of
/// a document, and `None` for anything else or when no text is left.
pub fn to_markdown(description: &Value) -> Option<String> {
    let text = match description {
        Value::String(text) => text.trim().to_owned(),
        Value::Object(_) => {
            let mut text = Converter::default().node(description, 0).join("\n");
            let mut end = MAX_OUTPUT.min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            text.trim().to_owned()
        }
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

#[derive(Default)]
struct Converter {
    /// Bytes of text read so far, which stops the walk once past the cap.
    emitted: usize,
}

fn kind(node: &Value) -> &str {
    node.get("type").and_then(Value::as_str).unwrap_or("")
}

fn children(node: &Value) -> &[Value] {
    node.get("content")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn attr<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("attrs")?.get(name)
}

fn attr_str<'a>(node: &'a Value, name: &str) -> Option<&'a str> {
    attr(node, name)?.as_str().filter(|value| !value.is_empty())
}

/// Whether `node` is text that flows within a line.
fn is_inline(node: &Value) -> bool {
    matches!(
        kind(node),
        "text"
            | "hardBreak"
            | "mention"
            | "emoji"
            | "inlineCard"
            | "status"
            | "date"
            | "inlineExtension"
    )
}

impl Converter {
    fn done(&self) -> bool {
        self.emitted > MAX_OUTPUT
    }

    /// The lines of block `node`.
    fn node(&mut self, node: &Value, depth: usize) -> Vec<String> {
        if depth > MAX_DEPTH || self.done() {
            return Vec::new();
        }
        let content = children(node);
        match kind(node) {
            "paragraph" => self.paragraph(content, depth),
            "heading" => {
                let level = attr(node, "level")
                    .and_then(Value::as_u64)
                    .map_or(1, |level| level.clamp(1, 6)) as usize;
                let text = self.inline(content, depth + 1);
                vec![format!("{} {}", "#".repeat(level), text.replace('\n', " "))]
            }
            "bulletList" => self.list(content, depth, |_| "- ".to_owned()),
            "orderedList" => {
                let start = attr(node, "order").and_then(Value::as_u64).unwrap_or(1);
                self.list(content, depth, |index| {
                    format!("{}. ", start + index as u64)
                })
            }
            "codeBlock" => self.code(node),
            "blockquote" | "panel" => {
                let inner = self.blocks(content, depth + 1, false);
                inner
                    .into_iter()
                    .map(|line| {
                        if line.is_empty() {
                            ">".to_owned()
                        } else {
                            format!("> {line}")
                        }
                    })
                    .collect()
            }
            "rule" => vec!["---".to_owned()],
            "table" => self.table(content, depth),
            "mediaSingle" | "mediaGroup" | "media" => self.media(node, depth),
            "blockCard" | "embedCard" => attr_str(node, "url")
                .or_else(|| attr(node, "data")?.get("url")?.as_str())
                .map(|url| vec![url.to_owned()])
                .unwrap_or_default(),
            // `doc`, and a node this does not know: its text stays.
            _ if is_inline(node) => self.paragraph(std::slice::from_ref(node), depth),
            _ if content.iter().any(is_inline) => self.paragraph(content, depth),
            _ => self.blocks(content, depth + 1, false),
        }
    }

    /// `nodes` as blocks, a blank line between them; a list follows its item's
    /// text directly when `tight`.
    fn blocks(&mut self, nodes: &[Value], depth: usize, tight: bool) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for node in nodes {
            if self.done() {
                break;
            }
            let block = self.node(node, depth);
            if block.is_empty() {
                continue;
            }
            let list = matches!(kind(node), "bulletList" | "orderedList");
            if !lines.is_empty() && !(tight && list) {
                lines.push(String::new());
            }
            lines.extend(block);
        }
        lines
    }

    fn paragraph(&mut self, content: &[Value], depth: usize) -> Vec<String> {
        let text = self.inline(content, depth + 1);
        if text.trim().is_empty() {
            return Vec::new();
        }
        text.split('\n').map(str::to_owned).collect()
    }

    /// A list's items, each led by `marker(index)`, its other lines indented
    /// under it.
    fn list(
        &mut self,
        items: &[Value],
        depth: usize,
        marker: impl Fn(usize) -> String,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        for (index, item) in items.iter().enumerate() {
            if self.done() {
                break;
            }
            let body = if kind(item) == "listItem" {
                self.blocks(children(item), depth + 1, true)
            } else {
                self.node(item, depth + 1)
            };
            let lead = marker(index);
            let indent = " ".repeat(lead.len());
            for (at, line) in body.into_iter().enumerate() {
                lines.push(match (at, line.is_empty()) {
                    (0, _) => format!("{lead}{line}"),
                    (_, true) => String::new(),
                    _ => format!("{indent}{line}"),
                });
            }
        }
        lines
    }

    fn code(&mut self, node: &Value) -> Vec<String> {
        let text: String = children(node)
            .iter()
            .filter_map(|part| part.get("text")?.as_str())
            .collect();
        let language = attr_str(node, "language")
            .filter(|language| {
                language
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"+-_#.".contains(&b))
            })
            .unwrap_or("");
        let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
        let fence = "`".repeat((longest + 1).max(3));
        self.emitted += text.len();
        let mut lines = vec![format!("{fence}{language}")];
        lines.extend(text.split('\n').map(str::to_owned));
        lines.push(fence);
        lines
    }

    /// A pipe table: the first row heads it, each cell one line.
    fn table(&mut self, rows: &[Value], depth: usize) -> Vec<String> {
        let mut grid: Vec<Vec<String>> = Vec::new();
        for row in rows.iter().filter(|row| kind(row) == "tableRow") {
            if self.done() {
                break;
            }
            let cells = children(row)
                .iter()
                .map(|cell| {
                    let lines = self.blocks(children(cell), depth + 2, true);
                    lines
                        .iter()
                        .filter(|line| !line.is_empty())
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                        .replace('|', "\\|")
                })
                .collect();
            grid.push(cells);
        }
        let width = grid.iter().map(Vec::len).max().unwrap_or(0);
        if width == 0 {
            return Vec::new();
        }
        let row = |cells: &[String]| {
            let mut padded = cells.to_vec();
            padded.resize(width, String::new());
            format!("| {} |", padded.join(" | "))
        };
        let mut lines = vec![row(&grid[0])];
        lines.push(format!("|{}", " --- |".repeat(width)));
        lines.extend(grid[1..].iter().map(|cells| row(cells)));
        lines
    }

    /// A placeholder for each file shown, as Azure's descriptions keep one
    /// for an image: the file itself is not fetched.
    fn media(&mut self, node: &Value, depth: usize) -> Vec<String> {
        if depth > MAX_DEPTH {
            return Vec::new();
        }
        if kind(node) != "media" {
            return children(node)
                .iter()
                .flat_map(|part| self.media(part, depth + 1))
                .take(64)
                .collect();
        }
        vec![match attr_str(node, "alt") {
            Some(alt) => format!("[image: {}]", alt.replace(['[', ']', '\n'], " ")),
            None => "[image]".to_owned(),
        }]
    }

    /// The text of inline `nodes`, with their marks.
    fn inline(&mut self, nodes: &[Value], depth: usize) -> String {
        let mut text = String::new();
        if depth > MAX_DEPTH {
            return text;
        }
        for node in nodes {
            if self.done() {
                break;
            }
            let part = match kind(node) {
                "text" => marked(node),
                "hardBreak" => "  \n".to_owned(),
                "mention" => attr_str(node, "text").unwrap_or("@someone").to_owned(),
                "emoji" => attr_str(node, "text")
                    .or_else(|| attr_str(node, "shortName"))
                    .unwrap_or("")
                    .to_owned(),
                "inlineCard" => attr_str(node, "url")
                    .or_else(|| attr(node, "data")?.get("url")?.as_str())
                    .unwrap_or("")
                    .to_owned(),
                "status" => attr_str(node, "text")
                    .map(|status| format!("[{status}]"))
                    .unwrap_or_default(),
                "date" => attr_str(node, "timestamp")
                    .and_then(|millis| millis.parse::<i64>().ok())
                    .and_then(chrono::DateTime::from_timestamp_millis)
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .unwrap_or_default(),
                // Unknown inline content keeps its text.
                _ => match node.get("text").and_then(Value::as_str) {
                    Some(text) => text.to_owned(),
                    None => self.inline(children(node), depth + 1),
                },
            };
            self.emitted += part.len();
            text.push_str(&part);
        }
        text
    }
}

/// A text node with its marks as Markdown. Marks wrap the text without its
/// surrounding space, which Markdown would not read as emphasis.
fn marked(node: &Value) -> String {
    let Some(text) = node.get("text").and_then(Value::as_str) else {
        return String::new();
    };
    let core = text.trim();
    if core.is_empty() {
        return text.to_owned();
    }
    let (start, end) = (text.len() - text.trim_start().len(), text.trim_end().len());
    let marks = node
        .get("marks")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let has = |name: &str| marks.iter().any(|mark| kind(mark) == name);
    let mut out = core.to_owned();
    if has("code") {
        let longest = out.split(|c| c != '`').map(str::len).max().unwrap_or(0);
        let ticks = "`".repeat(longest + 1);
        out = format!("{ticks}{out}{ticks}");
    }
    if has("strike") {
        out = format!("~~{out}~~");
    }
    if has("em") {
        out = format!("*{out}*");
    }
    if has("strong") {
        out = format!("**{out}**");
    }
    if let Some(href) = marks
        .iter()
        .filter(|mark| kind(mark) == "link")
        .find_map(|mark| attr_str(mark, "href"))
        .filter(|href| SCHEMES.iter().any(|scheme| href.starts_with(scheme)))
    {
        let href = href
            .replace(' ', "%20")
            .replace('(', "%28")
            .replace(')', "%29");
        out = format!("[{out}]({href})");
    }
    format!("{}{out}{}", &text[..start], &text[end..])
}
